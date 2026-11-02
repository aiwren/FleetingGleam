//! 文件夹里的照片：哪些名字不是照片、怎么列出一组文件夹里的全部照片、不解码像素读显示尺寸；以及几个写文件的小工具。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use walkdir::WalkDir;

/// 系统或 NAS 放在真内容旁边的文件夹与文件（不分大小写）：群晖的缩略图 `@eaDir`、回收站 `#recycle`、快照 `#snapshot`；
/// 威联通的回收站 `@Recycle`、快照 `@Recently-Snapshot`、缩略图 `@__thumb`；Windows 的回收站与卷信息。
const JUNK: [&str; 12] = ["__MACOSX", "@eaDir", "#recycle", "#snapshot", "@Recycle", "@Recently-Snapshot", "@__thumb", "$RECYCLE.BIN", "System Volume Information", "Thumbs.db", "desktop.ini", "lost+found"];

/// 从来不是用户照片的名字：隐藏文件（覆盖 macOS 的 `._IMG.jpg` 资源叉和 `.DS_Store`），以及 `JUNK` 里那些。
/// 回收站和快照里是用户删掉的或旧版本的照片，当成照片会让一个项目凭空多出成百上千张。
pub fn is_junk(name: &str) -> bool {
    name.starts_with('.') || JUNK.iter().any(|j| j.eq_ignore_ascii_case(name))
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
    // JPEG 自己读文件头：`image` 的 JPEG 解码器一上来就把整个文件读进内存（20 MB 的照片读 20 MB），
    // 手机存储上一个 1 GB 的文件夹光取尺寸就要十几秒；这里只读到尺寸标记为止，一般是开头几十 KB
    if matches!(p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(), Some("jpg" | "jpeg" | "jpe")) {
        if let Some(d) = jpeg_dims(p) {
            return Some(d);
        }
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

/// JPEG 的显示尺寸：逐个段往后读，记下 APP1 里 EXIF 的方向，读到帧头（SOFn）就给出宽高（方向是 90° / 270° 时对调）。
/// 读不懂的（没有 SOI、帧头里高为 0 之类）给 None，调用方退回慢的那条路。
pub fn jpeg_dims(p: &Path) -> Option<(u32, u32)> {
    use std::io::{BufReader, Read};
    let mut r = BufReader::with_capacity(64 * 1024, std::fs::File::open(p).ok()?);
    let mut b2 = [0u8; 2];
    r.read_exact(&mut b2).ok()?;
    if b2 != [0xFF, 0xD8] {
        return None;
    }
    let mut turned = false;
    loop {
        // 段标记：0xFF（可能有填充的 0xFF）后跟标记字节
        let mut m = [0u8; 1];
        r.read_exact(&mut m).ok()?;
        if m[0] != 0xFF {
            return None;
        }
        let mut marker = 0xFFu8;
        while marker == 0xFF {
            r.read_exact(&mut m).ok()?;
            marker = m[0];
        }
        // 没有长度的独立标记
        if marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        if marker == 0xD9 || marker == 0xDA {
            return None; // 到了图像数据还没见到帧头
        }
        r.read_exact(&mut b2).ok()?;
        let len = u16::from_be_bytes(b2) as usize;
        if len < 2 {
            return None;
        }
        let body = len - 2;
        let sof = matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF);
        if sof {
            let mut h = [0u8; 5];
            r.read_exact(&mut h).ok()?;
            let (hh, ww) = (u16::from_be_bytes([h[1], h[2]]) as u32, u16::from_be_bytes([h[3], h[4]]) as u32);
            if hh == 0 || ww == 0 {
                return None;
            }
            return Some(if turned { (hh, ww) } else { (ww, hh) });
        }
        if marker == 0xE1 && body > 6 {
            let mut seg = vec![0u8; body];
            r.read_exact(&mut seg).ok()?;
            if seg.starts_with(b"Exif\0\0") {
                if let Ok(ex) = exif::Reader::new().read_raw(seg[6..].to_vec()) {
                    let o = ex.get_field(exif::Tag::Orientation, exif::In::PRIMARY).and_then(|f| f.value.get_uint(0)).unwrap_or(1);
                    turned = (5..=8).contains(&o);
                }
            }
            continue;
        }
        std::io::copy(&mut (&mut r).take(body as u64), &mut std::io::sink()).ok()?;
    }
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
        for n in [".DS_Store", "._IMG_0001.jpg", ".Trashes", "__MACOSX", "@eaDir", "Thumbs.db", "#recycle", "#Recycle", "#snapshot", "@Recycle", "@Recently-Snapshot", "@__thumb", "$RECYCLE.BIN"] {
            assert!(is_junk(n), "{n}");
        }
        for n in ["IMG_0001.jpg", "Trip 2026", "DSCF2586.JPG", "recycle", "snapshot 2024", "#1 pick.jpg"] {
            assert!(!is_junk(n), "{n}");
        }
    }

    #[test]
    fn only_readable_photos_are_listed() {
        let dir = std::env::temp_dir().join(format!("fg-filter-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".hidden")).unwrap();
        std::fs::create_dir_all(dir.join("#recycle")).unwrap();
        std::fs::create_dir_all(dir.join("@Recently-Snapshot/GMT+08_2026-10-01")).unwrap();
        let img = image::RgbImage::from_fn(400, 300, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x * y) % 256) as u8]));
        img.save(dir.join("good.jpg")).unwrap();
        img.save(dir.join(".hidden").join("inside.jpg")).unwrap();
        img.save(dir.join("#recycle").join("deleted.jpg")).unwrap();
        img.save(dir.join("@Recently-Snapshot/GMT+08_2026-10-01").join("good.jpg")).unwrap();
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

#[cfg(test)]
mod jpeg_tests {
    use super::*;

    /// 自己读的 JPEG 文件头和 `image` 读出来的尺寸一样；带 EXIF 方向（旋转 90°）的宽高对调；不是 JPEG 的给 None。
    #[test]
    fn jpeg_header_gives_the_same_size_as_decoding() {
        let dir = std::env::temp_dir().join(format!("fg_jpeg_dims_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (w, h) in [(640u32, 480u32), (33, 1000), (4000, 3000)] {
            let p = dir.join(format!("{w}x{h}.jpg"));
            image::RgbImage::from_pixel(w, h, image::Rgb([120, 80, 40])).save(&p).unwrap();
            assert_eq!(jpeg_dims(&p), Some((w, h)));
            assert_eq!(oriented_dims(&p), Some((w, h)));
        }
        // 往 SOI 后面插一个带"方向 = 6（顺时针转 90°）"的 APP1
        let p = dir.join("640x480.jpg");
        let bytes = std::fs::read(&p).unwrap();
        let mut tiff = vec![b'M', b'M', 0, 42, 0, 0, 0, 8, 0, 1, 0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut app1 = b"Exif\0\0".to_vec();
        app1.append(&mut tiff);
        let mut turned = vec![0xFF, 0xD8, 0xFF, 0xE1];
        turned.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
        turned.extend_from_slice(&app1);
        turned.extend_from_slice(&bytes[2..]);
        let q = dir.join("turned.jpg");
        std::fs::write(&q, &turned).unwrap();
        assert_eq!(jpeg_dims(&q), Some((480, 640)));
        let png = dir.join("x.png");
        image::RgbImage::from_pixel(10, 20, image::Rgb([0, 0, 0])).save(&png).unwrap();
        assert_eq!(jpeg_dims(&png), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// 文件名（不含扩展名）的小写形式，配对按它认。
fn stem_lower(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// RAW + JPG 配对：同一文件夹里同名（不分大小写）的 RAW 和非 RAW 算一张照片。代表文件是非 RAW 那张（JPG 优先于 HEIC，再是其它），
/// RAW 作为"伴随文件"跟着它：分析只做一次，保留 / 淘汰、导出、回收一起动。只有 RAW（或只有非 RAW）时各算各的。
/// 返回（代表文件列表，代表 → 伴随文件）。
pub fn pair(files: Vec<PathBuf>) -> (Vec<PathBuf>, HashMap<PathBuf, Vec<PathBuf>>) {
    fn rank(p: &Path) -> u8 {
        let e = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        match e.as_str() {
            "jpg" | "jpeg" | "jpe" => 0,
            "heic" | "heif" => 1,
            _ => 2,
        }
    }
    let mut groups: HashMap<(PathBuf, String), Vec<PathBuf>> = HashMap::new();
    for f in files {
        let key = (f.parent().map(Path::to_path_buf).unwrap_or_default(), stem_lower(&f));
        groups.entry(key).or_default().push(f);
    }
    let mut reps = Vec::new();
    let mut map = HashMap::new();
    for (_, g) in groups {
        let (mut raws, mut others): (Vec<PathBuf>, Vec<PathBuf>) = g.into_iter().partition(|p| crate::imageio::is_raw(p));
        if raws.is_empty() || others.is_empty() {
            reps.extend(raws);
            reps.extend(others);
            continue;
        }
        others.sort_by_key(|p| (rank(p), p.clone()));
        raws.sort();
        let rep = others.remove(0);
        reps.extend(others);
        map.insert(rep.clone(), raws);
        reps.push(rep);
    }
    reps.sort();
    (reps, map)
}

/// 一张照片此刻在磁盘上的伴随文件（同文件夹、同名的 RAW），整理时用：保留 / 淘汰 / 导出一起动。RAW 自己没有伴随文件。
pub fn companions_of(p: &Path) -> Vec<PathBuf> {
    if crate::imageio::is_raw(p) {
        return Vec::new();
    }
    let Some(dir) = p.parent() else { return Vec::new() };
    let stem = stem_lower(p);
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|q| q != p && q.is_file() && crate::imageio::is_raw(q) && stem_lower(q) == stem)
        .collect();
    out.sort();
    out
}

/// 把一批路径扩成"自己 + 伴随文件"，去重、保持顺序。
pub fn with_companions(paths: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(paths.len());
    for p in paths {
        for q in std::iter::once(PathBuf::from(&p)).chain(companions_of(Path::new(&p))) {
            let q = q.to_string_lossy().into_owned();
            if !out.contains(&q) {
                out.push(q);
            }
        }
    }
    out
}

#[cfg(test)]
mod pair_tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn raw_pairs_with_its_jpg_and_lone_files_stand_alone() {
        let files = vec![p("/a/IMG_1.CR3"), p("/a/IMG_1.JPG"), p("/a/IMG_2.NEF"), p("/a/IMG_3.jpg"), p("/a/img_4.jpg"), p("/a/IMG_4.dng"), p("/a/IMG_4.heic"), p("/b/IMG_1.jpg")];
        let (reps, map) = pair(files);
        assert_eq!(reps, vec![p("/a/IMG_1.JPG"), p("/a/IMG_2.NEF"), p("/a/IMG_3.jpg"), p("/a/IMG_4.heic"), p("/a/img_4.jpg"), p("/b/IMG_1.jpg")]);
        assert_eq!(map[&p("/a/IMG_1.JPG")], vec![p("/a/IMG_1.CR3")]);
        assert_eq!(map[&p("/a/img_4.jpg")], vec![p("/a/IMG_4.dng")], "the JPG, not the HEIC, carries the RAW");
        assert!(!map.contains_key(&p("/a/IMG_2.NEF")), "a RAW without a JPG is its own photo");
        assert!(!map.contains_key(&p("/b/IMG_1.jpg")), "the same name in another folder is another photo");
    }

    #[test]
    fn companions_are_read_from_disk_when_organizing() {
        let dir = std::env::temp_dir().join(format!("fg_pair_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for n in ["a.jpg", "a.ARW", "a.txt", "b.jpg"] {
            std::fs::write(dir.join(n), b"x").unwrap();
        }
        assert_eq!(companions_of(&dir.join("a.jpg")), vec![dir.join("a.ARW")]);
        assert!(companions_of(&dir.join("b.jpg")).is_empty());
        assert!(companions_of(&dir.join("a.ARW")).is_empty());
        let all = with_companions(vec![dir.join("a.jpg").to_string_lossy().into_owned(), dir.join("b.jpg").to_string_lossy().into_owned()]);
        assert_eq!(all.len(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
