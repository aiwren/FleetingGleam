//! Persistent per-photo analysis cache. Re-opening a project, adding a folder or re-scoring never
//! re-analyses a file whose size and modification time are unchanged and whose thumbnail still exists.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::analyze::Photo;

/// The float vectors of an entry (CLIP, search, faces) are stored as text of raw bytes (base64) instead of a JSON array of numbers: a third of
/// the size and several times faster to write and read, which matters at tens of thousands of photos. Files written the old way still load.
pub(crate) mod packed {
    use serde::de::{self, Deserializer, SeqAccess, Visitor};
    use serde::ser::{SerializeSeq, Serializer};
    use std::fmt;

    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    fn encode(v: &[f32]) -> String {
        let mut bytes = Vec::with_capacity(v.len() * 4);
        for x in v {
            bytes.extend_from_slice(&x.to_le_bytes());
        }
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for c in bytes.chunks(3) {
            let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
            out.push(ALPHA[(n >> 18) as usize & 63] as char);
            out.push(ALPHA[(n >> 12) as usize & 63] as char);
            out.push(if c.len() > 1 { ALPHA[(n >> 6) as usize & 63] as char } else { '=' });
            out.push(if c.len() > 2 { ALPHA[n as usize & 63] as char } else { '=' });
        }
        out
    }

    fn decode(s: &str) -> Option<Vec<f32>> {
        let mut val = [255u8; 256];
        for (i, &a) in ALPHA.iter().enumerate() {
            val[a as usize] = i as u8;
        }
        let b = s.as_bytes();
        if b.len() % 4 != 0 {
            return None;
        }
        let mut bytes = Vec::with_capacity(b.len() / 4 * 3);
        for q in b.chunks(4) {
            let pad = q.iter().rev().take_while(|&&c| c == b'=').count();
            let mut n = 0u32;
            for (i, &c) in q.iter().enumerate() {
                let v = if c == b'=' { 0 } else { val[c as usize] };
                if v == 255 {
                    return None;
                }
                n |= (v as u32) << (18 - 6 * i as u32);
            }
            bytes.push((n >> 16) as u8);
            if pad < 2 { bytes.push((n >> 8) as u8); }
            if pad < 1 { bytes.push(n as u8); }
        }
        if bytes.len() % 4 != 0 {
            return None;
        }
        Some(bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
    }

    /// A vector: the packed text, or (old files) the plain array of numbers.
    struct One;
    impl<'de> Visitor<'de> for One {
        type Value = Vec<f32>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result { f.write_str("packed floats or a list of numbers") }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<Vec<f32>, E> { decode(v).ok_or_else(|| E::custom("bad packed vector")) }
        fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Vec<f32>, A::Error> {
            let mut out = Vec::with_capacity(a.size_hint().unwrap_or(0));
            while let Some(x) = a.next_element::<f32>()? { out.push(x); }
            Ok(out)
        }
    }
    struct Elem(Vec<f32>);
    impl<'de> serde::Deserialize<'de> for Elem {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> { d.deserialize_any(One).map(Elem) }
    }
    pub fn serialize<S: Serializer>(v: &Vec<f32>, s: S) -> Result<S::Ok, S::Error> { s.serialize_str(&encode(v)) }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<f32>, D::Error> { d.deserialize_any(One) }

    /// A list of vectors.
    pub mod many {
        use super::*;
        pub fn serialize<S: Serializer>(v: &Vec<Vec<f32>>, s: S) -> Result<S::Ok, S::Error> {
            let mut seq = s.serialize_seq(Some(v.len()))?;
            for x in v { seq.serialize_element(&encode(x))?; }
            seq.end()
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Vec<f32>>, D::Error> {
            struct Many;
            impl<'de> Visitor<'de> for Many {
                type Value = Vec<Vec<f32>>;
                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result { f.write_str("a list of vectors") }
                fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                    let mut out = Vec::with_capacity(a.size_hint().unwrap_or(0));
                    while let Some(Elem(x)) = a.next_element::<Elem>()? { out.push(x); }
                    Ok(out)
                }
            }
            d.deserialize_seq(Many)
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Entry {
    /// Analysis version + model; entries made by a different one are ignored and redone.
    #[serde(default)]
    tag: String,
    size: u64,
    mtime: i64,
    photo: Photo,
    hash: u64,
    #[serde(with = "packed")]
    emb: Vec<f32>,
    /// CLIP ViT-B/32 embedding used for search (512 numbers). Filled by the search index for photos analysed with the larger model.
    #[serde(default, with = "packed")]
    sem: Vec<f32>,
    /// Colour signature (see colorsig.rs), made together with the search vector.
    #[serde(default)]
    color: Vec<u8>,
    /// Face-recognition vectors, one per entry of `photo.face_boxes`.
    #[serde(default, with = "packed::many")]
    fe: Vec<Vec<f32>>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Cache {
    entries: HashMap<String, Entry>,
    #[serde(skip)]
    path: PathBuf,
}

pub fn file_key(path: &Path) -> Option<(u64, i64)> {
    let m = std::fs::metadata(path).ok()?;
    let mtime = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
    Some((m.len(), mtime))
}

impl Cache {
    pub fn load(path: PathBuf) -> Self {
        let mut c: Cache = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        c.path = path;
        c
    }

    /// Whether `get` would answer, without copying the photo (and its vectors) out: for the many places that only need to know.
    pub fn has(&self, key: &str, size: u64, mtime: i64, tag: &str) -> bool {
        self.entries.get(key).map_or(false, |e| e.tag == tag && e.size == size && e.mtime == mtime && Path::new(&e.photo.thumb).exists())
    }

    /// 已知这张照片（文件没变）时它的尺寸：不管是哪一版模型分析的，尺寸都一样，所以不看 tag。
    /// 探测和扫描用它跳过读文件头：读 395 个 20 MB 的 JPEG 头要一两秒，查表只要微秒。
    pub fn dims(&self, key: &str, size: u64, mtime: i64) -> Option<(u32, u32)> {
        let e = self.entries.get(key)?;
        (e.size == size && e.mtime == mtime && e.photo.width > 0 && e.photo.height > 0).then_some((e.photo.width, e.photo.height))
    }

    /// A cached photo, only if the file is unchanged and its thumbnail is still on disk.
    pub fn get(&self, key: &str, size: u64, mtime: i64, tag: &str) -> Option<Photo> {
        let e = self.entries.get(key)?;
        if e.tag != tag || e.size != size || e.mtime != mtime || !Path::new(&e.photo.thumb).exists() {
            return None;
        }
        let mut p = e.photo.clone();
        p.hash = e.hash;
        p.embedding = e.emb.clone();
        p.face_embs = e.fe.clone();
        Some(p)
    }

    pub fn put(&mut self, size: u64, mtime: i64, tag: &str, p: &Photo) {
        // a re-analysis of an unchanged file keeps the search embedding it already has
        let (sem, color) = self.entries.get(&p.path).filter(|e| e.size == size && e.mtime == mtime).map(|e| (e.sem.clone(), e.color.clone())).unwrap_or_default();
        // 文件没变而记录重做了（换了模型、强制重新分析）：用户转过的角度留着
        let rot = self.entries.get(&p.path).filter(|e| e.size == size && e.mtime == mtime).map(|e| e.photo.rotation).unwrap_or(0);
        let turned;
        let p = if rot != 0 && p.rotation == 0 {
            let mut q = p.clone();
            if let Some(dir) = Path::new(&p.thumb).parent().map(Path::to_path_buf) {
                let _ = crate::analyze::apply_rotation(&mut q, rot, &dir);
            }
            turned = q;
            &turned
        } else {
            p
        };
        self.entries.insert(
            p.path.clone(),
            Entry { tag: tag.to_string(), size, mtime, photo: p.clone(), hash: p.hash, emb: p.embedding.clone(), sem, color, fe: p.face_embs.clone() },
        );
    }

    /// The vector to search with: the dedicated one, or the analysis embedding itself when that already is ViT-B/32 (512 numbers).
    pub fn semantic(&self, key: &str) -> Option<&[f32]> {
        let e = self.entries.get(key)?;
        if !e.sem.is_empty() { Some(&e.sem) } else if e.emb.len() == 512 { Some(&e.emb) } else { None }
    }

    pub fn set_index(&mut self, key: &str, size: u64, mtime: i64, sem: Option<Vec<f32>>, color: Vec<u8>) {
        if let Some(e) = self.entries.get_mut(key) {
            if e.size == size && e.mtime == mtime {
                if let Some(v) = sem { e.sem = v; }
                e.color = color;
            }
        }
    }

    pub fn color(&self, key: &str) -> Option<&[u8]> {
        self.entries.get(key).map(|e| e.color.as_slice()).filter(|c| !c.is_empty())
    }

    /// The photo record of an entry (any analysis version), for showing search results.
    pub fn photo(&self, key: &str) -> Option<&Photo> {
        self.entries.get(key).map(|e| &e.photo)
    }

    /// 可改的记录（转照片用）。
    pub fn photo_mut(&mut self, key: &str) -> Option<&mut Photo> {
        self.entries.get_mut(key).map(|e| &mut e.photo)
    }

    /// Entries the search index still has to look at (no search vector or no colour signature), with their thumbnail (a 640 px JPEG:
    /// plenty for CLIP and very cheap to decode) and whether a search vector is needed as well.
    pub fn without_index(&self, keys: &[String]) -> Vec<(String, String, u64, i64, bool)> {
        keys.iter()
            .filter_map(|k| self.entries.get(k).map(|e| (k, e)))
            .filter(|(_, e)| Path::new(&e.photo.thumb).exists())
            .filter_map(|(k, e)| {
                let need_vec = e.sem.is_empty() && e.emb.len() != 512;
                (need_vec || e.color.is_empty()).then(|| (k.clone(), e.photo.thumb.clone(), e.size, e.mtime, need_vec))
            })
            .collect()
    }

    /// A sample of analysed photos (file stem and embedding): the "typical photo" the learned taste is compared against.
    /// At most a couple of thousand, spread over the cache: that is plenty to stand for "typical", and cloning every
    /// embedding of a large cache (tens of MB) on each keypress was what made marking a photo stall.
    pub fn pool(&self) -> Vec<(String, Vec<f32>)> {
        const MAX: usize = 2000;
        let with: Vec<_> = self.entries.values().filter(|e| !e.emb.is_empty()).collect();
        let step = (with.len() as f32 / MAX as f32).max(1.0);
        let mut out = Vec::with_capacity(with.len().min(MAX));
        let mut i = 0f32;
        while (i as usize) < with.len() && out.len() < MAX {
            let e = with[i as usize];
            out.push((Path::new(&e.photo.name).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default(), e.emb.clone()));
            i += step;
        }
        out
    }

    pub fn embedding(&self, key: &str) -> Option<Vec<f32>> {
        self.entries.get(key).map(|e| e.emb.clone()).filter(|v| !v.is_empty())
    }

    pub fn save(&self) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.path.with_extension("tmp");
        // written straight to the file as it is produced (no second copy of a file that can be hundreds of MB in memory)
        if let Ok(f) = std::fs::File::create(&tmp) {
            let mut w = std::io::BufWriter::with_capacity(1 << 20, f);
            // flushed and synced before the rename: a power cut must leave either the old file or the new one, never a torn one
            if serde_json::to_writer(&mut w, self).is_ok() && std::io::Write::flush(&mut w).is_ok() && w.get_ref().sync_all().is_ok() {
                let _ = std::fs::rename(&tmp, &self.path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big(n: usize) -> Cache {
        let mut c = Cache::default();
        let mut seed = 5u64;
        let mut rnd = move || { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); ((seed >> 40) as f32) / ((1u64 << 24) as f32) - 0.5 };
        for i in 0..n {
            let emb: Vec<f32> = (0..768).map(|_| rnd()).collect();
            let sem: Vec<f32> = (0..512).map(|_| rnd()).collect();
            let p = Photo { path: format!("/p/{i}.jpg"), name: format!("{i}.jpg"), thumb: "/tmp".into(), embedding: emb, face_embs: vec![(0..128).map(|_| rnd()).collect()], ..Default::default() };
            c.put(1, 1, "t", &p);
            c.set_index(&p.path, 1, 1, Some(sem), vec![1; 20]);
        }
        c
    }

    #[test]
    fn old_plain_array_files_still_load() {
        let mut c = Cache::default();
        let p = Photo { path: "/a.jpg".into(), name: "a.jpg".into(), thumb: "/t.jpg".into(), embedding: vec![0.5, -1.25, 3.0], face_embs: vec![vec![0.25, 0.5]], ..Default::default() };
        c.put(1, 2, "t", &p);
        c.set_index("/a.jpg", 1, 2, Some(vec![1.0, 2.0]), vec![1]);
        // what the files looked like before: the vectors as plain JSON arrays
        let mut v = serde_json::to_value(&c).unwrap();
        let e = v["entries"]["/a.jpg"].as_object_mut().unwrap();
        e.insert("emb".into(), serde_json::json!([0.5, -1.25, 3.0]));
        e.insert("sem".into(), serde_json::json!([1.0, 2.0]));
        e.insert("fe".into(), serde_json::json!([[0.25, 0.5]]));
        let old: Cache = serde_json::from_value(v).unwrap();
        let o = old.entries.get("/a.jpg").unwrap();
        assert_eq!((o.emb.clone(), o.sem.clone(), o.fe.clone()), (vec![0.5, -1.25, 3.0], vec![1.0, 2.0], vec![vec![0.25, 0.5]]));
        // and what is written now reads back to the same numbers, whatever the length
        let again: Cache = serde_json::from_str(&serde_json::to_string(&old).unwrap()).unwrap();
        let a = again.entries.get("/a.jpg").unwrap();
        assert_eq!((a.emb.clone(), a.sem.clone(), a.fe.clone()), (vec![0.5, -1.25, 3.0], vec![1.0, 2.0], vec![vec![0.25, 0.5]]));
        for len in 0..9usize {
            let v: Vec<f32> = (0..len).map(|i| i as f32 * 0.37 - 1.0).collect();
            let mut c = Cache::default();
            c.put(1, 2, "t", &Photo { path: "/b.jpg".into(), embedding: v.clone(), ..Default::default() });
            let back: Cache = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
            assert_eq!(back.entries.get("/b.jpg").unwrap().emb, v, "length {len}");
        }
    }

    #[test]
    fn what_a_large_cache_costs() {
        let n = 20_000;
        let c = big(n);
        let mut c = c;
        c.path = std::env::temp_dir().join("fg_cache_test.json");
        let t = std::time::Instant::now();
        c.save();
        let saved = t.elapsed();
        let size = std::fs::metadata(&c.path).map(|m| m.len()).unwrap_or(0);
        let t = std::time::Instant::now();
        let back = Cache::load(c.path.clone());
        let loaded = t.elapsed();
        println!("{n} photos: file {} MB, save {:?}, load {:?}", size / 1_000_000, saved, loaded);
        assert_eq!(back.entries.len(), n);
        let a = c.entries.get("/p/777.jpg").unwrap();
        let b = back.entries.get("/p/777.jpg").unwrap();
        assert_eq!(a.emb, b.emb);
        assert_eq!(a.sem, b.sem);
        assert_eq!(a.fe, b.fe);
        let _ = std::fs::remove_file(&c.path);
    }
}
