//! Learns the user's taste from their own keep / reject decisions. Everything stays on this device:
//! the labels and the tiny model are plain files in the app's data folder and are never sent anywhere.
//! The model is a logistic regression on the CLIP embedding each photo already has.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const SCALE: f32 = 16.0;
/// Need at least this many examples of each class before the model is used.
const MIN_PER_CLASS: usize = 3;
/// The most the learned taste may move a score, once there is enough evidence.
const MAX_STRENGTH: f32 = 0.35;

/// How many training examples it takes for the blend to reach this strength (the inverse of `strength`).
fn examples_for(strength: f32) -> usize {
    ((strength - 0.08) * 300.0).ceil().max(0.0) as usize
}

/// A photo from a folder of past picks ("this is what I choose"): only its embedding is kept, never the picture.
#[derive(Clone, Serialize, Deserialize)]
pub struct Reference {
    /// File name without extension, lower case: used to leave photos that are also in a project out of the "not chosen" examples.
    pub stem: String,
    /// Stored packed (base64 of little-endian floats, a quarter of the size of a JSON list); old files with plain lists still read.
    #[serde(with = "crate::cache::packed")]
    pub emb: Vec<f32>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Prefs {
    /// path -> true (keep) / false (reject)
    pub labels: HashMap<String, bool>,
    /// Photos from a folder of past picks; they count as "kept" examples.
    #[serde(default)]
    pub refs: Vec<Reference>,
    #[serde(default)]
    pub refs_from: String,
    w: Vec<f32>,
    b: f32,
    /// How many labelled photos the current weights were trained on.
    pub n: usize,
    #[serde(skip)]
    path: PathBuf,
}

/// What the settings show about a taste: the counts, whether it is in use, and the marks on the way to its full say.
#[derive(Serialize)]
pub struct PrefsInfo {
    pub labels: usize,
    pub keeps: usize,
    pub rejects: usize,
    pub refs: usize,
    pub refs_from: String,
    pub active: bool,
    /// How many examples the current weights were trained on (0 while the taste is not in use).
    pub trained: usize,
    /// How much of a score the learned taste may move right now.
    pub strength: f32,
    pub max_strength: f32,
    /// Keeps and rejects each needed before the taste is used at all.
    pub min_per_class: usize,
    /// Examples at which the strength passes half of its maximum, and at which it reaches it.
    pub half_at: usize,
    pub full_at: usize,
}

/// Reads a JSON file into its type. A file that is there but cannot be parsed is set aside under a timestamped name instead of
/// being replaced by the defaults on the next save: months of labels are worth a look by hand.
fn read_or_quarantine<T: serde::de::DeserializeOwned + Default>(path: &std::path::Path) -> T {
    let Ok(bytes) = std::fs::read(path) else { return T::default() };
    match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(e) => {
            let aside = crate::quarantine_name(path);
            eprintln!("{}: {e}; set aside as {}", path.display(), aside.display());
            let _ = std::fs::rename(path, &aside);
            T::default()
        }
    }
}

/// Written and synced to disk before the caller renames it into place.
fn write_synced(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

fn sigmoid(z: f32) -> f32 {
    1.0 / (1.0 + (-z).exp())
}

impl Prefs {
    pub fn load(path: PathBuf) -> Self {
        let mut p: Prefs = read_or_quarantine(&path);
        p.path = path;
        p
    }

    pub fn active(&self) -> bool {
        self.n >= 2 * MIN_PER_CLASS && !self.w.is_empty()
    }

    pub fn info(&self) -> PrefsInfo {
        let keeps = self.labels.values().filter(|v| **v).count();
        PrefsInfo {
            labels: self.labels.len(),
            keeps,
            rejects: self.labels.len() - keeps,
            refs: self.refs.len(),
            refs_from: self.refs_from.clone(),
            active: self.active(),
            trained: if self.active() { self.n } else { 0 },
            strength: if self.active() { self.strength() } else { 0.0 },
            max_strength: MAX_STRENGTH,
            min_per_class: MIN_PER_CLASS,
            half_at: examples_for(MAX_STRENGTH / 2.0),
            full_at: examples_for(MAX_STRENGTH),
        }
    }

    pub fn reset(&mut self) {
        self.labels.clear();
        self.refs.clear();
        self.refs_from.clear();
        self.w.clear();
        self.b = 0.0;
        self.n = 0;
    }

    /// Probability that the user would keep a photo with this (unit-length) embedding.
    pub fn predict(&self, emb: &[f32]) -> Option<f32> {
        if !self.active() || emb.len() != self.w.len() {
            return None;
        }
        let z: f32 = self.b + self.w.iter().zip(emb).map(|(w, x)| w * x * SCALE).sum::<f32>();
        Some(sigmoid(z))
    }

    /// Takes over the weights another copy of these preferences just trained (the labels here may already be newer; the next
    /// retraining catches up on them).
    pub fn adopt_weights(&mut self, trained: &Prefs) {
        self.w = trained.w.clone();
        self.b = trained.b;
        self.n = trained.n;
    }

    /// How strongly the learned taste is blended into a score: grows with evidence, capped.
    pub fn strength(&self) -> f32 {
        (0.08 + self.n as f32 / 300.0).min(MAX_STRENGTH)
    }

    /// Trains on the explicit keep / reject labels and on the reference photos. Reference photos only say what is chosen, so the "not chosen"
    /// examples come from the analysed photos in `pool` (as many as there are references, evenly spread, skipping any that are a reference
    /// themselves); explicit rejects always count.
    pub fn retrain(&mut self, emb_of: impl Fn(&str) -> Option<Vec<f32>>, pool: impl Fn() -> Vec<(String, Vec<f32>)>) {
        let mut data: Vec<(Vec<f32>, f32)> = self
            .labels
            .iter()
            .filter_map(|(k, &keep)| emb_of(k).map(|e| (e, if keep { 1.0 } else { 0.0 })))
            .collect();
        if !self.refs.is_empty() {
            let stems: std::collections::HashSet<&str> = self.refs.iter().map(|r| r.stem.as_str()).collect();
            data.extend(self.refs.iter().map(|r| (r.emb.clone(), 1.0)));
            let dim = self.refs[0].emb.len();
            let others: Vec<Vec<f32>> = pool().into_iter().filter(|(st, e)| e.len() == dim && !stems.contains(st.as_str())).map(|(_, e)| e).collect();
            let want = self.refs.len().max(6).saturating_sub(data.iter().filter(|d| d.1 < 0.5).count());
            if !others.is_empty() && want > 0 {
                let step = (others.len() as f32 / want as f32).max(1.0);
                let mut i = 0f32;
                while (i as usize) < others.len() && data.iter().filter(|d| d.1 < 0.5).count() < self.refs.len().max(6) {
                    data.push((others[i as usize].clone(), 0.0));
                    i += step;
                }
            }
        }
        // Photos analysed by different models have embeddings of different sizes; train on the most common size only.
        let mut sizes: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        data.iter().for_each(|d| *sizes.entry(d.0.len()).or_default() += 1);
        let dim = sizes.into_iter().max_by_key(|(_, n)| *n).map(|(d, _)| d).unwrap_or(0);
        let data: Vec<(Vec<f32>, f32)> = data.into_iter().filter(|d| d.0.len() == dim).collect();
        let pos = data.iter().filter(|d| d.1 > 0.5).count();
        let neg = data.len() - pos;
        if pos < MIN_PER_CLASS || neg < MIN_PER_CLASS {
            self.w.clear();
            self.n = 0;
            return;
        }
        let (mut w, mut b) = (vec![0f32; dim], 0f32);
        // Balanced classes, light L2, plain batch gradient descent: a few hundred labels train in milliseconds.
        let (wp, wn) = (0.5 / pos as f32, 0.5 / neg as f32);
        for _ in 0..400 {
            let (mut gw, mut gb) = (vec![0f32; dim], 0f32);
            for (x, y) in &data {
                let z = b + w.iter().zip(x).map(|(w, x)| w * x * SCALE).sum::<f32>();
                let err = (sigmoid(z) - y) * if *y > 0.5 { wp } else { wn };
                for i in 0..dim {
                    gw[i] += err * x[i] * SCALE;
                }
                gb += err;
            }
            for i in 0..dim {
                w[i] -= 2.0 * (gw[i] + 0.002 * w[i]);
            }
            b -= 2.0 * gb;
        }
        self.w = w;
        self.b = b;
        self.n = data.len();
    }
}

/// A person can keep several tastes ("styles"): each has its own labels, reference photos and learned model, and a project chooses which one
/// it is judged by. There is always a default style; everything a project learns goes into it unless the project uses another.
#[derive(Clone, Serialize, Deserialize)]
pub struct Style {
    pub id: String,
    pub name: String,
    pub prefs: Prefs,
}

#[derive(Serialize)]
pub struct StyleInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub info: PrefsInfo,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Styles {
    pub list: Vec<Style>,
    pub default_id: String,
    #[serde(skip)]
    path: PathBuf,
}

pub const DEFAULT_STYLE: &str = "default";

/// A serialised `styles.json` waiting to be written: through a temporary file, synced, then renamed into place.
pub struct PendingSave {
    path: PathBuf,
    bytes: Vec<u8>,
}

impl PendingSave {
    pub fn write(&self) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.path.with_extension("tmp");
        if write_synced(&tmp, &self.bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &self.path);
        }
    }
}

impl Styles {
    /// Loads `styles.json`; the single preferences file of earlier versions becomes the default style.
    pub fn load(dir: &std::path::Path) -> Self {
        let path = dir.join("styles.json");
        let mut s: Styles = read_or_quarantine(&path);
        s.path = path;
        if s.list.is_empty() {
            let old = Prefs::load(dir.join("preferences.json"));
            s.list.push(Style { id: DEFAULT_STYLE.into(), name: String::new(), prefs: old });
            s.default_id = DEFAULT_STYLE.into();
            s.save();
        }
        if !s.list.iter().any(|x| x.id == s.default_id) {
            s.default_id = s.list[0].id.clone();
        }
        s
    }

    pub fn save(&self) {
        if let Some(pending) = self.to_save() {
            pending.write();
        }
    }

    /// The file as it would be saved now, to be written once the lock on these styles is released (the write can take a while with
    /// thousands of reference vectors, and the settings page reads through the same lock).
    pub fn to_save(&self) -> Option<PendingSave> {
        serde_json::to_vec(self).ok().map(|bytes| PendingSave { path: self.path.clone(), bytes })
    }

    fn index(&self, id: Option<&str>) -> usize {
        let want = id.filter(|i| !i.is_empty()).unwrap_or(&self.default_id);
        self.list.iter().position(|s| s.id == want).or_else(|| self.list.iter().position(|s| s.id == self.default_id)).unwrap_or(0)
    }

    /// The style asked for, or the default one when it is unknown (a deleted style falls back instead of breaking a project).
    pub fn get(&self, id: Option<&str>) -> &Style {
        &self.list[self.index(id)]
    }

    pub fn get_mut(&mut self, id: Option<&str>) -> &mut Style {
        let i = self.index(id);
        &mut self.list[i]
    }

    pub fn infos(&self) -> Vec<StyleInfo> {
        self.list.iter().map(|s| StyleInfo { id: s.id.clone(), name: s.name.clone(), is_default: s.id == self.default_id, info: s.prefs.info() }).collect()
    }

    pub fn create(&mut self, name: &str) -> String {
        let id = format!("s{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
        self.list.push(Style { id: id.clone(), name: name.trim().to_string(), prefs: Prefs::default() });
        id
    }

    pub fn rename(&mut self, id: &str, name: &str) {
        if let Some(s) = self.list.iter_mut().find(|s| s.id == id) {
            s.name = name.trim().to_string();
        }
    }

    /// The default style cannot be deleted.
    pub fn delete(&mut self, id: &str) {
        if id != self.default_id {
            self.list.retain(|s| s.id != id);
        }
    }

    pub fn set_default(&mut self, id: &str) {
        if self.list.iter().any(|s| s.id == id) {
            self.default_id = id.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_marks_match_the_strength_curve() {
        // the settings draw marks at half_at / full_at; they must be where strength() actually gets there
        let mut p = super::Prefs::default();
        let (half, full) = (super::examples_for(super::MAX_STRENGTH / 2.0), super::examples_for(super::MAX_STRENGTH));
        p.n = full;
        assert_eq!(p.strength(), super::MAX_STRENGTH);
        p.n = full - 1;
        assert!(p.strength() < super::MAX_STRENGTH);
        p.n = half;
        assert!(p.strength() >= super::MAX_STRENGTH / 2.0);
        p.n = half - 1;
        assert!(p.strength() < super::MAX_STRENGTH / 2.0);
        assert!(half > 2 * super::MIN_PER_CLASS && full > half);
    }

    use super::*;

    fn unit(v: Vec<f32>) -> Vec<f32> {
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.into_iter().map(|x| x / n).collect()
    }

    #[test]
    fn learns_a_simple_taste() {
        // "keep" photos lean toward axis 0, "reject" toward axis 1, with some noise on other axes.
        let mut store: HashMap<String, Vec<f32>> = HashMap::new();
        let mut p = Prefs::default();
        for i in 0..12 {
            let keep = i % 2 == 0;
            let mut v = vec![0.1f32; 16];
            v[if keep { 0 } else { 1 }] = 1.0;
            v[2 + (i % 5)] += 0.15 * (i as f32);
            store.insert(format!("p{i}"), unit(v));
            p.labels.insert(format!("p{i}"), keep);
        }
        p.retrain(|k| store.get(k).cloned(), Vec::new);
        assert!(p.active());
        let mut keep_probe = vec![0.1f32; 16];
        keep_probe[0] = 1.0;
        let mut rej_probe = vec![0.1f32; 16];
        rej_probe[1] = 1.0;
        let (a, b) = (p.predict(&unit(keep_probe)).unwrap(), p.predict(&unit(rej_probe)).unwrap());
        println!("keep-like={a:.2} reject-like={b:.2}");
        assert!(a > 0.7 && b < 0.3);
    }

    #[test]
    fn stays_off_without_both_classes() {
        let mut p = Prefs::default();
        for i in 0..10 {
            p.labels.insert(format!("p{i}"), true);
        }
        p.retrain(|_| Some(vec![0.5; 4]), Vec::new);
        assert!(!p.active());
    }
}

#[cfg(test)]
mod style_tests {
    use super::*;

    fn unit(v: Vec<f32>) -> Vec<f32> {
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.into_iter().map(|x| x / n).collect()
    }

    #[test]
    fn a_styles_file_that_cannot_be_read_is_set_aside_not_replaced() {
        let dir = std::env::temp_dir().join(format!("fg_styles_corrupt_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("styles.json"), b"{ this is not json").unwrap();
        let s = Styles::load(&dir);
        assert_eq!(s.get(None).prefs.labels.len(), 0, "starts from the defaults");
        let names: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        let aside = names.iter().find(|n| n.starts_with("styles.json.corrupt-")).expect("the unreadable file is kept under another name");
        assert_eq!(std::fs::read(dir.join(aside)).unwrap(), b"{ this is not json", "byte for byte");
        if let Ok(b) = std::fs::read(dir.join("styles.json")) {
            assert!(serde_json::from_slice::<serde_json::Value>(&b).is_ok(), "whatever sits there now is a fresh, readable file");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn references_teach_a_taste_without_any_labels() {
        let mut p = Prefs::default();
        let mut pool = Vec::new();
        for i in 0..20 {
            let mut chosen = vec![0.1f32; 12];
            chosen[0] = 1.0;
            chosen[3 + (i % 4)] += 0.1 * i as f32;
            p.refs.push(Reference { stem: format!("pick{i}"), emb: unit(chosen) });
            let mut other = vec![0.1f32; 12];
            other[1] = 1.0;
            other[3 + (i % 4)] += 0.1 * i as f32;
            pool.push((format!("shot{i}"), unit(other)));
        }
        p.retrain(|_| None, || pool.clone());
        assert!(p.active(), "20 references and the analysed photos are enough to start");
        let mut like = vec![0.1f32; 12];
        like[0] = 1.0;
        let mut unlike = vec![0.1f32; 12];
        unlike[1] = 1.0;
        assert!(p.predict(&unit(like)).unwrap() > 0.65);
        assert!(p.predict(&unit(unlike)).unwrap() < 0.35);
    }

    #[test]
    fn a_reference_that_is_also_in_the_pool_is_not_a_negative() {
        let mut p = Prefs::default();
        for i in 0..8 {
            p.refs.push(Reference { stem: format!("a{i}"), emb: vec![1.0, 0.0] });
        }
        // the whole pool is the same files as the references: nothing is left to serve as "not chosen"
        let pool: Vec<(String, Vec<f32>)> = (0..8).map(|i| (format!("a{i}"), vec![1.0, 0.0])).collect();
        p.retrain(|_| None, || pool.clone());
        assert!(!p.active());
    }

    #[test]
    fn styles_migrate_and_fall_back() {
        let dir = std::env::temp_dir().join(format!("fg-styles-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut old = Prefs::default();
        old.labels.insert("x.jpg".into(), true);
        std::fs::write(dir.join("preferences.json"), serde_json::to_vec(&old).unwrap()).unwrap();
        let mut s = Styles::load(&dir);
        assert_eq!(s.list.len(), 1);
        assert_eq!(s.get(None).prefs.labels.len(), 1, "the old preferences became the default style");
        let id = s.create("Street");
        s.get_mut(Some(&id)).prefs.labels.insert("y.jpg".into(), false);
        assert_eq!(s.get(Some(&id)).prefs.labels.len(), 1);
        assert_eq!(s.get(None).prefs.labels.len(), 1);
        s.save();
        let s2 = Styles::load(&dir);
        assert_eq!(s2.list.len(), 2);
        // deleting a style: projects that used it fall back to the default
        let mut s3 = s2;
        s3.delete(&id);
        assert_eq!(s3.get(Some(&id)).id, DEFAULT_STYLE);
        s3.delete(DEFAULT_STYLE);
        assert_eq!(s3.list.len(), 1, "the default style cannot be deleted");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// On real photos: teach the taste with some of the red-backdrop studio portraits as "past picks" and check that the held-out ones
    /// (never shown to the learner) come out as more likely to be kept than the average photo. FG_CACHE = the analysis cache.
    #[test]
    #[ignore = "needs a real analysis cache (FG_CACHE)"]
    fn references_generalise_on_real_photos() {
        let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(std::env::var("FG_CACHE").expect("FG_CACHE")).unwrap()).unwrap();
        let mut all: Vec<(String, Vec<f32>, bool)> = Vec::new();
        for (_, e) in raw["entries"].as_object().unwrap() {
            let name = e["photo"]["name"].as_str().unwrap().to_lowercase();
            let emb: Vec<f32> = serde_json::from_value(e["emb"].clone()).unwrap();
            // the red-backdrop portraits are the photos named by this list (found by eye on the contact sheets)
            let red = ["f6eeffd8e7ede29e96cf3b4c993b403d_big.jpg", "ab0ffdf43aa422705b831c59a518c429_big.jpg", "a688262c28c17221ba74c1cc2d2336aa_big.jpg"].iter().any(|n| name == *n)
                || name.starts_with("mmexport17796");
            all.push((name, emb, red));
        }
        let reds: Vec<&(String, Vec<f32>, bool)> = all.iter().filter(|x| x.2).collect();
        println!("{} red-backdrop portraits found", reds.len());
        assert!(reds.len() >= 6);
        let (teach, held) = reds.split_at(reds.len() / 2);
        let mut p = Prefs::default();
        for t in teach { p.refs.push(Reference { stem: t.0.clone(), emb: t.1.clone() }); }
        let pool: Vec<(String, Vec<f32>)> = all.iter().map(|x| (x.0.clone(), x.1.clone())).collect();
        p.retrain(|_| None, || pool.clone());
        println!("active: {} (trained on {})", p.active(), p.n);
        assert!(p.active());
        let held_avg = held.iter().filter_map(|h| p.predict(&h.1)).sum::<f32>() / held.len() as f32;
        let rest: Vec<f32> = all.iter().filter(|x| !x.2).filter_map(|x| p.predict(&x.1)).collect();
        let rest_avg = rest.iter().sum::<f32>() / rest.len() as f32;
        println!("held-out red portraits: {held_avg:.2}   all other photos: {rest_avg:.2}");
        assert!(held_avg > rest_avg + 0.15);
    }
}
