//! FleetingGleam 的核心库：桌面端（Tauri）与 Android 端（UniFFI）共用的全部逻辑。
//!
//! 这里没有任何窗口、事件或平台 API：分析（`analyze`）、分组与打分（`analyze::group_and_score`、`scoring`）、取舍（`decide`）、
//! 分析缓存（`cache`）、喜好学习（`prefs`、`taste`）、照片搜索（`textsearch`、`colorsig`）、场景命名与描述（`scene`、`scene_vocab`、
//! `florence`、`translate`）、模型文件与按需下载（`modelfiles`、`modelpacks`）、人脸（`faces`、`mediapipe`）、NAS 直连（`nas`）、
//! 云备份（`r2`）、本机加密仓库（`secrets`）。平台提供的能力（系统图像解码器、系统视觉信号、GPU 上的 CLIP）在 `platform` 里
//! 定义成 trait，由宿主注册实现；什么都不注册也能跑。

pub mod analyze;
pub mod cache;
pub mod colorsig;
pub mod consistency;
pub mod decide;
pub mod diagnostics;
pub mod engine;
pub mod errors;
pub mod facepipe;
pub mod faces;
pub mod files;
pub mod florence;
pub mod imageio;
pub mod media;
pub mod mediapipe;
pub mod modelfiles;
pub mod modelpacks;
pub mod nas;
pub mod onnx;
pub mod platform;
pub mod prefs;
pub mod r2;
pub mod scene;
pub mod scene_vocab;
pub mod scorer;
pub mod scoring;
pub mod scratch;
pub mod secrets;
pub mod taste;
pub mod textsearch;
pub mod translate;
pub mod xmp;

pub use files::{is_junk, list_all, list_all_with, list_images, oriented_dims, quarantine_name, unique_path, write_synced, MIN_PHOTO_BYTES};

/// 这个库的版本（与应用版本一致）。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 只在测试里用：开发机上的模型目录（`apps/desktop/models/`，不进仓库；见 CONTRIBUTING.md）。
#[cfg(test)]
pub(crate) fn dev_models_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/models")
}
