//! Models are downloaded when they are first needed, one pack at a time, not shipped inside the app and not fetched all at once.
//! Every file has a fixed address, size and SHA-256 in `models.manifest.json`, which is built into the app. Most come straight from
//! the public repositories they were published in (Hugging Face). A download is checked against its SHA-256 and only then moved into
//! place, so a broken or tampered file can never be used. Photos are never sent anywhere: this is the only network traffic the app
//! makes, and it only ever fetches these files.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::modelfiles;

pub struct Pack {
    pub id: &'static str,
    pub files: &'static [&'static str],
}

pub const PACKS: [Pack; 8] = [
    Pack { id: "standard", files: &["clip_vit_b32_vision_fp16.onnx", "clip_vit_b32_vision_int8.onnx", "aesthetic_head_vit_b_32.json"] },
    Pack { id: "high", files: &["clip_vit_l14_vision_fp16.onnx", "aesthetic_head_vit_l_14.json"] },
    Pack { id: "ultra", files: &["siglip_so400m_vision_fp16.onnx", "aesthetic_v25_head.json"] },
    Pack { id: "search", files: &["text_multilingual_qint8.onnx", "text_multilingual_tokenizer.json", "text_multilingual_dense.bin"] },
    Pack { id: "eyes", files: &[crate::mediapipe::LANDMARKS_FILE, crate::mediapipe::BLENDSHAPES_FILE] },
    Pack { id: "faces", files: &[crate::faces::MODEL_FILE] },
    // 没有系统人脸检测的平台（Android）用它找脸，再交给 MediaPipe 的网格模型（`eyes` 包）量眼睛、定五官
    Pack { id: "detect", files: &[crate::facepipe::DETECTOR_FILE] },
    Pack { id: "scenetext", files: &[
        crate::florence::VISION, crate::florence::EMBED, crate::florence::ENCODER, crate::florence::DECODER, crate::florence::TOKENIZER,
        crate::translate::ENCODER, crate::translate::DECODER, crate::translate::TOKENIZER,
    ] },
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
    p.files.iter().all(|f| modelfiles::has_model(dir, f))
}

#[derive(Deserialize)]
struct FileInfo {
    /// None for the files that ship inside the app.
    url: Option<String>,
    size: u64,
    sha256: String,
}

/// The built-in list of every model file: where it comes from, how big it is and what its SHA-256 must be.
fn manifest() -> &'static HashMap<String, FileInfo> {
    static MANIFEST: OnceLock<HashMap<String, FileInfo>> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        #[derive(Deserialize)]
        struct Manifest {
            files: HashMap<String, FileInfo>,
        }
        serde_json::from_str::<Manifest>(include_str!("../models.manifest.json")).expect("models.manifest.json is not valid").files
    })
}

#[derive(Serialize, Clone)]
pub struct Progress {
    pub pack: String,
    pub file: String,
    pub done: u64,
    pub total: u64,
}

fn get(url: &str) -> Result<ureq::Response, String> {
    crate::platform::http_agent(std::time::Duration::from_secs(15), std::time::Duration::from_secs(30)).get(url).call().map_err(|e| format!("download failed: {e}"))
}

/// The files of a pack that still have to be downloaded, with what the manifest says about each.
fn missing<'a>(dir: &Path, p: &'a Pack) -> Result<Vec<(&'a str, &'static FileInfo, &'static str)>, String> {
    let m = manifest();
    p.files
        .iter()
        .filter(|f| !modelfiles::has_model(dir, f))
        .map(|f| {
            let info = m.get(*f).ok_or_else(|| format!("{f} is not in the model list"))?;
            let url = info.url.as_deref().ok_or_else(|| format!("{f} has no download address"))?;
            Ok((*f, info, url))
        })
        .collect()
}

/// How many bytes downloading this pack would take (what is already in `dir` is not counted).
pub fn manifest_sizes(dir: &Path, id: &str) -> Result<u64, String> {
    let p = pack(id).ok_or("unknown pack")?;
    Ok(missing(dir, p)?.iter().map(|(_, i, _)| i.size).sum())
}

/// Downloads what a pack is missing into `dir`. `cancel` stops it between chunks; half-finished files are removed.
pub fn download(dir: &Path, id: &str, cancel: &AtomicBool, on_progress: &dyn Fn(Progress)) -> Result<(), String> {
    let p = pack(id).ok_or("unknown pack")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let todo = missing(dir, p)?;
    let total: u64 = todo.iter().map(|(_, i, _)| i.size).sum();
    let mut done_before = 0u64;
    for (name, info, url) in todo {
        let part: PathBuf = dir.join(format!("{name}.part"));
        let result = (|| -> Result<(), String> {
            let resp = get(url)?;
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
            if modelfiles::sha256_file(&part)? != info.sha256.to_lowercase() { return Err(format!("{name}: the file did not match its checksum")); }
            std::fs::rename(&part, dir.join(name)).map_err(|e| e.to_string())
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
        let _ = std::fs::remove_file(dir.join(f));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_sha256(s: &str) -> bool {
        s.len() == 64 && s.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    }

    /// The built-in list must cover every file of every pack, and nothing else; the files that ship inside the app must match it exactly.
    #[test]
    fn manifest_agrees_with_the_packs_and_the_bundled_files() {
        let m = manifest();
        for p in PACKS.iter() {
            for f in p.files {
                let info = m.get(*f).unwrap_or_else(|| panic!("{f} (pack {}) is not in models.manifest.json", p.id));
                assert!(is_sha256(&info.sha256), "{f}: sha256 must be 64 lowercase hex characters");
                match modelfiles::bundled(f) {
                    Some(bytes) => {
                        assert!(info.url.is_none(), "{f} ships inside the app, so it has no download address");
                        assert_eq!(info.size, bytes.len() as u64, "{f}: size");
                        assert_eq!(info.sha256, modelfiles::sha256_hex(bytes), "{f}: the copy in model-data/ differs from the manifest");
                    }
                    None => {
                        let url = info.url.as_deref().unwrap_or_else(|| panic!("{f} is neither bundled nor has a download address"));
                        assert!(url.starts_with("https://"), "{f}: {url}");
                        assert!(info.size > 0, "{f}: size");
                    }
                }
            }
        }
        for name in m.keys() {
            assert!(PACKS.iter().any(|p| p.files.contains(&name.as_str())), "{name} is in the manifest but in no pack");
        }
    }

    #[test]
    fn sizes_count_only_what_has_to_be_downloaded() {
        let nowhere = Path::new("/nonexistent-model-dir");
        // the search pack: the text model and its tokenizer are downloaded, the projection (dense.bin) ships inside the app
        let m = manifest();
        let want = m["text_multilingual_qint8.onnx"].size + m["text_multilingual_tokenizer.json"].size;
        assert_eq!(manifest_sizes(nowhere, "search").unwrap(), want);
        assert!(manifest_sizes(nowhere, "nope").is_err());
        // nothing left to download once the files are there
        let dir = std::env::temp_dir().join(format!("fg-packs-size-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in pack("search").unwrap().files {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        assert_eq!(manifest_sizes(&dir, "search").unwrap(), 0);
        assert!(installed(&dir, pack("search").unwrap()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// End to end against the real address (38 MB): download, checksum, load the model, nothing to do the second time, cancel leaves nothing behind.
    #[test]
    #[ignore = "downloads 38 MB from huggingface.co"]
    fn downloads_verifies_and_loads() {
        let dir = std::env::temp_dir().join(format!("fg-packs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cancel = AtomicBool::new(false);
        let seen = std::sync::Mutex::new(0u64);
        let size = manifest_sizes(&dir, "faces").unwrap();
        println!("faces pack: {size} bytes");
        download(&dir, "faces", &cancel, &|p| { *seen.lock().unwrap() = p.done; }).unwrap();
        assert!(installed(&dir, pack("faces").unwrap()));
        assert_eq!(*seen.lock().unwrap(), size);
        assert!(std::fs::read_dir(&dir).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".part")));
        // nothing to do the second time
        download(&dir, "faces", &cancel, &|_| panic!("should not download again")).unwrap();
        // the downloaded model works
        crate::faces::FaceEmbedder::load(&dir).unwrap();
        // a cancelled download leaves nothing behind
        remove(&dir, "faces").unwrap();
        cancel.store(true, Ordering::Relaxed);
        assert!(download(&dir, "faces", &cancel, &|_| {}).is_err());
        assert!(!installed(&dir, pack("faces").unwrap()));
        assert!(std::fs::read_dir(&dir).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".part")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
