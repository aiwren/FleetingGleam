//! 平台提供的能力，核心只通过这些 trait 使用：系统图像解码器（macOS 的 ImageIO 边车）、系统视觉信号（macOS 的 Apple Vision 边车）、
//! GPU 上的 CLIP 编码器（macOS 的 Metal 实现）。宿主在启动时把自己的实现注册进来；什么都不注册也能跑，只是没有这些信号——
//! 这也是 Android 端在模型层（路线图阶段 B 第 2 步）接上 MediaPipe 之前的状态。
//!
//! 注册的是全局单例（`OnceLock`），因为这些能力在一个进程里只有一套，而且调用方散落在分析流水线的各处，一路传引用并不比全局更清楚。

use std::path::Path;
use std::sync::OnceLock;

use crate::faces::FaceMark;

/// 系统图像解码器能告诉我们的：显示尺寸（已套用 EXIF 方向）和是否 HDR。
#[derive(Clone, Copy, Debug)]
pub struct ImageInfo {
    pub w: u32,
    pub h: u32,
    /// 只有 `with_hdr` 为真时才查（慢）；分析路径现在不用它（见 media.rs），留着是为了格式不变。
    pub hdr: bool,
}

/// 系统图像解码器：`image` crate 不认识的格式（HEIC / AVIF / JPEG XL / TIFF / 相机 RAW、HDR 增益图）都走它。
pub trait SystemImages: Send + Sync {
    /// 这个解码器认识的扩展名（小写、不带点）。`image` crate 自己能读的（jpg / png / webp）不用列。
    fn supports(&self, ext: &str) -> bool;
    /// 显示尺寸（方向已套用）；`with_hdr = false` 跳过较慢的 HDR 检查。
    fn info(&self, path: &Path, with_hdr: bool) -> Option<ImageInfo>;
    /// 一张摆正的 JPEG，长边不超过 `max`；除非 `sdr`，HDR（增益图）保留。
    fn render(&self, src: &Path, out: &Path, max: u32, sdr: bool) -> Result<(), String>;
    /// 照片的一块区域（摆正后的照片的比例，左上为原点），宽至多 `out_w` 像素，存成 JPEG。
    fn crop(&self, src: &Path, out: &Path, f: [f64; 4], out_w: u32) -> Result<(), String>;
}

/// 系统视觉信号对一张照片的回答（macOS 上来自 Apple Vision）。
pub struct VisionOut {
    /// Apple 的 overallScore，大约 -1..1。
    pub aesthetic: f32,
    /// 截图、文档之类的"实用图片"。
    pub utility: bool,
    /// 每张检测到的脸一个拍摄质量值（0..1）。
    pub faces: Vec<f32>,
    /// 每张能定位到眼睛的脸一个睁眼值（大约 0.25–0.4 睁着，低于 0.15 闭着）。
    pub eyes: Vec<f32>,
    /// 与 `eyes` 同序同脸：`[x, y, w, h]`，左上为原点，相对交给分析的那张图归一化。学习模型（mediapipe.rs）读的就是这块。
    pub eye_boxes: Vec<[f32; 4]>,
    /// 每张有完整关键点的脸，用来区分人物（见 faces.rs）。
    pub face_marks: Vec<FaceMark>,
}

/// 系统视觉信号：美学分、实用图片、人脸与眼睛。没有就什么信号都没有，分析照常进行。
pub trait SystemVision: Send + Sync {
    fn analyze(&self, path: &Path) -> Option<VisionOut>;
}

/// GPU 上的 CLIP 视觉编码器：输入按 CLIP 预处理好的 3 x 224 x 224 浮点，输出未归一化的向量。
pub trait GpuEncoder: Send + Sync {
    fn embed(&self, pixels: &[f32]) -> Result<Vec<f32>, String>;
    fn embed_dim(&self) -> usize;
}

/// 从 ONNX 文件的字节建一个 GPU 编码器；这台机器做不到就返回 Err，调用方继续用 ONNX Runtime。
pub trait GpuClipFactory: Send + Sync {
    fn from_onnx(&self, model: &[u8]) -> Result<Box<dyn GpuEncoder>, String>;
}

static IMAGES: OnceLock<Box<dyn SystemImages>> = OnceLock::new();
static GPU_CLIP: OnceLock<Box<dyn GpuClipFactory>> = OnceLock::new();

/// 宿主启动时注册系统图像解码器；只认第一次。
pub fn set_images(p: Box<dyn SystemImages>) {
    let _ = IMAGES.set(p);
}

pub fn images() -> Option<&'static dyn SystemImages> {
    IMAGES.get().map(|b| b.as_ref())
}

/// 宿主启动时注册 GPU 编码器工厂；只认第一次。
pub fn set_gpu_clip(f: Box<dyn GpuClipFactory>) {
    let _ = GPU_CLIP.set(f);
}

pub fn gpu_clip() -> Option<&'static dyn GpuClipFactory> {
    GPU_CLIP.get().map(|b| b.as_ref())
}

/// 这台机器的物理内存（字节）；读不出来就是 0。
pub fn physical_memory() -> u64 {
    #[cfg(target_os = "macos")]
    unsafe {
        let mut v: u64 = 0;
        let mut len = std::mem::size_of::<u64>();
        let name = std::ffi::CString::new("hw.memsize").unwrap();
        if libc::sysctlbyname(name.as_ptr(), &mut v as *mut u64 as *mut libc::c_void, &mut len, std::ptr::null_mut(), 0) == 0 {
            return v;
        }
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        // `MemTotal:       16303236 kB`
        if let Ok(s) = std::fs::read_to_string("/proc/meminfo") {
            if let Some(kb) = s.lines().find_map(|l| l.strip_prefix("MemTotal:")).and_then(|v| v.split_whitespace().next()).and_then(|n| n.parse::<u64>().ok()) {
                return kb * 1024;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_is_known_on_this_machine() {
        assert!(physical_memory() >> 30 >= 1, "a development machine has at least a gigabyte");
    }
}
