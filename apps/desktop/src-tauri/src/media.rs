//! What a photo carries beyond its pixels: an HDR gain map (with a thumbnail that keeps it) and, for motion photos, a short video
//! hidden after the JPEG (Google's Motion Photo / Micro Video, which is what Xiaomi, Pixel and Samsung phones write).

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;


pub struct Media {
    pub hdr: bool,
    pub motion: bool,
}

/// How much of the start of a file is searched for the XMP block.
const HEAD: usize = 256 * 1024;

fn head(path: &Path) -> Option<(Vec<u8>, u64)> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let mut buf = vec![0u8; HEAD.min(len as usize)];
    f.read_exact(&mut buf).ok()?;
    Some((buf, len))
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// The number after `key="` in `hay`.
fn number_after(hay: &[u8], key: &[u8]) -> Option<u64> {
    let i = find(hay, key)? + key.len();
    let digits: Vec<u8> = hay[i..].iter().copied().take_while(|b| b.is_ascii_digit()).collect();
    std::str::from_utf8(&digits).ok()?.parse().ok()
}

/// A JPEG with a gain map (Ultra HDR): found without the helper, for platforms that have none.
pub fn has_gain_map(path: &Path) -> bool {
    head(path).map_or(false, |(h, _)| find(&h, b"hdrgm:").is_some() || find(&h, b"GainMap").is_some())
}

/// Where the video of a motion photo starts and how long it is, if the photo has one.
pub fn motion_range(path: &Path) -> Option<(u64, u64)> {
    let (h, len) = head(path)?;
    let from_end = if let Some(n) = number_after(&h, b"MicroVideoOffset=\"") {
        Some(n)
    } else if find(&h, b"MotionPhoto=\"1\"").is_some() || find(&h, b"Semantic=\"MotionPhoto\"").is_some() {
        // the item that is the video: the `<Container:Item ... Item:Semantic="MotionPhoto" ... Item:Length="N" .../>` element that holds the
        // semantic (the gain map's item sits right before it and has a Length of its own, so only this element is searched)
        let at = find(&h, b"Semantic=\"MotionPhoto\"")?;
        let lo = h[..at].windows(15).rposition(|w| w == b"<Container:Item").unwrap_or(at.saturating_sub(300));
        let hi = find(&h[at..], b"/>").map_or(h.len(), |i| at + i);
        number_after(&h[lo..hi], b"Length=\"").filter(|&n| n > 0)
    } else {
        None
    }?;
    if from_end == 0 || from_end >= len {
        return None;
    }
    let start = len - from_end;
    // the clip is an MP4: "ftyp" follows the 4-byte box size
    let mut f = std::fs::File::open(path).ok()?;
    f.seek(SeekFrom::Start(start + 4)).ok()?;
    let mut tag = [0u8; 4];
    f.read_exact(&mut tag).ok()?;
    (&tag == b"ftyp").then_some((start, from_end))
}

/// Writes the video of a motion photo to `out`.
pub fn extract_motion(path: &Path, out: &Path) -> Result<(), String> {
    let (start, n) = motion_range(path).ok_or("this photo has no motion clip")?;
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    f.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
    let mut tmp = out.to_path_buf();
    tmp.set_extension("tmp");
    let mut dst = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    std::io::copy(&mut f.take(n), &mut dst).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, out).map_err(|e| e.to_string())
}

/// Whether a photo is HDR (a gain map) and whether it is a motion photo, from the start of the file alone: no decoding and no helper process,
/// so it costs next to nothing per photo. Other formats (HEIC, AVIF, RAW...) are not looked at here; the viewer asks the system decoder when it
/// opens one. Thumbnails are always plain, small and standard range.
pub fn probe(path: &Path, _cache_dir: &Path) -> Media {
    let jpeg = matches!(path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(), Some("jpg" | "jpeg" | "jpe"));
    if !jpeg {
        return Media { hdr: false, motion: false };
    }
    Media { hdr: has_gain_map(path), motion: motion_range(path).is_some() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_read_after_a_key() {
        assert_eq!(number_after(b"x GCamera:MicroVideoOffset=\"4242\" y", b"MicroVideoOffset=\""), Some(4242));
        assert_eq!(number_after(b"nothing here", b"Length=\""), None);
    }

    #[test]
    fn a_motion_photo_is_found_by_its_xmp() {
        let dir = std::env::temp_dir().join("fg_motion_test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("m.jpg");
        // the gain map's item (with a Length of its own) comes first, as in real Xiaomi files
        let xmp = b"\xff\xd8 <x:xmpmeta GCamera:MotionPhoto=\"1\"><Container:Item Item:Mime=\"image/jpeg\" Item:Length=\"667\" Item:Semantic=\"GainMap\"/><Container:Item Item:Mime=\"video/mp4\" Item:Semantic=\"MotionPhoto\" Item:Length=\"20\" Item:Padding=\"0\"/></x:xmpmeta> more jpeg bytes ";
        let mut file = xmp.to_vec();
        let video: Vec<u8> = [&[0u8, 0, 0, 20][..], b"ftypisom", &[0u8; 8]].concat();
        file.extend_from_slice(&video);
        std::fs::write(&p, &file).unwrap();
        let (start, n) = motion_range(&p).expect("motion photo");
        assert_eq!((start as usize, n as usize), (xmp.len(), 20));
        let out = dir.join("m.mp4");
        extract_motion(&p, &out).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), video);
        assert!(motion_range(&dir.join("nope.jpg")).is_none());
    }
}
