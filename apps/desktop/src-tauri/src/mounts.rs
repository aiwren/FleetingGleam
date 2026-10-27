//! 访达挂载的网络卷（用户自己挂的，不走应用内置的 SMB 客户端）：这样的卷没有废纸篓，淘汰的照片放到卷根目录的回收文件夹。
//! 只有 macOS 有这件事；Android 上照片要么在本机，要么在 NAS 镜像里。

use std::path::{Path, PathBuf};

use fg_core::nas::{today, RECYCLE_DIR};

/// 一个路径所在的卷：挂载点和文件系统类型（来自系统）。
#[cfg(target_os = "macos")]
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

#[cfg(not(target_os = "macos"))]
pub fn volume_of(_path: &Path) -> Option<(PathBuf, String)> {
    None
}

/// 网络上的文件系统：没有自己的废纸篓。
pub fn is_network_fs(fstype: &str) -> bool {
    matches!(fstype, "smbfs" | "cifs" | "nfs" | "webdav" | "afpfs")
}

/// 访达挂载的网络卷上，淘汰的照片去的地方：卷根目录的回收文件夹，按天分开，可以手动找回。只算出目标路径，不动文件。
pub fn network_bin_target(volume_root: &Path, photo: &Path) -> PathBuf {
    let dir = volume_root.join(RECYCLE_DIR).join(today());
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

/// 注册给核心库的本机废纸篓：照片进系统废纸篓（能在访达里放回）；访达挂载的网络卷没有废纸篓，照片挪到卷根目录的回收文件夹。
pub struct DesktopTrash;

impl fg_core::platform::LocalTrash for DesktopTrash {
    fn trash(&self, path: &Path) -> Result<Option<String>, String> {
        self.trash_at(path).map(|(bin, _)| bin)
    }

    /// 网络卷：挪进卷上的回收文件夹，交回那个文件的位置。本机：系统废纸篓，用 NSFileManager 的 `trashItemAtURL`
    /// 拿回它在废纸篓里的确切位置（访达"放回原处"认的也是它）；不需要完全磁盘访问，也不弹访达的自动化授权。
    fn trash_at(&self, path: &Path) -> Result<(Option<String>, Option<PathBuf>), String> {
        if let Some((root, _)) = volume_of(path).filter(|(_, fstype)| is_network_fs(fstype)) {
            let target = network_bin_target(&root, path);
            let bin = target.parent().and_then(|d| d.parent()).map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
            if let Some(dir) = target.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::rename(path, &target).map_err(|e| e.to_string())?;
            return Ok((Some(bin), Some(target)));
        }
        #[cfg(target_os = "macos")]
        {
            use objc2_foundation::{NSFileManager, NSString, NSURL};
            let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
            let mut out = None;
            NSFileManager::defaultManager()
                .trashItemAtURL_resultingItemURL_error(&url, Some(&mut out))
                .map_err(|e| e.localizedDescription().to_string())?;
            let at = out.and_then(|u| u.path()).map(|p| PathBuf::from(p.to_string()));
            Ok((None, at))
        }
        #[cfg(not(target_os = "macos"))]
        {
            trash::TrashContext::default().delete(path).map(|_| (None, None)).map_err(|e| e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn a_local_disk_is_not_a_network_volume() {
        let (root, fstype) = volume_of(&std::env::temp_dir()).expect("statfs works on the temp dir");
        assert!(root.is_absolute());
        assert!(!is_network_fs(&fstype), "{fstype}");
    }

    /// 进废纸篓、拿回确切位置、按它放回原处（撤销用的就是这条路）；放回之后废纸篓里不留东西。
    #[test]
    fn a_photo_goes_to_the_trash_and_comes_back() {
        use fg_core::platform::LocalTrash;
        let dir = std::env::temp_dir().join(format!("fg_trash_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let photo = dir.join("fg-test-photo.jpg");
        std::fs::write(&photo, b"not really a jpeg").unwrap();
        let (bin, at) = DesktopTrash.trash_at(&photo).unwrap();
        assert_eq!(bin, None);
        assert!(!photo.exists(), "the photo should be in the trash now");
        let at = at.expect("the trash tells where the photo went");
        assert!(at.is_file(), "{at:?}");
        std::fs::rename(&at, &photo).unwrap();
        assert_eq!(std::fs::read(&photo).unwrap(), b"not really a jpeg");
        let _ = std::fs::remove_dir_all(&dir);
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
