//! Where the model files are and how to read them. Most are downloaded into the app's data folder when first needed (modelpacks.rs).
//! Small ones ship inside the app, from `model-data/`: weights we converted ourselves that exist nowhere else, and every pack whose
//! files add up to less than 10 MB（作者 2026-10-22：10 MB 以下的模型直接放进应用，不让用户为几 MB 再等一次下载）。

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
    // 整包不到 10 MB 的：眼睛（MediaPipe 网格与表情，6.8 MB）、人脸检测（UltraFace RFB-640，1.6 MB）
    "face_landmarks.onnx",
    "face_blendshapes.onnx",
    "face_detector_rfb640.onnx",
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

    /// 随应用的 ONNX 模型不用下载也能建会话。
    #[test]
    fn bundled_onnx_models_load_without_a_folder() {
        let nowhere = Path::new("/nonexistent-model-dir");
        assert!(crate::mediapipe::available(nowhere));
        assert!(crate::facepipe::available(nowhere));
        crate::mediapipe::MediapipeEyes::load(nowhere).expect("eye models from inside the app");
        crate::facepipe::FaceDetector::load(nowhere).expect("face detector from inside the app");
        for f in ["face_landmarks.onnx", "face_blendshapes.onnx", "face_detector_rfb640.onnx"] {
            let want = crate::modelpacks::manifest_sha(f).unwrap();
            assert_eq!(sha256_hex(bundled(f).unwrap()), want, "{f} matches the manifest");
        }
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
