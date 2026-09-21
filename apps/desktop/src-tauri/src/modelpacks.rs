//! Models are downloaded when they are first needed, one pack at a time, not shipped inside the app and not fetched all at once.
//! What is downloaded is the *sealed* form (see seal.rs); it is checked against the SHA-256 in the server's manifest and only then
//! moved into place, so a broken or tampered download can never be used. Photos are never sent anywhere: this is the only
//! network traffic the app makes, and it only ever fetches these files.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::seal;

/// Where the packs live on the server, set when the app is built (`FG_MODEL_BASE`) or, for testing, when it runs.
fn base_url() -> Option<String> {
    std::env::var("FG_MODEL_BASE").ok().or_else(|| option_env!("FG_MODEL_BASE").map(str::to_string)).map(|s| s.trim_end_matches('/').to_string())
}

pub struct Pack {
    pub id: &'static str,
    pub files: &'static [&'static str],
}

pub const PACKS: [Pack; 4] = [
    Pack { id: "standard", files: &["clip_vit_b32_vision_fp16.onnx", "clip_vit_b32_vision_int8.onnx", "aesthetic_head_vit_b_32.json"] },
    Pack { id: "high", files: &["clip_vit_l14_vision_fp16.onnx", "aesthetic_head_vit_l_14.json"] },
    Pack { id: "ultra", files: &["siglip_so400m_vision_fp16.onnx", "aesthetic_v25_head.json"] },
    Pack { id: "search", files: &["text_multilingual_qint8.onnx", "text_multilingual_tokenizer.json", "text_multilingual_dense.bin"] },
];

pub fn pack(id: &str) -> Option<&'static Pack> {
    PACKS.iter().find(|p| p.id == id)
}

#[derive(Serialize)]
pub struct PackStatus {
    pub id: String,
    pub installed: bool,
    /// Bytes still to download (known once the manifest was read; otherwise 0).
    pub bytes: u64,
}

pub fn installed(dir: &Path, p: &Pack) -> bool {
    p.files.iter().all(|f| seal::has_model(dir, f))
}

#[derive(Deserialize)]
struct FileInfo {
    size: u64,
    sha256: String,
}
#[derive(Deserialize)]
struct Manifest {
    files: std::collections::HashMap<String, FileInfo>,
}

#[derive(Serialize, Clone)]
pub struct Progress {
    pub pack: String,
    pub file: String,
    pub done: u64,
    pub total: u64,
}

fn get(url: &str) -> Result<ureq::Response, String> {
    ureq::AgentBuilder::new().timeout_connect(std::time::Duration::from_secs(15)).timeout_read(std::time::Duration::from_secs(30)).build().get(url).call().map_err(|e| format!("download failed: {e}"))
}

fn manifest(base: &str) -> Result<Manifest, String> {
    let mut text = String::new();
    get(&format!("{base}/manifest.json"))?.into_reader().take(4 << 20).read_to_string(&mut text).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("the model manifest is not valid: {e}"))
}

pub fn manifest_sizes(id: &str) -> Result<u64, String> {
    let p = pack(id).ok_or("unknown pack")?;
    let base = base_url().ok_or("the model server is not configured")?;
    let m = manifest(&base)?;
    Ok(p.files.iter().filter_map(|f| m.files.get(&format!("{f}.{}", seal::EXT))).map(|i| i.size).sum())
}

/// Downloads what a pack is missing into `dir`. `cancel` stops it between chunks; half-finished files are removed.
pub fn download(dir: &Path, id: &str, cancel: &AtomicBool, on_progress: &dyn Fn(Progress)) -> Result<(), String> {
    let p = pack(id).ok_or("unknown pack")?;
    let base = base_url().ok_or("the model server is not configured")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let m = manifest(&base)?;
    let todo: Vec<(&str, &FileInfo)> = p
        .files
        .iter()
        .filter(|f| !seal::has_model(dir, f))
        .map(|f| m.files.get(&format!("{f}.{}", seal::EXT)).map(|i| (*f, i)).ok_or_else(|| format!("{f} is not on the server")))
        .collect::<Result<_, _>>()?;
    let total: u64 = todo.iter().map(|(_, i)| i.size).sum();
    let mut done_before = 0u64;
    for (name, info) in todo {
        let file = format!("{name}.{}", seal::EXT);
        let part: PathBuf = dir.join(format!("{file}.part"));
        let result = (|| -> Result<(), String> {
            let resp = get(&format!("{base}/{file}"))?;
            let mut reader = resp.into_reader();
            let mut out = std::io::BufWriter::new(std::fs::File::create(&part).map_err(|e| e.to_string())?);
            let mut buf = vec![0u8; 256 * 1024];
            let mut got = 0u64;
            let mut last = std::time::Instant::now();
            loop {
                if cancel.load(Ordering::Relaxed) { return Err("cancelled".into()); }
                let n = reader.read(&mut buf).map_err(|e| format!("download interrupted: {e}"))?;
                if n == 0 { break; }
                out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                got += n as u64;
                if last.elapsed().as_millis() > 120 {
                    last = std::time::Instant::now();
                    on_progress(Progress { pack: id.into(), file: name.into(), done: done_before + got, total });
                }
            }
            out.flush().map_err(|e| e.to_string())?;
            drop(out);
            if got != info.size { return Err(format!("{name}: incomplete download")); }
            if seal::sha256_file(&part)? != info.sha256.to_lowercase() { return Err(format!("{name}: the file did not match its checksum")); }
            std::fs::rename(&part, dir.join(&file)).map_err(|e| e.to_string())
        })();
        if let Err(e) = result {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
        done_before += info.size;
        on_progress(Progress { pack: id.into(), file: name.into(), done: done_before, total });
    }
    Ok(())
}

pub fn remove(dir: &Path, id: &str) -> Result<(), String> {
    let p = pack(id).ok_or("unknown pack")?;
    for f in p.files {
        let _ = std::fs::remove_file(dir.join(format!("{f}.{}", seal::EXT)));
        let _ = std::fs::remove_file(dir.join(f));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End to end against a local server: FG_MODEL_BASE=http://127.0.0.1:8765 (a folder made by the seal_models tool).
    #[test]
    #[ignore = "needs a local model server and FG_MODEL_BASE"]
    fn downloads_verifies_and_loads() {
        let dir = std::env::temp_dir().join(format!("fg-packs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cancel = AtomicBool::new(false);
        let seen = std::sync::Mutex::new(0u64);
        let size = manifest_sizes("search").unwrap();
        println!("search pack: {size} bytes");
        download(&dir, "search", &cancel, &|p| { *seen.lock().unwrap() = p.done; }).unwrap();
        assert!(installed(&dir, pack("search").unwrap()));
        assert_eq!(*seen.lock().unwrap(), size);
        // nothing to do the second time
        download(&dir, "search", &cancel, &|_| panic!("should not download again")).unwrap();
        // the downloaded (sealed) text model works
        let enc = crate::textsearch::TextEncoder::load(&dir).unwrap();
        let v = enc.encode("烟花").unwrap();
        assert_eq!(v.len(), 512);
        // a corrupted file is rejected
        remove(&dir, "search").unwrap();
        cancel.store(true, Ordering::Relaxed);
        assert!(download(&dir, "search", &cancel, &|_| {}).is_err());
        assert!(!installed(&dir, pack("search").unwrap()), "a cancelled download leaves nothing behind");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
