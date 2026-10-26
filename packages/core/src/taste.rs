//! "My settings and taste" as one portable file: the app settings and every style with its learned taste, so a person can carry
//! their taste to another computer (or to the Android app) without an account. What goes in is embeddings, never photos or paths:
//! each labelled photo becomes its embedding plus keep / reject, each reference photo its embedding. Which embedding model made a
//! vector is told by its size, so the importer trains only on vectors the local model can produce.

use serde::{Deserialize, Serialize};

use crate::prefs::{Example, Prefs, Reference, Styles};

pub const FORMAT: &str = "fleetinggleam-taste/1";

/// The embedding model a vector came from, by its size.
pub fn model_of(dim: usize) -> &'static str {
    match dim {
        512 => "clip-b32",
        768 => "clip-l14",
        1152 => "siglip-so400m",
        _ => "unknown",
    }
}

#[derive(Serialize, Deserialize)]
pub struct Pack {
    pub format: String,
    pub exported_at: u64,
    pub app_version: String,
    /// 哪台设备做的这份（备份恢复前显示；老文件没有）。
    #[serde(default)]
    pub device: String,
    /// "mac" / "android" / …
    #[serde(default)]
    pub platform: String,
    /// The frontend's settings object, carried as it is; the importer picks what makes sense on the other machine.
    pub settings: serde_json::Value,
    pub styles: Vec<PackedStyle>,
}

#[derive(Serialize, Deserialize)]
pub struct PackedStyle {
    pub name: String,
    pub is_default: bool,
    /// What the examples below are: labelled photos as their embedding plus the decision, references as their embedding.
    pub examples: Vec<PackedExample>,
    pub refs: Vec<PackedRef>,
    pub refs_from: String,
    /// The weights as trained on this machine (usable directly where the same model runs; otherwise the examples retrain).
    pub weights: Option<PackedWeights>,
}

#[derive(Serialize, Deserialize)]
pub struct PackedExample {
    pub model: String,
    #[serde(with = "crate::cache::packed")]
    pub emb: Vec<f32>,
    pub keep: bool,
}

#[derive(Serialize, Deserialize)]
pub struct PackedRef {
    pub model: String,
    pub stem: String,
    #[serde(with = "crate::cache::packed")]
    pub emb: Vec<f32>,
}

#[derive(Serialize, Deserialize)]
pub struct PackedWeights {
    pub model: String,
    #[serde(with = "crate::cache::packed")]
    pub w: Vec<f32>,
    pub b: f32,
    pub n: usize,
}

#[derive(Serialize, Default, Debug, PartialEq)]
pub struct Summary {
    pub styles: usize,
    pub examples: usize,
    pub refs: usize,
    /// Labelled photos whose embedding was not in the cache (never analysed here, or analysed before the cache kept vectors).
    pub skipped: usize,
}

/// Builds the file from the styles and the embeddings the cache has for their labelled photos.
pub fn pack(styles: &Styles, settings: serde_json::Value, app_version: &str, device: &str, platform: &str, emb_of: impl Fn(&str) -> Option<Vec<f32>>) -> (Pack, Summary) {
    let mut sum = Summary::default();
    let mut out = Vec::new();
    for s in &styles.list {
        let p = &s.prefs;
        let mut examples: Vec<PackedExample> = Vec::new();
        for (path, &keep) in &p.labels {
            match emb_of(path) {
                Some(emb) => examples.push(PackedExample { model: model_of(emb.len()).to_string(), emb, keep }),
                None => sum.skipped += 1,
            }
        }
        examples.extend(p.imported.iter().map(|e| PackedExample { model: model_of(e.emb.len()).to_string(), emb: e.emb.clone(), keep: e.keep }));
        let refs: Vec<PackedRef> = p.refs.iter().map(|r| PackedRef { model: model_of(r.emb.len()).to_string(), stem: r.stem.clone(), emb: r.emb.clone() }).collect();
        sum.examples += examples.len();
        sum.refs += refs.len();
        out.push(PackedStyle {
            name: s.name.clone(),
            is_default: s.id == styles.default_id,
            examples,
            refs,
            refs_from: p.refs_from.clone(),
            weights: p.weights().map(|(w, b, n)| PackedWeights { model: model_of(w.len()).to_string(), w: w.to_vec(), b, n }),
        });
    }
    sum.styles = out.len();
    let exported_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    (Pack { format: FORMAT.to_string(), exported_at, app_version: app_version.to_string(), device: device.to_string(), platform: platform.to_string(), settings, styles: out }, sum)
}

/// Merges the file into the styles: a style with the same name takes the examples and references in (without duplicates); a new
/// name becomes a new style. Nothing already here is removed. Weights are not copied: every touched style is retrained by the caller.
pub fn unpack(styles: &mut Styles, pack: &Pack) -> Result<(Summary, Vec<String>), String> {
    if pack.format != FORMAT {
        return Err(format!("not a FleetingGleam taste file ({})", pack.format));
    }
    let mut sum = Summary::default();
    let mut touched = Vec::new();
    for ps in &pack.styles {
        let id = match styles.list.iter().find(|s| s.name.trim() == ps.name.trim()).map(|s| s.id.clone()) {
            Some(id) => id,
            None => {
                sum.styles += 1;
                if ps.name.trim().is_empty() { styles.default_id.clone() } else { styles.create(&ps.name) }
            }
        };
        let p: &mut Prefs = &mut styles.get_mut(Some(&id)).prefs;
        for e in &ps.examples {
            if e.emb.is_empty() || p.imported.iter().any(|x| x.keep == e.keep && x.emb == e.emb) {
                continue;
            }
            p.imported.push(Example { emb: e.emb.clone(), keep: e.keep });
            sum.examples += 1;
        }
        for r in &ps.refs {
            if r.emb.is_empty() || p.refs.iter().any(|x| x.emb == r.emb) {
                continue;
            }
            p.refs.push(Reference { stem: r.stem.clone(), emb: r.emb.clone() });
            sum.refs += 1;
        }
        if p.refs_from.is_empty() && !ps.refs_from.is_empty() {
            p.refs_from = ps.refs_from.clone();
        }
        touched.push(id);
    }
    Ok((sum, touched))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(mut v: Vec<f32>) -> Vec<f32> {
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter_mut().for_each(|x| *x /= n);
        v
    }
    fn vec_at(i: usize, keep: bool) -> Vec<f32> {
        let mut v = vec![0.05f32; 16];
        v[if keep { 0 } else { 1 }] = 1.0;
        v[2 + (i % 8)] += 0.1 * i as f32;
        unit(v)
    }

    #[test]
    fn a_taste_survives_the_trip_to_another_machine_without_any_paths() {
        let dir = std::env::temp_dir().join(format!("fg_taste_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut here = Styles::load(&dir);
        let photos: Vec<(String, Vec<f32>, bool)> = (0..12).map(|i| (format!("/Users/me/Photos/img{i}.jpg"), vec_at(i, i % 2 == 0), i % 2 == 0)).collect();
        for (path, _, keep) in &photos {
            here.get_mut(None).prefs.labels.insert(path.clone(), *keep);
        }
        let emb_of = |k: &str| photos.iter().find(|(p, _, _)| p == k).map(|(_, e, _)| e.clone());
        here.get_mut(None).prefs.retrain(0, emb_of, Vec::new);
        assert!(here.get(None).prefs.active());

        let (pack, sum) = pack(&here, serde_json::json!({ "theme": "dark" }), "0.1.0", "test box", "mac", emb_of);
        assert_eq!((sum.styles, sum.examples, sum.skipped), (1, 12, 0));
        let text = serde_json::to_string(&pack).unwrap();
        assert!(!text.contains("/Users/me"), "no path leaves the machine");
        assert_eq!(pack.styles[0].examples[0].model, "unknown", "a 16-dim test vector has no known model; real ones say clip-b32 etc.");

        let there_dir = dir.join("there");
        std::fs::create_dir_all(&there_dir).unwrap();
        let mut there = Styles::load(&there_dir);
        let pack2: Pack = serde_json::from_str(&text).unwrap();
        let (sum2, touched) = unpack(&mut there, &pack2).unwrap();
        assert_eq!((sum2.styles, sum2.examples), (0, 12), "the nameless default style merges into the default style here");
        for id in &touched {
            there.get_mut(Some(id)).prefs.retrain(0, |_| None, Vec::new);
        }
        let p = there.get(None).prefs.clone();
        assert!(p.active(), "trained from the imported examples alone, with no photo analysed here");
        assert!(p.predict(&vec_at(20, true)).unwrap() > p.predict(&vec_at(21, false)).unwrap());
        assert_eq!(p.info().imported, 12);

        let (sum3, _) = unpack(&mut there, &pack2).unwrap();
        assert_eq!(sum3.examples, 0, "importing the same file twice adds nothing");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_of_another_kind_is_refused() {
        let pack = Pack { format: "something-else/9".into(), exported_at: 0, app_version: String::new(), device: String::new(), platform: String::new(), settings: serde_json::Value::Null, styles: Vec::new() };
        let mut s = Styles::load(&std::env::temp_dir().join("fg_taste_refuse"));
        assert!(unpack(&mut s, &pack).is_err());
    }
}
