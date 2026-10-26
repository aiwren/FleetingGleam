//! 本地加密的小秘密仓库：NAS 密码、云备份的密钥。不用系统钥匙串（开发版每次重编译都要重新授权，用户嫌烦），
//! 放在应用数据目录自己的 `secrets.json` 里，每一项用 AES-256-GCM 加密；密钥是首次用到时随机生成的 `secrets.key`，
//! 只有当前用户可读（0600）。这挡得住把数据目录随手拷走、备份工具扫到明文这类情况；拿到同一个账号的人仍然能解，
//! 这是本地应用能做到的程度。纯 Rust，以后 Android 也能直接用。

use std::collections::BTreeMap;
use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// 密钥文件：没有就生成一个。
fn key(dir: &Path) -> Result<[u8; 32], String> {
    let path = dir.join("secrets.key");
    if let Ok(bytes) = std::fs::read(&path) {
        if bytes.len() == 32 {
            let mut k = [0u8; 32];
            k.copy_from_slice(&bytes);
            return Ok(k);
        }
    }
    let mut k = [0u8; 32];
    getrandom::fill(&mut k).map_err(|e| format!("no randomness for the secrets key: {e}"))?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("key.tmp");
    crate::write_synced(&tmp, &k).map_err(|e| format!("{}: {e}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(k)
}

fn load(dir: &Path) -> BTreeMap<String, String> {
    std::fs::read(dir.join("secrets.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(dir: &Path, map: &BTreeMap<String, String>) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(map).map_err(|e| e.to_string())?;
    let path = dir.join("secrets.json");
    let tmp = path.with_extension("json.tmp");
    crate::write_synced(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))
}

/// 存一项（覆盖同名的）。`id` 是调用方自己定的名字，例如 `nas:主机:用户` 或 `backup:目标id`。
pub fn set(dir: &Path, id: &str, value: &str) -> Result<(), String> {
    let k = key(dir)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&k));
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|e| format!("no randomness: {e}"))?;
    // 把 id 一起认证：同一段密文换到别的名字下就解不开
    let sealed = cipher.encrypt(Nonce::from_slice(&nonce), aes_gcm::aead::Payload { msg: value.as_bytes(), aad: id.as_bytes() }).map_err(|_| "could not encrypt the secret".to_string())?;
    let mut blob = nonce.to_vec();
    blob.extend_from_slice(&sealed);
    let mut map = load(dir);
    map.insert(id.to_string(), B64.encode(blob));
    save(dir, &map)
}

/// 读一项；没有、密钥不对或文件被动过都当作没有。
pub fn get(dir: &Path, id: &str) -> Option<String> {
    let blob = B64.decode(load(dir).get(id)?).ok()?;
    if blob.len() < 12 {
        return None;
    }
    let k = key(dir).ok()?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&k));
    let plain = cipher.decrypt(Nonce::from_slice(&blob[..12]), aes_gcm::aead::Payload { msg: &blob[12..], aad: id.as_bytes() }).ok()?;
    String::from_utf8(plain).ok()
}

/// 仓库里存的全部秘密的明文。只给诊断用：导出诊断信息前把它们从文字里抹掉。
pub fn all_values(dir: &Path) -> Vec<String> {
    load(dir).keys().filter_map(|id| get(dir, id)).collect()
}

pub fn delete(dir: &Path, id: &str) {
    let mut map = load(dir);
    if map.remove(id).is_some() {
        let _ = save(dir, &map);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_round_trip_and_stay_unreadable_on_disk() {
        let dir = std::env::temp_dir().join(format!("fg_secrets_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        set(&dir, "nas:nas.local:me", "p@ss w0rd").unwrap();
        set(&dir, "backup:t1", "s3cret").unwrap();
        assert_eq!(get(&dir, "nas:nas.local:me").as_deref(), Some("p@ss w0rd"));
        assert_eq!(get(&dir, "backup:t1").as_deref(), Some("s3cret"));
        assert!(get(&dir, "backup:t2").is_none());
        let raw = std::fs::read_to_string(dir.join("secrets.json")).unwrap();
        assert!(!raw.contains("s3cret") && !raw.contains("p@ss"), "nothing in clear text: {raw}");
        // 密文搬到别的名字下解不开
        let mut map = load(&dir);
        let v = map.get("backup:t1").unwrap().clone();
        map.insert("backup:t9".into(), v);
        save(&dir, &map).unwrap();
        assert!(get(&dir, "backup:t9").is_none());
        delete(&dir, "backup:t1");
        assert!(get(&dir, "backup:t1").is_none());
        assert_eq!(get(&dir, "nas:nas.local:me").as_deref(), Some("p@ss w0rd"), "the others are untouched");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(dir.join("secrets.key")).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
