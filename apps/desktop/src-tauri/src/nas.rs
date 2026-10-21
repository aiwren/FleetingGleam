//! Network shares (a NAS) the app connects to itself, through the `fg-mount` helper and the system's own file-sharing client,
//! so the person never goes through the Finder. A share is remembered as its address and user name only; the password lives in
//! the Keychain. Each share is mounted at its own folder under the app's data folder, which is inside the web view's asset scope,
//! so photos on it are shown like any other.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Share {
    pub id: String,
    pub name: String,
    /// `smb://host/share`, `nfs://host/export`, `afp://…` or `https://…/webdav`.
    pub url: String,
    /// Empty = guest.
    #[serde(default)]
    pub user: String,
}

/// A share as the settings show it.
#[derive(Clone, Serialize)]
pub struct ShareStatus {
    pub id: String,
    pub name: String,
    pub url: String,
    pub user: String,
    pub mountpoint: String,
    pub connected: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct Store {
    list: Vec<Share>,
}

pub const SCHEMES: [&str; 6] = ["smb", "cifs", "nfs", "afp", "http", "https"];

/// The address, checked: a scheme the system client speaks, a host, and a share name.
pub fn check_url(url: &str) -> Result<String, String> {
    let url = url.trim().trim_end_matches('/');
    let (scheme, rest) = url.split_once("://").ok_or("the address needs to start with smb://, nfs://, afp:// or https://")?;
    if !SCHEMES.contains(&scheme.to_ascii_lowercase().as_str()) {
        return Err(format!("{scheme}:// is not a kind of share the system can connect to"));
    }
    let (host, share) = rest.split_once('/').ok_or("the address needs the share name after the host, like smb://nas.local/photos")?;
    if host.is_empty() || host.contains('@') {
        return Err("put the user name in its own field, not in the address".to_string());
    }
    if share.is_empty() {
        return Err("the address needs the share name after the host, like smb://nas.local/photos".to_string());
    }
    Ok(url.to_string())
}

pub fn host_of(url: &str) -> &str {
    url.split("://").nth(1).and_then(|r| r.split('/').next()).unwrap_or("")
}

pub fn load(dir: &Path) -> Vec<Share> {
    std::fs::read(dir.join("nas.json")).ok().and_then(|b| serde_json::from_slice::<Store>(&b).ok()).map(|s| s.list).unwrap_or_default()
}

pub fn save(dir: &Path, list: &[Share]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(&Store { list: list.to_vec() }).map_err(|e| e.to_string())?;
    let path = dir.join("nas.json");
    let tmp = path.with_extension("json.tmp");
    crate::write_synced(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Where a share is mounted: its own folder under the app's data folder.
pub fn mount_dir(data_dir: &Path, id: &str) -> PathBuf {
    data_dir.join("mounts").join(id)
}

/// The volume a path is on: its mount point and file-system type, from the system.
pub fn volume_of(path: &Path) -> Option<(PathBuf, String)> {
    use std::ffi::{CStr, CString};
    let c = CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    let mut fs: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(c.as_ptr(), &mut fs) } != 0 {
        return None;
    }
    let name = |b: &[libc::c_char]| unsafe { CStr::from_ptr(b.as_ptr()) }.to_string_lossy().into_owned();
    Some((PathBuf::from(name(&fs.f_mntonname)), name(&fs.f_fstypename)))
}

/// A file system served over the network: it has no Trash of its own.
pub fn is_network_fs(fstype: &str) -> bool {
    matches!(fstype, "smbfs" | "cifs" | "nfs" | "webdav" | "afpfs")
}

pub fn is_mounted(dir: &Path) -> bool {
    volume_of(dir).map_or(false, |(at, fstype)| at == dir && is_network_fs(&fstype))
}

pub fn status(data_dir: &Path, s: &Share) -> ShareStatus {
    let dir = mount_dir(data_dir, &s.id);
    ShareStatus { id: s.id.clone(), name: s.name.clone(), url: s.url.clone(), user: s.user.clone(), mountpoint: dir.to_string_lossy().into_owned(), connected: is_mounted(&dir) }
}

/// Connecting can take a while when the server is slow to answer; the helper's own limit is about a minute.
const TIMEOUT: Duration = Duration::from_secs(90);

/// Runs the helper with the given arguments (and the password on stdin, when there is one) and reads its one JSON line.
fn helper(bin: &Path, args: &[&str], stdin: Option<&str>) -> Result<serde_json::Value, String> {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot start the share helper: {e}"))?;
    if let (Some(text), Some(mut w)) = (stdin, child.stdin.take()) {
        use std::io::Write;
        let _ = writeln!(w, "{text}");
    }
    let mut out = child.stdout.take().ok_or("no output from the share helper")?;
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut v = Vec::new();
        let _ = out.read_to_end(&mut v);
        v
    });
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < TIMEOUT => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("the server did not answer in time".to_string());
            }
        }
    }
    let bytes = reader.join().map_err(|_| "the share helper stopped")?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| "the share helper gave no answer".to_string())?;
    if v.get("ok").and_then(|b| b.as_bool()) == Some(true) {
        Ok(v)
    } else {
        Err(v.get("error").and_then(|e| e.as_str()).unwrap_or("could not connect").to_string())
    }
}

/// Mounts the share; the password comes from the caller (and is remembered when `save`) or, when `None`, from the Keychain.
pub fn mount(bin: &Path, data_dir: &Path, s: &Share, password: Option<&str>, save: bool) -> Result<PathBuf, String> {
    let dir = mount_dir(data_dir, &s.id);
    if is_mounted(&dir) {
        return Ok(dir);
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let dir_s = dir.to_string_lossy().into_owned();
    let mut args: Vec<&str> = vec!["mount", &s.url, &dir_s];
    if !s.user.is_empty() {
        args.push(&s.user);
    }
    if password.is_some() {
        args.push("--stdin-password");
        if save {
            args.push("--save");
        }
    }
    helper(bin, &args, password)?;
    Ok(dir)
}

pub fn unmount(bin: &Path, data_dir: &Path, s: &Share) -> Result<(), String> {
    let dir = mount_dir(data_dir, &s.id);
    if !is_mounted(&dir) {
        return Ok(());
    }
    helper(bin, &["unmount", &dir.to_string_lossy()], None).map(|_| ())
}

pub fn forget(bin: &Path, s: &Share) {
    if !s.user.is_empty() {
        let _ = helper(bin, &["forget", host_of(&s.url), &s.user], None);
    }
}

/// Where a photo on a network volume goes instead of the Trash: a recycle folder at the root of that volume, by day, so it can be
/// found and restored by hand. Returns the destination path (not yet used).
pub fn network_bin_target(volume_root: &Path, photo: &Path) -> PathBuf {
    let day = {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let days = secs / 86400;
        // civil date from days since 1970 (Howard Hinnant's algorithm), enough for a folder name
        let z = days as i64 + 719468;
        let era = z.div_euclid(146097);
        let doe = z.rem_euclid(146097);
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
    };
    let dir = volume_root.join(".FleetingGleam Recycle Bin").join(day);
    let name = photo.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    let mut target = dir.join(&name);
    let mut n = 1;
    while target.exists() {
        let stem = photo.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let ext = photo.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        target = dir.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    target
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_checked_before_anything_is_tried() {
        assert_eq!(check_url(" smb://nas.local/photos/ ").unwrap(), "smb://nas.local/photos");
        assert!(check_url("ftp://nas/photos").is_err(), "not a kind the system client speaks");
        assert!(check_url("smb://nas.local").is_err(), "no share name");
        assert!(check_url("smb://me@nas.local/photos").is_err(), "the user name has its own field");
        assert!(check_url("nas.local/photos").is_err(), "no scheme");
        assert_eq!(host_of("smb://nas.local/photos"), "nas.local");
    }

    #[test]
    fn the_shares_file_round_trips_without_passwords() {
        let dir = std::env::temp_dir().join(format!("fg_nas_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let list = vec![Share { id: "n1".into(), name: "Home NAS".into(), url: "smb://nas.local/photos".into(), user: "me".into() }];
        save(&dir, &list).unwrap();
        let back = load(&dir);
        assert_eq!(back.len(), 1);
        assert_eq!((back[0].id.as_str(), back[0].user.as_str()), ("n1", "me"));
        assert!(!std::fs::read_to_string(dir.join("nas.json")).unwrap().contains("password"));
        assert!(!is_mounted(&mount_dir(&dir, "n1")), "a plain folder is not a connected share");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_local_disk_is_not_a_network_volume() {
        let (root, fstype) = volume_of(&std::env::temp_dir()).expect("statfs works on the temp dir");
        assert!(root.is_absolute());
        assert!(!is_network_fs(&fstype), "{fstype}");
    }

    #[test]
    fn the_recycle_folder_keeps_names_apart() {
        let dir = std::env::temp_dir().join(format!("fg_bin_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let t1 = network_bin_target(&dir, Path::new("/share/a/IMG_1.jpg"));
        std::fs::create_dir_all(t1.parent().unwrap()).unwrap();
        std::fs::write(&t1, b"x").unwrap();
        let t2 = network_bin_target(&dir, Path::new("/share/b/IMG_1.jpg"));
        assert_ne!(t1, t2);
        assert!(t2.file_name().unwrap().to_string_lossy().contains("IMG_1 (1).jpg"));
        assert!(t1.to_string_lossy().contains(".FleetingGleam Recycle Bin/"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
