//! 系统图像解码器的门面：`image` crate 自己能读的格式（JPEG / PNG / WebP）在这里认；其余的（HEIC / AVIF / JPEG XL / TIFF / 相机 RAW、
//! HDR 增益图）交给宿主注册的 `platform::SystemImages`（macOS 上是 ImageIO 边车 `fg-image`）。没有注册时这些格式就不提供。

use std::path::Path;

pub use crate::platform::ImageInfo as Info;

/// `image` crate 自己解码的格式。
const NATIVE: [&str; 5] = ["jpg", "jpeg", "jpe", "png", "webp"];
/// Web 视图能直接显示原文件（HDR 与广色域保留）的格式。
const WEB: [&str; 14] = ["jpg", "jpeg", "jpe", "png", "webp", "gif", "bmp", "tif", "tiff", "heic", "heif", "avif", "jxl", "svg"];

fn ext(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default()
}

/// 这台机器上有系统解码器。
pub fn available() -> bool {
    crate::platform::images().is_some()
}

/// 这台机器能打开的照片。
pub fn is_supported(p: &Path) -> bool {
    let e = ext(p);
    NATIVE.contains(&e.as_str()) || crate::platform::images().map_or(false, |s| s.supports(&e))
}

/// 由 `image` crate 解码（否则走系统解码器）。
pub fn is_native(p: &Path) -> bool {
    NATIVE.contains(&ext(p).as_str())
}

/// Web 视图能否直接显示原文件。
pub fn web_can_show(p: &Path) -> bool {
    WEB.contains(&ext(p).as_str())
}

/// 显示尺寸（方向已套用）和是否 HDR。`with_hdr = false` 跳过（较慢的）HDR 检查。
pub fn info(path: &Path, with_hdr: bool) -> Option<Info> {
    crate::platform::images()?.info(path, with_hdr)
}

/// 一张摆正的 JPEG，长边不超过 `max`。除非 `sdr`，HDR（增益图）保留。
pub fn render(src: &Path, out: &Path, max: u32, sdr: bool) -> Result<(), String> {
    crate::platform::images().ok_or("the image helper is not available")?.render(src, out, max, sdr)
}

/// 照片的一块区域（摆正后的照片的比例，左上为原点），宽至多 `out_w` 像素，存成 JPEG 到 `out`。
pub fn crop(src: &Path, out: &Path, f: [f64; 4], out_w: u32) -> Result<(), String> {
    crate::platform::images().ok_or("the image helper is not available")?.crop(src, out, f, out_w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_are_classified() {
        assert!(is_native(Path::new("a.JPG")) && !is_native(Path::new("a.heic")));
        assert!(is_supported(Path::new("a.jpg")) && !is_supported(Path::new("x.txt")));
        // 没注册系统解码器时，它的格式就不提供
        assert_eq!(is_supported(Path::new("IMG_0001.DNG")), available());
        assert!(web_can_show(Path::new("a.heic")) && !web_can_show(Path::new("a.dng")));
    }
}
