//! FleetingGleam 核心库给 Kotlin 的接口（UniFFI）。路线图阶段 B 第 1 步只放冒烟需要的几个入口：库版本、机器信息、
//! 列出一台 NAS 的共享、对一个本地文件夹做一次完整分析（分组、打分、取舍）。后面各步按功能对齐清单往这里加。
//!
//! 约定：路径都用 `String`（Kotlin 侧传应用私有目录或 SAF 解析出来的真实路径）；错误统一成 `FgError`，消息给界面直接显示。

use std::path::Path;
use std::sync::{Arc, OnceLock};

use fg_core::analyze::{self, EyeMethod};
use fg_core::decide::{self, PickMode};
use fg_core::scorer::{AiScorer, ClipScorer, Quality};
use fg_core::{faces, mediapipe, nas};
use rayon::prelude::*;

uniffi::setup_scaffolding!("fleetinggleam");

/// 核心库报出来的错误：只有一条消息，界面直接显示。
#[derive(Debug, uniffi::Error)]
pub enum FgError {
    Failed { msg: String },
}

impl std::fmt::Display for FgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FgError::Failed { msg } => f.write_str(msg),
        }
    }
}

impl std::error::Error for FgError {}

impl From<String> for FgError {
    fn from(msg: String) -> Self {
        FgError::Failed { msg }
    }
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
}

#[uniffi::export]
pub fn machine_info() -> Machine {
    Machine { memory_gb: fg_core::platform::physical_memory() >> 30, cores: std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1) }
}

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
    pub group: u32,
    pub group_best: bool,
    /// `keep` / `review` / `reject`。
    pub state: String,
    pub flags: Vec<String>,
}

/// 对一个文件夹做一次完整分析：列出照片、逐张分析（有模型就用 CLIP 打分，有 MediaPipe 与人脸模型就用）、分组打分、按保留比例取舍。
/// 这是冒烟用的直通版本：不走分析缓存、不同步 NAS、不能取消；桌面端 `scan_project` 的完整流程在阶段 B 第 3 步搬进核心库后两端共用。
#[uniffi::export]
pub fn analyze_folder(dir: String, cache_dir: String, models_dir: String, keep_ratio: f32, progress: Arc<dyn ScanProgress>) -> Result<Vec<PhotoVerdict>, FgError> {
    let cache_dir = Path::new(&cache_dir);
    std::fs::create_dir_all(cache_dir).map_err(|e| format!("{}: {e}", cache_dir.display()))?;
    let models = Path::new(&models_dir);
    let files = fg_core::list_all(&[dir]);
    if files.is_empty() {
        return Err("no photos in that folder".to_string().into());
    }
    // 模型都是可选的：没下载就没有那项信号，分析照常进行
    let scorer = ClipScorer::load(models, true, Quality::Fast).map_err(|e| eprintln!("AI disabled: {e}")).ok();
    let mp = if mediapipe::available(models) { mediapipe::MediapipeEyes::load(models).ok() } else { None };
    let fe = if faces::available(models) { faces::FaceEmbedder::load(models).ok() } else { None };
    let total = files.len() as u32;
    let done = std::sync::atomic::AtomicU32::new(0);
    let mut photos: Vec<analyze::Photo> = files
        .par_iter()
        .filter_map(|p| {
            let r = analyze::analyze(p, cache_dir, scorer.as_ref().map(|s| s as &dyn AiScorer), None, EyeMethod::Mediapipe, mp.as_ref(), fe.as_ref(), &|_, _, _| {});
            progress.on_photo(done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1, total);
            r.map_err(|e| eprintln!("skip: {e}")).ok()
        })
        .collect();
    photos.sort_by(|a, b| a.path.cmp(&b.path));
    analyze::group_and_score(&mut photos, None);
    let verdicts = decide::decide(&photos, keep_ratio, PickMode::Best, false, &Default::default());
    Ok(photos
        .into_iter()
        .map(|p| {
            let state = verdicts.get(&p.path).map(|d| match d.state { decide::State::Keep => "keep", decide::State::Review => "review", decide::State::Reject => "reject" }).unwrap_or_default().to_string();
            PhotoVerdict { path: p.path, name: p.name, thumb: p.thumb, width: p.width, height: p.height, score: p.score, sharp_score: p.sharp_score, expo_score: p.expo_score, clip_score: p.clip_score, group: p.group as u32, group_best: p.group_best, state, flags: p.flags }
        })
        .collect())
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
        let out = analyze_folder(dir.join("photos").to_string_lossy().into_owned(), dir.join("cache").to_string_lossy().into_owned(), dir.join("models").to_string_lossy().into_owned(), 0.5, Arc::new(Quiet)).unwrap();
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|p| Path::new(&p.thumb).exists()));
        assert!(out.iter().any(|p| p.state == "keep"), "{:?}", out.iter().map(|p| &p.state).collect::<Vec<_>>());
        assert!(out.iter().all(|p| p.clip_score.is_none()), "no models in that folder");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
