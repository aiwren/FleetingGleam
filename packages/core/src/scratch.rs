//! Keeps the scratch files next to the thumbnails within bounds. The thumbnails and analysis copies are what the project needs; these are
//! only conveniences that are made again when wanted: the larger previews, the sharp crops made while zooming, and the clips cut out of
//! motion photos. Left alone they grow with every photo looked at (a single zoom session writes dozens of 2 MB crops).

use std::path::Path;
use std::time::SystemTime;

const DETAIL_MAX: u64 = 150 << 20;
const LARGE_MAX: u64 = 400 << 20;
const CLIPS_MAX: u64 = 300 << 20;

/// Removes the oldest files of `dir` whose name satisfies `pick` until what is left of them takes at most `max` bytes.
fn trim(dir: &Path, pick: impl Fn(&str) -> bool, max: u64) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(SystemTime, u64, std::path::PathBuf)> = rd
        .flatten()
        .filter(|e| pick(&e.file_name().to_string_lossy()))
        .filter_map(|e| {
            let m = e.metadata().ok().filter(|m| m.is_file())?;
            Some((m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    if total <= max {
        return;
    }
    files.sort_by_key(|f| f.0);
    for (_, len, path) in files {
        if total <= max {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}

/// Whole pass over the thumbnail folder: run in the background when the app starts.
pub fn prune(thumbs: &Path) {
    // pictures an earlier version made for HDR thumbnails; nothing uses them any more
    if let Ok(rd) = std::fs::read_dir(thumbs) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().ends_with(".h.jpg") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    prune_detail(thumbs);
    trim(&thumbs.join("large"), |_| true, LARGE_MAX);
    trim(thumbs, |n| n.ends_with(".m.mp4"), CLIPS_MAX);
}

/// The sharp crops: cheap to check, so also run while zooming (rarely).
pub fn prune_detail(thumbs: &Path) {
    trim(&thumbs.join("detail"), |_| true, DETAIL_MAX);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn put(dir: &Path, name: &str, len: usize, age_s: u64) {
        let p = dir.join(name);
        std::fs::write(&p, vec![0u8; len]).unwrap();
        let f = std::fs::File::options().write(true).open(&p).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(age_s)).unwrap();
    }

    #[test]
    fn the_oldest_files_go_first_and_only_until_it_fits() {
        let dir = std::env::temp_dir().join("fg_scratch_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        put(&dir, "old.m.mp4", 400, 300);
        put(&dir, "mid.m.mp4", 400, 200);
        put(&dir, "new.m.mp4", 400, 100);
        put(&dir, "keep.jpg", 5000, 900);   // not a clip: never touched by the clip limit
        trim(&dir, |n| n.ends_with(".m.mp4"), 900);
        assert!(!dir.join("old.m.mp4").exists());
        assert!(dir.join("mid.m.mp4").exists() && dir.join("new.m.mp4").exists());
        assert!(dir.join("keep.jpg").exists());
        trim(&dir, |n| n.ends_with(".m.mp4"), 10_000);   // already within bounds: nothing happens
        assert!(dir.join("mid.m.mp4").exists());
    }

    #[test]
    fn old_hdr_thumbnails_are_removed() {
        let dir = std::env::temp_dir().join("fg_scratch_test2");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        put(&dir, "aaaa.h.jpg", 10, 1);
        put(&dir, "aaaa.jpg", 10, 1);
        prune(&dir);
        assert!(!dir.join("aaaa.h.jpg").exists() && dir.join("aaaa.jpg").exists());
    }
}
