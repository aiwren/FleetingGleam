//! FleetingGleam 核心库给 Kotlin 的接口（UniFFI）。路线图阶段 B 第 1、2 步的入口：库版本与机器信息、列出一台 NAS 的共享、
//! 模型包（状态 / 大小 / 下载 / 删除）、对一个本地文件夹做一次完整分析（分组、打分、取舍；有模型就用 CLIP 打分，有人脸包就找脸量眼睛）。
//! 后面各步按功能对齐清单往这里加。
//!
//! 约定：路径都用 `String`（Kotlin 侧传应用私有目录或 SAF 解析出来的真实路径）；错误统一成 `FgError`，消息给界面直接显示。

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use fg_core::analyze::{self, EyeMethod};
use fg_core::decide::{self, PickMode};
use fg_core::platform::SystemVision;
use fg_core::scorer::{AiScorer, ClipScorer, Quality};
use fg_core::{facepipe, faces, mediapipe, modelpacks, nas};
use rayon::prelude::*;

uniffi::setup_scaffolding!("fleetinggleam");

/// 核心库报出来的错误：只有一条消息，界面直接显示（`flat_error`：Kotlin 的 `message` 就是这条消息本身）。
#[derive(Debug, uniffi::Error)]
#[uniffi(flat_error)]
pub enum FgError {
    Failed(String),
}

impl std::fmt::Display for FgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FgError::Failed(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for FgError {}

impl From<String> for FgError {
    fn from(msg: String) -> Self {
        FgError::Failed(msg)
    }
}

impl From<&str> for FgError {
    fn from(msg: &str) -> Self {
        FgError::Failed(msg.to_string())
    }
}

/// 应用启动时叫一次：告诉核心临时文件放哪（Android 上系统的临时目录应用写不了），以及系统的 HTTP 代理（`http://主机:端口`，没有就空）。
#[uniffi::export]
pub fn init(scratch_dir: String, proxy: Option<String>) {
    fg_core::platform::set_scratch_dir(scratch_dir.into());
    fg_core::platform::set_proxy(proxy);
}

/// 核心库的版本（与应用版本一致）。
#[uniffi::export]
pub fn core_version() -> String {
    fg_core::VERSION.to_string()
}

/// 这台机器的情况，界面据此选默认的分析档位。
#[derive(uniffi::Record)]
pub struct Machine {
    pub memory_gb: u64,
    pub cores: u32,
    /// 这个平台的模型加速器："nnapi"、"coreml" 或 "cpu"。
    pub accelerator: String,
}

#[uniffi::export]
pub fn machine_info() -> Machine {
    Machine { memory_gb: fg_core::platform::physical_memory() >> 30, cores: std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1), accelerator: fg_core::onnx::accelerator().to_string() }
}

/// 排查联网问题用：用核心库同一套 HTTP 客户端（ureq + rustls）请求一个地址，返回状态码或错误；设置页"诊断"以后会用它。
#[uniffi::export]
pub fn net_probe(url: String) -> Result<u16, FgError> {
    let agent = fg_core::platform::http_agent(std::time::Duration::from_secs(15), std::time::Duration::from_secs(15));
    match agent.head(&url).call() {
        Ok(r) => Ok(r.status()),
        Err(ureq::Error::Status(code, _)) => Ok(code),
        Err(e) => Err(format!("{e}").into()),
    }
}

// ---------------------------------------------------------------- NAS

/// NAS 连接池：整个进程一个，连接住在它自己的线程里（见核心库 nas.rs）。
fn nas_pool() -> &'static nas::Pool {
    static POOL: OnceLock<nas::Pool> = OnceLock::new();
    POOL.get_or_init(nas::Pool::start)
}

/// "测试连接"：连上这台服务器，返回它的共享名；什么都不记。
#[uniffi::export]
pub fn nas_probe(host: String, user: String, password: String) -> Result<Vec<String>, FgError> {
    let host = nas::check_host(&host)?;
    Ok(nas_pool().probe(&host, user.trim(), &password)?)
}

// ---------------------------------------------------------------- 模型包

/// 一个模型包的状态：装没装、还要下载多少字节。
#[derive(uniffi::Record)]
pub struct PackStatus {
    pub id: String,
    pub installed: bool,
    /// 还要下载的字节数（装好了是 0；清单读不到时也是 0）。
    pub bytes: u64,
}

/// 全部模型包（与 Mac 端同一份清单、同样的 SHA-256 校验）。
#[uniffi::export]
pub fn packs_status(models_dir: String) -> Vec<PackStatus> {
    let dir = Path::new(&models_dir);
    modelpacks::PACKS
        .iter()
        .map(|p| {
            let installed = modelpacks::installed(dir, p);
            PackStatus { id: p.id.to_string(), installed, bytes: if installed { 0 } else { modelpacks::manifest_sizes(dir, p.id).unwrap_or(0) } }
        })
        .collect()
}

/// 下载进度，Kotlin 侧实现。
#[uniffi::export(foreign)]
pub trait DownloadProgress: Send + Sync {
    fn on_progress(&self, pack: String, file: String, done: u64, total: u64);
}

static DOWNLOAD_CANCEL: AtomicBool = AtomicBool::new(false);
static DOWNLOAD_BUSY: Mutex<Option<String>> = Mutex::new(None);

/// 下载一个包缺的文件（一次只能下一个包）；`pack_cancel` 可以中途停，下了一半的文件会删掉。
#[uniffi::export]
pub fn pack_download(models_dir: String, id: String, progress: Arc<dyn DownloadProgress>) -> Result<(), FgError> {
    {
        let mut busy = DOWNLOAD_BUSY.lock().unwrap_or_else(|e| e.into_inner());
        if busy.is_some() {
            return Err("another download is running".into());
        }
        *busy = Some(id.clone());
        DOWNLOAD_CANCEL.store(false, Ordering::Relaxed);
    }
    let r = modelpacks::download(Path::new(&models_dir), &id, &DOWNLOAD_CANCEL, &|p| progress.on_progress(p.pack, p.file, p.done, p.total));
    *DOWNLOAD_BUSY.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(r?)
}

#[uniffi::export]
pub fn pack_cancel() {
    DOWNLOAD_CANCEL.store(true, Ordering::Relaxed);
}

#[uniffi::export]
pub fn pack_remove(models_dir: String, id: String) -> Result<(), FgError> {
    Ok(modelpacks::remove(Path::new(&models_dir), &id)?)
}

// ---------------------------------------------------------------- 分析

/// 分析进度，Kotlin 侧实现，每分析完一张叫一次。
#[uniffi::export(foreign)]
pub trait ScanProgress: Send + Sync {
    fn on_photo(&self, done: u32, total: u32);
}

/// 一张照片的分析与取舍结果（界面要的那部分；完整的 `Photo` 在后面的步骤里才整个暴露）。
#[derive(uniffi::Record)]
pub struct PhotoVerdict {
    pub path: String,
    pub name: String,
    /// 分析时写到 `cache_dir` 的 640 px 缩略图。
    pub thumb: String,
    pub width: u32,
    pub height: u32,
    pub score: f32,
    pub sharp_score: f32,
    pub expo_score: f32,
    /// CLIP + 美学头，没有模型时为空。
    pub clip_score: Option<f32>,
    pub face_count: u32,
    /// 最闭的那双眼的睁开程度（有人脸包时才有）。
    pub eyes_open: Option<f32>,
    pub group: u32,
    pub group_best: bool,
    /// `keep` / `review` / `reject`。
    pub state: String,
    pub flags: Vec<String>,
}

/// 一次分析用了什么：界面的"分析信息"。
#[derive(uniffi::Record)]
pub struct ScanInfo {
    /// CLIP 跑在哪："cpu-int8"、"cpu-fp16"、"nnapi-fp16"……；没有模型时为空。
    pub clip_backend: Option<String>,
    pub clip_model: Option<String>,
    /// 找脸与量眼睛的流水线（`detect` + `eyes` 包）是否在用。
    pub faces: bool,
    /// 人物识别（`faces` 包）是否在用。
    pub people: bool,
    pub photos: Vec<PhotoVerdict>,
}

/// 对一个文件夹做一次完整分析：列出照片、逐张分析（有模型就用 CLIP 打分；有 `detect` + `eyes` 包就找脸、量眼睛；有 `faces` 包就认人）、
/// 分组打分、按保留比例取舍。`quality` 是 "fast" / "high" / "ultra"（没有对应的模型就退到有的那档）。
/// 这是冒烟用的直通版本：不走分析缓存、不同步 NAS、不能取消；桌面端 `scan_project` 的完整流程在阶段 B 第 3 步搬进核心库后两端共用。
#[uniffi::export]
pub fn analyze_folder(dir: String, cache_dir: String, models_dir: String, quality: String, keep_ratio: f32, progress: Arc<dyn ScanProgress>) -> Result<ScanInfo, FgError> {
    let cache_dir = Path::new(&cache_dir);
    std::fs::create_dir_all(cache_dir).map_err(|e| format!("{}: {e}", cache_dir.display()))?;
    let models = Path::new(&models_dir);
    let files = fg_core::list_all(&[dir]);
    if files.is_empty() {
        return Err("no photos in that folder".into());
    }
    // 模型都是可选的：没下载就没有那项信号，分析照常进行
    let scorer = ClipScorer::load(models, true, Quality::parse(Some(&quality))).map_err(|e| eprintln!("AI disabled: {e}")).ok();
    let vision = if facepipe::available(models) { facepipe::FacePipeline::load(models).map_err(|e| eprintln!("faces disabled: {e}")).ok() } else { None };
    let mp = if mediapipe::available(models) { mediapipe::MediapipeEyes::load(models).ok() } else { None };
    let fe = if faces::available(models) { faces::FaceEmbedder::load(models).ok() } else { None };
    let total = files.len() as u32;
    let done = AtomicU32::new(0);
    let mut photos: Vec<analyze::Photo> = files
        .par_iter()
        .filter_map(|p| {
            let r = analyze::analyze(p, cache_dir, scorer.as_ref().map(|s| s as &dyn AiScorer), vision.as_ref().map(|v| v as &dyn SystemVision), EyeMethod::Mediapipe, mp.as_ref(), fe.as_ref(), &|_, _, _| {});
            progress.on_photo(done.fetch_add(1, Ordering::Relaxed) + 1, total);
            r.map_err(|e| eprintln!("skip: {e}")).ok()
        })
        .collect();
    photos.sort_by(|a, b| a.path.cmp(&b.path));
    analyze::group_and_score(&mut photos, None);
    let verdicts = decide::decide(&photos, keep_ratio, PickMode::Best, false, &Default::default());
    let out = photos
        .into_iter()
        .map(|p| {
            let state = verdicts.get(&p.path).map(|d| match d.state { decide::State::Keep => "keep", decide::State::Review => "review", decide::State::Reject => "reject" }).unwrap_or_default().to_string();
            PhotoVerdict { path: p.path, name: p.name, thumb: p.thumb, width: p.width, height: p.height, score: p.score, sharp_score: p.sharp_score, expo_score: p.expo_score, clip_score: p.clip_score, face_count: p.face_count, eyes_open: p.eyes_open, group: p.group as u32, group_best: p.group_best, state, flags: p.flags }
        })
        .collect();
    Ok(ScanInfo { clip_backend: scorer.as_ref().map(|s| s.backend.to_string()), clip_model: scorer.as_ref().map(|s| s.model.to_string()), faces: vision.is_some(), people: fe.is_some(), photos: out })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Quiet;
    impl ScanProgress for Quiet {
        fn on_photo(&self, _: u32, _: u32) {}
    }

    /// 没有任何模型、没有任何平台信号时，一个文件夹也能分析出分组与取舍：这正是 Android 端第一次跑起来时的状态。
    #[test]
    fn a_folder_is_analysed_without_models_or_platform_signals() {
        let dir = std::env::temp_dir().join(format!("fg-android-smoke-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("photos")).unwrap();
        for (i, blur) in [0.0f32, 0.0, 6.0].iter().enumerate() {
            let img = image::RgbImage::from_fn(640, 480, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x * y) % 256) as u8]));
            let img = if *blur > 0.0 { image::imageops::blur(&img, *blur) } else { img };
            img.save(dir.join("photos").join(format!("p{i}.jpg"))).unwrap();
        }
        let models = dir.join("models").to_string_lossy().into_owned();
        let out = analyze_folder(dir.join("photos").to_string_lossy().into_owned(), dir.join("cache").to_string_lossy().into_owned(), models.clone(), "fast".into(), 0.5, Arc::new(Quiet)).unwrap();
        assert_eq!(out.photos.len(), 3);
        assert!(out.photos.iter().all(|p| Path::new(&p.thumb).exists()));
        assert!(out.photos.iter().any(|p| p.state == "keep"), "{:?}", out.photos.iter().map(|p| &p.state).collect::<Vec<_>>());
        assert!(out.photos.iter().all(|p| p.clip_score.is_none()) && out.clip_backend.is_none() && !out.faces, "no models in that folder");
        // 模型包：一个都没装，每个都说要下载多少
        let packs = packs_status(models);
        assert_eq!(packs.len(), modelpacks::PACKS.len());
        assert!(packs.iter().all(|p| !p.installed && p.bytes > 0), "{:?}", packs.iter().map(|p| (p.id.clone(), p.bytes)).collect::<Vec<_>>());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
