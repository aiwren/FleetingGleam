//! Sealed model files. The CLIP encoders and the aesthetic heads are the most valuable part of the app, so a release ships them
//! encrypted (AES-256-GCM, in 4 MiB chunks) and only ever decrypts them into memory: no plaintext copy is written to disk.
//!
//! This makes the files useless to anyone who just copies them out of the app bundle. It does not, and cannot, stop someone who
//! debugs the running process: the key has to exist in memory to use the model. See docs/PROTECTION.md for the honest picture.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;

include!(concat!(env!("OUT_DIR"), "/seal_key.rs"));

const MAGIC: &[u8; 4] = b"FGM1";
const CHUNK: usize = 4 * 1024 * 1024;
const TAG: usize = 16;
pub const EXT: &str = "fgm";

fn cipher() -> Aes256Gcm {
    let mut k = [0u8; 32];
    for i in 0..32 {
        k[i] = SHARE_A[i] ^ SHARE_B[i];
    }
    Aes256Gcm::new_from_slice(&k).expect("32-byte key")
}

fn nonce(base: &[u8; 12], index: u32) -> [u8; 12] {
    let mut n = *base;
    for (i, b) in index.to_le_bytes().iter().enumerate() {
        n[8 + i] ^= b;
    }
    n
}

fn aad(index: u32, total: u64) -> Vec<u8> {
    let mut a = index.to_le_bytes().to_vec();
    a.extend_from_slice(&total.to_le_bytes());
    a
}

/// Encrypts `src` into `dst` (used by the `seal_models` tool at release time).
pub fn seal_file(src: &Path, dst: &Path, base_nonce: [u8; 12]) -> Result<(), String> {
    let c = cipher();
    let mut input = std::fs::File::open(src).map_err(|e| format!("{}: {e}", src.display()))?;
    let total = input.metadata().map_err(|e| e.to_string())?.len();
    let mut out = std::io::BufWriter::new(std::fs::File::create(dst).map_err(|e| e.to_string())?);
    out.write_all(MAGIC).and_then(|_| out.write_all(&base_nonce)).and_then(|_| out.write_all(&total.to_le_bytes())).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; CHUNK];
    let mut index = 0u32;
    loop {
        let mut filled = 0;
        while filled < CHUNK {
            let n = input.read(&mut buf[filled..]).map_err(|e| e.to_string())?;
            if n == 0 { break; }
            filled += n;
        }
        if filled == 0 { break; }
        let ct = c.encrypt(Nonce::from_slice(&nonce(&base_nonce, index)), Payload { msg: &buf[..filled], aad: &aad(index, total) }).map_err(|e| e.to_string())?;
        out.write_all(&ct).map_err(|e| e.to_string())?;
        index += 1;
        if filled < CHUNK { break; }
    }
    out.flush().map_err(|e| e.to_string())
}

/// Decrypts a sealed file entirely in memory. Any change to the file (or a wrong key) makes it fail rather than return garbage.
pub fn open_file(path: &Path) -> Result<Vec<u8>, String> {
    let c = cipher();
    let mut f = std::io::BufReader::new(std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?);
    let mut head = [0u8; 4 + 12 + 8];
    f.read_exact(&mut head).map_err(|e| e.to_string())?;
    if &head[..4] != MAGIC {
        return Err("not a sealed model".into());
    }
    let base: [u8; 12] = head[4..16].try_into().unwrap();
    let total = u64::from_le_bytes(head[16..24].try_into().unwrap());
    let mut plain = Vec::with_capacity(total as usize);
    let mut buf = vec![0u8; CHUNK + TAG];
    let mut index = 0u32;
    while (plain.len() as u64) < total {
        let want = ((total - plain.len() as u64) as usize).min(CHUNK) + TAG;
        f.read_exact(&mut buf[..want]).map_err(|e| e.to_string())?;
        let pt = c.decrypt(Nonce::from_slice(&nonce(&base, index)), Payload { msg: &buf[..want], aad: &aad(index, total) }).map_err(|_| "model file failed verification".to_string())?;
        plain.extend_from_slice(&pt);
        index += 1;
    }
    Ok(plain)
}

fn sealed_path(dir: &Path, name: &str) -> std::path::PathBuf {
    dir.join(format!("{name}.{EXT}"))
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

/// A model counts as present if either its sealed or its plain (development) form exists.
pub fn has_model(dir: &Path, name: &str) -> bool {
    sealed_path(dir, name).exists() || dir.join(name).exists()
}

pub fn is_sealed(dir: &Path, name: &str) -> bool {
    sealed_path(dir, name).exists()
}

/// The bytes of a model file, decrypted if it is sealed.
pub fn read_model(dir: &Path, name: &str) -> Result<Vec<u8>, String> {
    if is_sealed(dir, name) { open_file(&sealed_path(dir, name)) } else { std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_roundtrip_and_tamper() {
        let dir = std::env::temp_dir().join(format!("fg-seal-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // 9 MiB + a bit: several chunks and a short last one
        let data: Vec<u8> = (0..(9 * 1024 * 1024 + 123)).map(|i| (i * 31 % 251) as u8).collect();
        std::fs::write(dir.join("m.onnx"), &data).unwrap();
        seal_file(&dir.join("m.onnx"), &dir.join("m.onnx.fgm"), [7u8; 12]).unwrap();
        assert!(std::fs::read(dir.join("m.onnx.fgm")).unwrap().windows(64).all(|w| w != &data[1000..1064]), "plaintext leaked into the sealed file");
        assert_eq!(open_file(&dir.join("m.onnx.fgm")).unwrap(), data);
        // flip one byte in the middle: opening must fail
        let mut sealed = std::fs::read(dir.join("m.onnx.fgm")).unwrap();
        let mid = sealed.len() / 2;
        sealed[mid] ^= 1;
        std::fs::write(dir.join("t.fgm"), &sealed).unwrap();
        assert!(open_file(&dir.join("t.fgm")).is_err());
        // an empty file also round-trips
        std::fs::write(dir.join("e.json"), b"").unwrap();
        seal_file(&dir.join("e.json"), &dir.join("e.json.fgm"), [1u8; 12]).unwrap();
        assert!(open_file(&dir.join("e.json.fgm")).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
