//! The system image decoder (ImageIO, through the `fg-image` helper) for everything the `image` crate cannot do: HDR gain maps,
//! HEIC / AVIF / JPEG XL / TIFF / camera RAW. On other platforms there is no helper and these formats are simply not offered.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use serde::Deserialize;

static BIN: OnceLock<PathBuf> = OnceLock::new();

/// Tells this module where the helper is (done once at start-up).
pub fn init(bin: PathBuf) {
    let _ = BIN.set(bin);
}

fn bin() -> Option<&'static PathBuf> {
    let b = BIN.get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/fg-image"));
    b.exists().then_some(b)
}

pub fn available() -> bool {
    cfg!(target_os = "macos") && bin().is_some()
}

/// Formats the `image` crate decodes itself.
const NATIVE: [&str; 5] = ["jpg", "jpeg", "jpe", "png", "webp"];
/// Formats only the system decoder reads (macOS): modern containers and camera RAW (Apple ProRAW and other DNG included).
const SYSTEM: [&str; 36] = [
    "heic", "heif", "avif", "jxl", "tif", "tiff", "bmp", "gif",
    "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef", "srw", "x3f", "3fr", "erf", "kdc", "mef", "mos", "mrw", "raw", "rwl", "iiq", "fff", "dcr", "k25", "rwz",
];
/// What a web view shows straight from the original file, with its HDR and wide colour intact.
const WEB: [&str; 14] = ["jpg", "jpeg", "jpe", "png", "webp", "gif", "bmp", "tif", "tiff", "heic", "heif", "avif", "jxl", "svg"];

fn ext(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default()
}

/// A photo the app can open on this machine.
pub fn is_supported(p: &Path) -> bool {
    let e = ext(p);
    NATIVE.contains(&e.as_str()) || (available() && SYSTEM.contains(&e.as_str()))
}

/// Decoded by the `image` crate (otherwise it goes through the helper).
pub fn is_native(p: &Path) -> bool {
    NATIVE.contains(&ext(p).as_str())
}

/// Whether the web view can show the original file itself.
pub fn web_can_show(p: &Path) -> bool {
    WEB.contains(&ext(p).as_str())
}

#[derive(Deserialize, Clone, Copy)]
pub struct Info {
    pub w: u32,
    pub h: u32,
    #[allow(dead_code)] // the HDR check of the helper is not used on the analysis path any more (see media.rs); kept for the format
    pub hdr: bool,
}

/// Size as displayed (orientation applied) and whether the photo is HDR. `with_hdr = false` skips the (slower) HDR check.
pub fn info(path: &Path, with_hdr: bool) -> Option<Info> {
    let b = bin()?;
    let mut cmd = Command::new(b);
    cmd.arg("info").arg(path);
    if !with_hdr {
        cmd.arg("fast");
    }
    let out = cmd.output().ok().filter(|o| o.status.success())?;
    serde_json::from_slice(&out.stdout).ok()
}

/// An upright JPEG of the photo, longest side at most `max`. HDR is kept (as a gain map) unless `sdr`.
pub fn render(src: &Path, out: &Path, max: u32, sdr: bool) -> Result<(), String> {
    let b = bin().ok_or("the image helper is not available")?;
    let mut cmd = Command::new(b);
    cmd.arg("render").arg(src).arg(out).arg(max.to_string());
    if sdr {
        cmd.arg("sdr");
    }
    let o = cmd.output().map_err(|e| e.to_string())?;
    if o.status.success() && out.exists() {
        Ok(())
    } else {
        Err(format!("{}: {}", src.display(), String::from_utf8_lossy(&o.stderr).trim()))
    }
}

/// A region of the photo (fractions of the upright photo, top-left origin) at up to `out_w` pixels wide, as a JPEG at `out`.
pub fn crop(src: &Path, out: &Path, f: [f64; 4], out_w: u32) -> Result<(), String> {
    let b = bin().ok_or("the image helper is not available")?;
    let o = Command::new(b)
        .arg("crop").arg(src).arg(out).arg(f[0].to_string()).arg(f[1].to_string()).arg(f[2].to_string()).arg(f[3].to_string()).arg(out_w.to_string())
        .output().map_err(|e| e.to_string())?;
    if o.status.success() && out.exists() { Ok(()) } else { Err(format!("{}: {}", src.display(), String::from_utf8_lossy(&o.stderr).trim())) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_are_classified() {
        assert!(is_native(Path::new("a.JPG")) && !is_native(Path::new("a.heic")));
        assert!(is_supported(Path::new("IMG_0001.DNG")) == available() && !is_supported(Path::new("x.txt")));
        assert!(web_can_show(Path::new("a.heic")) && !web_can_show(Path::new("a.dng")));
    }
}
