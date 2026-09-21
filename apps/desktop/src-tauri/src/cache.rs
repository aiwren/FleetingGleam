//! Persistent per-photo analysis cache. Re-opening a project, adding a folder or re-scoring never
//! re-analyses a file whose size and modification time are unchanged and whose thumbnail still exists.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::analyze::Photo;

#[derive(Serialize, Deserialize)]
struct Entry {
    /// Analysis version + model; entries made by a different one are ignored and redone.
    #[serde(default)]
    tag: String,
    size: u64,
    mtime: i64,
    photo: Photo,
    hash: u64,
    emb: Vec<f32>,
    /// CLIP ViT-B/32 embedding used for search (512 numbers). Filled by the search index for photos analysed with the larger model.
    #[serde(default)]
    sem: Vec<f32>,
    /// Colour signature (see colorsig.rs), made together with the search vector.
    #[serde(default)]
    color: Vec<u8>,
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

    /// A cached photo, only if the file is unchanged and its thumbnail is still on disk.
    pub fn get(&self, key: &str, size: u64, mtime: i64, tag: &str) -> Option<Photo> {
        let e = self.entries.get(key)?;
        if e.tag != tag || e.size != size || e.mtime != mtime || !Path::new(&e.photo.thumb).exists() {
            return None;
        }
        let mut p = e.photo.clone();
        p.hash = e.hash;
        p.embedding = e.emb.clone();
        Some(p)
    }

    pub fn put(&mut self, size: u64, mtime: i64, tag: &str, p: &Photo) {
        // a re-analysis of an unchanged file keeps the search embedding it already has
        let (sem, color) = self.entries.get(&p.path).filter(|e| e.size == size && e.mtime == mtime).map(|e| (e.sem.clone(), e.color.clone())).unwrap_or_default();
        self.entries.insert(
            p.path.clone(),
            Entry { tag: tag.to_string(), size, mtime, photo: p.clone(), hash: p.hash, emb: p.embedding.clone(), sem, color },
        );
    }

    /// The vector to search with: the dedicated one, or the analysis embedding itself when that already is ViT-B/32 (512 numbers).
    pub fn semantic(&self, key: &str) -> Option<Vec<f32>> {
        let e = self.entries.get(key)?;
        if !e.sem.is_empty() { Some(e.sem.clone()) } else if e.emb.len() == 512 { Some(e.emb.clone()) } else { None }
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

    /// Every analysed photo's file stem and embedding: the "typical photo" the learned taste is compared against.
    pub fn pool(&self) -> Vec<(String, Vec<f32>)> {
        self.entries
            .values()
            .filter(|e| !e.emb.is_empty())
            .map(|e| (Path::new(&e.photo.name).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default(), e.emb.clone()))
            .collect()
    }

    pub fn embedding(&self, key: &str) -> Option<Vec<f32>> {
        self.entries.get(key).map(|e| e.emb.clone()).filter(|v| !v.is_empty())
    }

    pub fn save(&self) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.path.with_extension("tmp");
        if let Ok(bytes) = serde_json::to_vec(self) {
            if std::fs::write(&tmp, bytes).is_ok() {
                let _ = std::fs::rename(&tmp, &self.path);
            }
        }
    }
}
