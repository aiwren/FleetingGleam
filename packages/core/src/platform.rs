//! 平台提供的能力，核心只通过这些 trait 使用：系统图像解码器（macOS 的 ImageIO 边车）、系统视觉信号（macOS 的 Apple Vision 边车）、
//! GPU 上的 CLIP 编码器（macOS 的 Metal 实现）。宿主在启动时把自己的实现注册进来；什么都不注册也能跑，只是没有这些信号——
//! 这也是 Android 端在模型层（路线图阶段 B 第 2 步）接上 MediaPipe 之前的状态。
//!
//! 注册的是全局单例（`OnceLock`），因为这些能力在一个进程里只有一套，而且调用方散落在分析流水线的各处，一路传引用并不比全局更清楚。

use std::path::Path;
use std::sync::OnceLock;

use image::DynamicImage;

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
    /// `image` crate 自己能读的格式（JPEG / PNG / WebP）也优先交给它：进程内的解码器（Android 的 BitmapFactory）按需缩放解码，
    /// 两亿像素的照片也不用整张进内存；macOS 的边车是另一个进程，每次调用都有开销，所以默认不。
    fn prefers_native(&self) -> bool {
        false
    }
    /// 大图预览（查看器、单张、对比用的那张）里，非原生格式（HEIC / AVIF / RAW…）的长边上限。默认 30000，也就是不缩：
    /// Mac 上 RAW 的大图要全像素（放大看细节），ImageIO 渲染也不怕大；手机上一张 48 MP 的 HEIC 整张解出来是 190 MB 的位图，
    /// 还会预取好几张，Android 给 2560，和原生格式一样。
    fn preview_max(&self) -> u32 {
        30000
    }
    /// 直接解到内存：照片摆正、缩到长边不超过 `max`、SDR 的像素，连同原图的显示尺寸（宽、高）。有这条路的实现（macOS 常驻的
    /// ImageIO 边车：JPEG 在 DCT 域缩放，比整张解出来再缩快好几倍；Android 的 BitmapFactory：按 2 的幂跳着解，像素经 UniFFI 回来，
    /// 不再压一张 JPEG 写盘再解回来）分析优先走它；默认没有（None），调用方按老路子解。
    /// 解不了这张（格式不认、文件坏了）也是 None，调用方会再试别的路。
    fn decode(&self, _src: &Path, _max: u32) -> Option<(DynamicImage, u32, u32)> {
        None
    }
}

/// 系统视觉信号对一张照片的回答（macOS 上来自 Apple Vision）。
pub struct VisionOut {
    /// Apple 的 overallScore，大约 -1..1；只有 Apple Vision 给得出，别的实现是 None。
    pub aesthetic: Option<f32>,
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
    /// `eyes` 已经是 MediaPipe 量表上的睁眼值（Android 的人脸流水线自己量的）：分析时不用再对同一张脸算一遍网格。
    pub eyes_mediapipe: bool,
}

/// 系统视觉信号：美学分、实用图片、人脸与眼睛。没有就什么信号都没有，分析照常进行。
///
/// 两条路：`analyze_pixels` 直接看分析线程内存里那张摆正的 1024px 副本（Mac 的边车经管道收像素，Android 的人脸流水线本来就在进程内），
/// 不用先写一张 JPEG 到盘上再读回来；`analyze` 从文件读，留给手头只有缓存副本的地方（重新量眼睛、补人物向量）。
pub trait SystemVision: Send + Sync {
    fn analyze(&self, path: &Path) -> Option<VisionOut>;
    /// 这个实现能直接看像素吗？能，分析就走 `analyze_pixels`；不能（默认），分析先存一张分析副本再按路径问。
    fn takes_pixels(&self) -> bool {
        false
    }
    /// 直接看内存里的像素（摆正、长边 1024 的分析副本）。只有 `takes_pixels` 为真才会被调用；默认没有这条路。
    fn analyze_pixels(&self, _img: &DynamicImage) -> Option<VisionOut> {
        None
    }
}

/// 每次分析起一套系统视觉信号：桌面端是几个 Apple Vision 边车进程，Android 是核心库自己的人脸流水线（`facepipe::Factory`）。
/// 这台机器、这套模型给不出就返回 None，分析照常进行，只是没有人脸与闭眼。
pub trait VisionFactory: Send + Sync {
    fn start(&self, model_dir: &Path, workers: usize, cpu_only: bool) -> Option<Box<dyn SystemVision>>;
}

/// 把一张本机照片放进系统的废纸篓（能找回）。`Ok(None)` 是进了废纸篓；`Ok(Some(位置))` 是这个卷没有废纸篓，
/// 照片挪到了那里的回收文件夹（页面要告诉用户在哪）。Android 上进系统回收站要用户在系统确认框里点同意，核心做不了，
/// 所以 Android 不注册它，由界面自己处理（见 `engine::TrashResult::deferred`）。
pub trait LocalTrash: Send + Sync {
    fn trash(&self, path: &Path) -> Result<Option<String>, String>;
}

static VISION: OnceLock<Box<dyn VisionFactory>> = OnceLock::new();
static LOCAL_TRASH: OnceLock<Box<dyn LocalTrash>> = OnceLock::new();

/// 宿主启动时注册系统视觉信号的工厂；只认第一次。
pub fn set_vision(f: Box<dyn VisionFactory>) {
    let _ = VISION.set(f);
}

pub fn vision() -> Option<&'static dyn VisionFactory> {
    VISION.get().map(|b| b.as_ref())
}

/// 宿主启动时注册本机废纸篓；只认第一次。
pub fn set_local_trash(t: Box<dyn LocalTrash>) {
    let _ = LOCAL_TRASH.set(t);
}

pub fn local_trash() -> Option<&'static dyn LocalTrash> {
    LOCAL_TRASH.get().map(|b| b.as_ref())
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
static SCRATCH: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

/// 宿主告诉核心可以写临时文件的地方（Android 上系统的临时目录应用写不了）；没设就用系统的。
pub fn set_scratch_dir(dir: std::path::PathBuf) {
    let _ = std::fs::create_dir_all(&dir);
    *SCRATCH.lock().unwrap_or_else(|e| e.into_inner()) = Some(dir);
}

static PROXY: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// 宿主告诉核心走哪个 HTTP 代理（`http://主机:端口`；Android 上来自系统的全局代理设置）；None = 直连。
pub fn set_proxy(url: Option<String>) {
    *PROXY.lock().unwrap_or_else(|e| e.into_inner()) = url.filter(|u| !u.trim().is_empty());
}

pub fn proxy() -> Option<String> {
    PROXY.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// 核心所有联网（模型下载、云备份）共用的 HTTP 客户端构建器：超时由调用方定，代理按 `proxy()`。
pub fn http_agent(connect: std::time::Duration, read: std::time::Duration) -> ureq::Agent {
    let mut b = ureq::AgentBuilder::new().timeout_connect(connect).timeout_read(read);
    if let Some(p) = proxy() {
        match ureq::Proxy::new(&p) {
            Ok(px) => b = b.proxy(px),
            Err(e) => eprintln!("proxy {p} ignored: {e}"),
        }
    }
    b.build()
}

/// 临时文件的目录：非原生格式的临时渲染、加速器的标记文件都放这里。
pub fn scratch_dir() -> std::path::PathBuf {
    SCRATCH.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_else(std::env::temp_dir)
}
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

/// 一行诊断信息（性能数字、加速器的选择）：Android 上写进系统日志（tag "FleetingGleam"，`adb logcat -s FleetingGleam`），
/// 别处写 stderr。Android 的 stderr 没人看，以前那些 eprintln 在手机上全丢了。
pub fn log(msg: &str) {
    #[cfg(target_os = "android")]
    {
        extern "C" {
            fn __android_log_write(prio: i32, tag: *const std::os::raw::c_char, text: *const std::os::raw::c_char) -> i32;
        }
        let tag = std::ffi::CString::new("FleetingGleam").unwrap();
        if let Ok(text) = std::ffi::CString::new(msg.replace('\0', " ")) {
            // 4 = ANDROID_LOG_INFO
            unsafe { __android_log_write(4, tag.as_ptr(), text.as_ptr()) };
        }
    }
    #[cfg(not(target_os = "android"))]
    eprintln!("{msg}");
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
