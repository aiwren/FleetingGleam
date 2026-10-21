//! 文件夹里的照片：哪些名字不是照片、怎么列出一组文件夹里的全部照片、不解码像素读显示尺寸；以及几个写文件的小工具。

use std::path::{Path, PathBuf};

use rayon::prelude::*;
use walkdir::WalkDir;

/// 从来不是用户照片的名字：隐藏文件（覆盖 macOS 的 `._IMG.jpg` 资源叉和 `.DS_Store`），以及系统或 NAS 放在真内容旁边的文件夹。
pub fn is_junk(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "__MACOSX" | "@eaDir" | "$RECYCLE.BIN" | "System Volume Information" | "Thumbs.db" | "desktop.ini")
}

/// 比这小的是图标或缩略图，不是照片。
pub const MIN_PHOTO_BYTES: u64 = 8 * 1024;

/// 一个文件夹（递归）里的照片，按路径排序；只看扩展名和大小，不读文件。
pub fn list_images(dir: &str) -> Vec<PathBuf> {
    let mut files: Vec<_> = WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !is_junk(&e.file_name().to_string_lossy()))
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && crate::analyze::is_image(e.path()) && e.metadata().map_or(false, |m| m.len() >= MIN_PHOTO_BYTES))
        .map(|e| e.into_path())
        .collect();
    files.sort();
    files
}

/// 几个文件夹里的全部照片，去重。文件头读不出来的（损坏，或扩展名不对）不算，这样用户看到的数量、照片墙和分析才一致。
pub fn list_all(dirs: &[String]) -> Vec<PathBuf> {
    list_all_with(dirs, |_| false)
}

/// 同 `list_all`，但 `known(path)` 为真的文件（分析缓存里已经有、文件没变过）不再读文件头验证：打开一个分析过的项目时，
/// 几百个 20 MB 的 JPEG 头一张张读要一两秒，这是"正在读取文件夹"卡住的主要原因。
pub fn list_all_with(dirs: &[String], known: impl Fn(&Path) -> bool + Sync) -> Vec<PathBuf> {
    let mut all: Vec<PathBuf> = dirs.iter().flat_map(|d| list_images(d)).collect();
    all.sort();
    all.dedup();
    all.into_par_iter().filter(|p| known(p) || oriented_dims(p).is_some()).collect()
}

/// 显示尺寸（套用 EXIF 方向），只读文件头不解码像素。`image` crate 不认识的格式问系统解码器。
pub fn oriented_dims(p: &Path) -> Option<(u32, u32)> {
    use image::metadata::Orientation as O;
    if !crate::imageio::is_native(p) {
        return crate::imageio::info(p, false).map(|i| (i.w, i.h));
    }
    use image::ImageDecoder;
    let reader = image::ImageReader::open(p).ok()?.with_guessed_format().ok()?;
    let mut dec = reader.into_decoder().ok()?;
    let (w, h) = dec.dimensions();
    Some(match dec.orientation().unwrap_or(O::NoTransforms) {
        O::Rotate90 | O::Rotate270 | O::Rotate90FlipH | O::Rotate270FlipH => (h, w),
        _ => (w, h),
    })
}

/// 写入并等它落盘，这样随后的重命名在断电后也不会指向一个空的或写了一半的文件。
pub fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

/// `name.json` → `name.json.corrupt-<unix 秒>`：读不出来的文件改名放在一旁，不删。
pub fn quarantine_name(path: &Path) -> PathBuf {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(format!(".corrupt-{secs}"));
    path.with_file_name(name)
}

/// `dir` 里还不存在的一个路径：`name.jpg`，然后 `name (2).jpg`、`name (3).jpg`……
pub fn unique_path(dir: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let p = Path::new(name);
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|c| !c.exists()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_and_system_names_are_junk() {
        for n in [".DS_Store", "._IMG_0001.jpg", ".Trashes", "__MACOSX", "@eaDir", "Thumbs.db"] {
            assert!(is_junk(n), "{n}");
        }
        for n in ["IMG_0001.jpg", "Trip 2026", "DSCF2586.JPG"] {
            assert!(!is_junk(n), "{n}");
        }
    }

    #[test]
    fn only_readable_photos_are_listed() {
        let dir = std::env::temp_dir().join(format!("fg-filter-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".hidden")).unwrap();
        let img = image::RgbImage::from_fn(400, 300, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x * y) % 256) as u8]));
        img.save(dir.join("good.jpg")).unwrap();
        img.save(dir.join(".hidden").join("inside.jpg")).unwrap();
        img.save(dir.join("._good.jpg")).unwrap();
        std::fs::write(dir.join("fake.jpg"), vec![7u8; 20_000]).unwrap(); // 不是图片
        std::fs::write(dir.join("tiny.jpg"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"hello").unwrap();
        std::fs::write(dir.join("clip.mov"), vec![0u8; 20_000]).unwrap();
        let found = list_all(&[dir.to_string_lossy().into_owned()]);
        let names: Vec<_> = found.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["good.jpg"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unique_names_never_overwrite() {
        let dir = std::env::temp_dir().join(format!("fg_unique_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.jpg"), b"1").unwrap();
        std::fs::write(dir.join("a (2).jpg"), b"2").unwrap();
        assert_eq!(unique_path(&dir, std::ffi::OsStr::new("a.jpg")), dir.join("a (3).jpg"));
        assert_eq!(unique_path(&dir, std::ffi::OsStr::new("b.jpg")), dir.join("b.jpg"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
