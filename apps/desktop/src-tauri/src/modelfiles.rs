//! Where the model files are and how to read them. Most are downloaded into the app's data folder when first needed (modelpacks.rs).
//! A few small ones that exist nowhere else (weights we converted ourselves) ship inside the app, from `model-data/`.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

macro_rules! bundled_files {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_bytes!(concat!("../model-data/", $name)) as &[u8])),*]
    };
}

/// Model files that are part of the app itself. `models.manifest.json` lists them without a download address.
static BUNDLED: &[(&str, &[u8])] = bundled_files!(
    "aesthetic_head_vit_b_32.json",
    "aesthetic_head_vit_l_14.json",
    "aesthetic_v25_head.json",
    "text_multilingual_dense.bin",
    "opusmt_tokenizer.json",
);

pub fn bundled(name: &str) -> Option<&'static [u8]> {
    BUNDLED.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Hex SHA-256 of a file, streamed (model files are hundreds of MB).
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// A model counts as present if it is in `dir` or ships inside the app.
pub fn has_model(dir: &Path, name: &str) -> bool {
    dir.join(name).exists() || bundled(name).is_some()
}

/// The bytes of a model file: the copy in `dir` if there is one, otherwise the one inside the app.
pub fn read_model(dir: &Path, name: &str) -> Result<Vec<u8>, String> {
    match std::fs::read(dir.join(name)) {
        Ok(b) => Ok(b),
        Err(e) => bundled(name).map(<[u8]>::to_vec).ok_or_else(|| format!("{name}: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_files_are_readable_without_a_folder() {
        let nowhere = Path::new("/nonexistent-model-dir");
        assert!(has_model(nowhere, "aesthetic_v25_head.json"));
        assert!(!has_model(nowhere, "clip_vit_b32_vision_int8.onnx"));
        let head: serde_json::Value = serde_json::from_slice(&read_model(nowhere, "aesthetic_head_vit_b_32.json").unwrap()).unwrap();
        assert_eq!(head["weight"].as_array().unwrap().len(), 512);
        assert!(read_model(nowhere, "clip_vit_b32_vision_int8.onnx").is_err());
    }

    #[test]
    fn a_copy_in_the_folder_wins() {
        let dir = std::env::temp_dir().join(format!("fg-modelfiles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("aesthetic_v25_head.json"), b"{}").unwrap();
        assert_eq!(read_model(&dir, "aesthetic_v25_head.json").unwrap(), b"{}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
