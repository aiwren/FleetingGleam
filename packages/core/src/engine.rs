//! 引擎：应用层的全部流程，桌面端（Tauri 命令）与 Android（UniFFI）共用同一份。
//!
//! 以前这些都写在桌面端的 `lib.rs` 里，和 Tauri 的 `AppHandle` 缠在一起：探测与分析一个项目（缓存命中、NAS 同步、分批落盘、
//! 补测眼睛 / 人脸 / HDR、取消）、取舍、项目文件、我的设置与喜好的导入导出、云备份、NAS、整理（导出 / 回收）、查看器要的大图与
//! 局部、照片搜索、场景命名、模型包、喜好学习与风格、缓存。现在它们是 `Engine` 的方法：所有方法都是阻塞的（宿主自己放到后台线程），
//! 进度经 `Events` 发出去，事件名与载荷和桌面端前端原来监听的完全一样。
//!
//! 平台差异只剩三处，都走 `platform`：系统视觉信号（`VisionFactory`）、本机废纸篓（`LocalTrash`）、系统图像解码器（`SystemImages`）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::analyze::{self, EyeMethod, Photo};
use crate::platform::SystemVision;
use crate::scorer::{self, AiScorer, ClipScorer, Quality};
use crate::{cache, colorsig, decide, faces, florence, imageio, media, mediapipe, modelpacks, nas, prefs, r2, scene_vocab, scratch, taste, textsearch, translate};
use crate::{list_all_with, list_images, oriented_dims, quarantine_name, unique_path, write_synced};

// ---------------------------------------------------------------- 宿主给的东西

/// 引擎往界面发的事件。`payload` 是 JSON：桌面端原样交给 WebView，Android 交给 Kotlin。
pub trait Events: Send + Sync {
    fn emit(&self, event: &str, payload: serde_json::Value);
}

/// 不发任何事件（测试、命令行用）。
pub struct NoEvents;

impl Events for NoEvents {
    fn emit(&self, _: &str, _: serde_json::Value) {}
}

/// 引擎用到的目录。
#[derive(Clone, Debug)]
pub struct Paths {
    /// 应用数据：`projects.json`、`styles.json`、分析缓存、NAS 与备份的设置、加密仓库、NAS 镜像。
    pub data: PathBuf,
    /// 缩略图与分析副本，以及它下面的 `large/`、`detail/`、动态照片的视频；清除缓存就是清它。
    pub thumbs: PathBuf,
    /// 从哪里读模型（开发时可能是仓库里的 `models/`）。
    pub models: PathBuf,
    /// 模型包下载到哪里。
    pub downloads: PathBuf,
}

// ---------------------------------------------------------------- 发给界面的数据（字段名就是前端读的名字）

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
}

#[derive(Serialize, Clone)]
struct Stage<'a> {
    path: &'a str,
    stage: &'a str,
    pct: u8,
    thumb: Option<&'a str>,
    width: Option<u32>,
    height: Option<u32>,
}

/// 这次分析实际用了什么。
#[derive(Serialize, Clone)]
pub struct Info {
    pub cpu_only: bool,
    pub clip: Option<&'static str>,
    pub model: Option<&'static str>,
    pub vision: bool,
    pub cores: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProbeFile {
    pub path: String,
    /// Displayed size (EXIF orientation applied), read from the file header without decoding pixels.
    pub width: u32,
    pub height: u32,
    /// True when a valid analysis for this exact file is already cached.
    pub cached: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Probe {
    pub files: Vec<ProbeFile>,
    pub bytes: u64,
    pub uncached: usize,
}

/// 分析一个项目要的参数（和桌面端 `scan_project` 命令的参数一样）。
#[derive(Deserialize, Clone, Default, Debug)]
pub struct ScanArgs {
    /// 项目的标识：`decide_photos` 用它确认手里的照片是哪个项目的。
    pub key: String,
    pub dirs: Vec<String>,
    /// "cpu" = 只用 CPU。
    #[serde(default)]
    pub compute: Option<String>,
    /// "fast" / "high" / "ultra"。
    #[serde(default)]
    pub quality: Option<String>,
    /// 混入学到的喜好。
    #[serde(default)]
    pub learn: bool,
    /// 不发进度事件（瞬间重开、重新打分时）。
    #[serde(default)]
    pub quiet: bool,
    #[serde(default)]
    pub style: Option<String>,
    /// 不管缓存，全部重新分析。
    #[serde(default)]
    pub force: bool,
    /// "vision" / "mediapipe"。
    #[serde(default)]
    pub eye_method: Option<String>,
}

/// 设置页看到的一个备份目标；从不带 Secret。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TargetStatus {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub account: String,
    pub bucket: String,
    pub access_key: String,
    pub has_secret: bool,
    pub last_ok: u64,
    pub last_error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupStatus {
    /// 至少有一个能用的目标。
    pub configured: bool,
    pub enabled: bool,
    /// 自动备份保留的份数。
    pub keep: usize,
    pub targets: Vec<TargetStatus>,
}

/// 添加或修改一个备份目标的参数（`id` 为空 = 添加；`secret` 留空 = 沿用记住的）。
#[derive(Deserialize, Clone, Default, Debug)]
pub struct TargetArgs {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: String,
    pub kind: String,
    pub account: String,
    pub bucket: String,
    pub access_key: String,
    #[serde(default)]
    pub secret: Option<String>,
}

/// 浏览 NAS 上的一个位置（新建项目时选文件夹用）：根是共享列表。
#[derive(Serialize)]
pub struct Listing {
    /// `共享/文件夹/…`（`/` 分隔，根是空串）。
    pub path: String,
    /// 这个文件夹在镜像里的本地路径：选了它，项目的文件夹就是这个。根（还没选共享）时为空，不能当项目文件夹。
    pub local: String,
    pub entries: Vec<nas::Entry>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExportResult {
    pub copied: usize,
    pub skipped: usize,
    pub dest: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TrashResult {
    pub moved: usize,
    pub failed: Vec<String>,
    /// Where photos on network volumes went instead of the Trash (one entry per volume), for the message.
    pub network_bins: Vec<String>,
    /// 本机的照片，但宿主没有注册 `LocalTrash`（Android）：由界面自己放进系统回收站（要用户在系统确认框里同意）。
    #[serde(default)]
    pub deferred: Vec<String>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct FileInfo {
    pub format: Option<String>,
    pub bytes: u64,
    pub color_space: Option<String>,
    pub bit_depth: Option<String>,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub focal: Option<String>,
    pub aperture: Option<String>,
    pub shutter: Option<String>,
    pub iso: Option<String>,
    pub software: Option<String>,
    pub taken: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchStatus {
    pub total: usize,
    pub ready: usize,
    /// The text model is present, so searching is possible at all.
    pub available: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Hit {
    pub path: String,
    pub name: String,
    pub thumb: String,
    pub width: u32,
    pub height: u32,
    pub score: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SceneReq {
    pub id: usize,
    /// The scene's photos, best first.
    pub paths: Vec<String>,
}

#[derive(Serialize, Clone)]
struct SceneText {
    id: usize,
    text: String,
}

/// What the machine is, so that the app can pick the analysis that suits it without asking.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Machine {
    pub memory_gb: u64,
    pub cores: usize,
    /// 这个平台的模型加速器："coreml"、"nnapi" 或 "cpu"。
    #[serde(default)]
    pub accelerator: String,
}

// ---------------------------------------------------------------- 引擎

#[derive(Default)]
struct Downloads {
    cancel: AtomicBool,
    busy: Mutex<Option<String>>,
}

/// Shared, lazily loaded state: the analysis cache and the on-device taste model, and everything else that lives as long as the app.
pub struct Engine {
    paths: Paths,
    events: Box<dyn Events>,
    /// 写进"我的设置与喜好"文件里的应用版本。
    app_version: String,
    text: Mutex<Option<Arc<textsearch::TextEncoder>>>,
    cache: Mutex<Option<cache::Cache>>,
    prefs: Mutex<prefs::Styles>,
    /// Loaded once, on first use: the eye-openness models are a few MB each, not worth reopening per scan.
    mediapipe_eyes: Mutex<Option<Arc<mediapipe::MediapipeEyes>>>,
    /// Loaded once, on first use: the face-recognition network is tens of MB.
    face_embedder: Mutex<Option<Arc<faces::FaceEmbedder>>>,
    /// The last project's photos (with the project key they belong to), kept so `decide_photos` can re-run keep/review/reject
    /// on a plain override change without the whole (possibly large) list crossing the boundary again. The key stops a
    /// scan that finished late (of a project since left) from answering for the project now open.
    last_photos: Mutex<(String, Vec<Photo>)>,
    /// Bumped by every scan and by `scan_cancel`, so a scan that is no longer the current one stops at the next photo.
    scan_job: AtomicU64,
    /// Bumped by every recorded label; the retraining scheduled by a label only runs if no newer label came in the meantime.
    retrain_gen: AtomicU64,
    /// Loaded once, on first use: the scene-description model (a few hundred MB) and the English-to-Chinese one.
    captioner: Mutex<Option<Arc<florence::Captioner>>>,
    translator: Mutex<Option<Arc<translate::Translator>>>,
    /// The English sentence for each photo already described (keyed by its thumbnail), kept in memory and on disk.
    scene_sentences: Mutex<Option<HashMap<String, String>>>,
    /// Bumped by every request to describe scenes, so an older one stops as soon as a newer one arrives.
    scene_job: AtomicU64,
    /// NAS 共享的连接，住在自己的线程里（见 nas.rs）。
    nas: nas::Pool,
    downloads: Downloads,
    /// Writes of `projects.json` are serialised and go through a temporary file and a rename.
    projects_write: Mutex<()>,
    /// 这台设备的名字（备份里记下来，另一端恢复前能看到是谁做的）；应用启动时告诉引擎，没告诉就用平台名。
    device: Mutex<String>,
}

/// 这一端是哪种设备（备份的来源说明里显示）。
pub const PLATFORM: &str = if cfg!(target_os = "android") {
    "android"
} else if cfg!(target_os = "macos") {
    "mac"
} else if cfg!(target_os = "windows") {
    "windows"
} else {
    "other"
};

/// 桶里"最新"那份备份的说明。
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct BackupMeta {
    pub device: String,
    pub platform: String,
    pub exported_at: u64,
    pub app_version: String,
    pub styles: usize,
    pub examples: usize,
}

impl Engine {
    pub fn new(paths: Paths, events: Box<dyn Events>, app_version: &str) -> Arc<Engine> {
        let _ = std::fs::create_dir_all(&paths.data);
        let _ = std::fs::create_dir_all(&paths.thumbs);
        let prefs = prefs::Styles::load(&paths.data);
        Arc::new(Engine {
            paths,
            events,
            app_version: app_version.to_string(),
            text: Mutex::new(None),
            cache: Mutex::new(None),
            prefs: Mutex::new(prefs),
            mediapipe_eyes: Mutex::new(None),
            face_embedder: Mutex::new(None),
            last_photos: Mutex::new((String::new(), Vec::new())),
            scan_job: AtomicU64::new(0),
            retrain_gen: AtomicU64::new(0),
            captioner: Mutex::new(None),
            translator: Mutex::new(None),
            scene_sentences: Mutex::new(None),
            scene_job: AtomicU64::new(0),
            nas: nas::Pool::start(),
            downloads: Downloads::default(),
            projects_write: Mutex::new(()),
            device: Mutex::new(String::new()),
        })
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    /// 这台设备的名字（Mac 的电脑名、手机的型号），写进备份。
    pub fn set_device(&self, name: &str) {
        *self.device.lock().unwrap_or_else(|e| e.into_inner()) = name.trim().to_string();
    }

    fn device_name(&self) -> String {
        let d = self.device.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if d.is_empty() { PLATFORM.to_string() } else { d }
    }

    fn emit<T: Serialize + ?Sized>(&self, event: &str, payload: &T) {
        if let Ok(v) = serde_json::to_value(payload) {
            self.events.emit(event, v);
        }
    }

    fn with_cache<R>(&self, f: impl FnOnce(&mut cache::Cache) -> R) -> R {
        let mut guard = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let c = guard.get_or_insert_with(|| cache::Cache::load(self.paths.data.join("analysis-cache.json")));
        f(c)
    }

    /// The MediaPipe eye models, loaded once and kept: `None` only when the pack has not been downloaded, or failed to load.
    fn mediapipe_eyes(&self) -> Option<Arc<mediapipe::MediapipeEyes>> {
        let mut guard = self.mediapipe_eyes.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() && mediapipe::available(&self.paths.models) {
            match mediapipe::MediapipeEyes::load(&self.paths.models) {
                Ok(m) => *guard = Some(Arc::new(m)),
                Err(e) => eprintln!("MediaPipe eyes disabled: {e}"),
            }
        }
        guard.clone()
    }

    /// The face-recognition network, loaded once and kept: `None` when it has not been downloaded or failed to load.
    fn face_embedder(&self) -> Option<Arc<faces::FaceEmbedder>> {
        let mut guard = self.face_embedder.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() && faces::available(&self.paths.models) {
            match faces::FaceEmbedder::load(&self.paths.models) {
                Ok(m) => *guard = Some(Arc::new(m)),
                Err(e) => eprintln!("face recognition disabled: {e}"),
            }
        }
        guard.clone()
    }

    /// 起一套系统视觉信号（宿主注册的工厂）；没有就 None。
    fn start_vision(&self, workers: usize, cpu_only: bool) -> Option<Box<dyn SystemVision>> {
        crate::platform::vision()?.start(&self.paths.models, workers, cpu_only)
    }

    /// 启动后在后台做的事：把启用的 NAS 都连上，修剪缩略图旁边的草稿文件。
    pub fn start_background(self: &Arc<Self>) {
        let e = self.clone();
        std::thread::spawn(move || {
            for s in nas::load(&e.paths.data).into_iter().filter(|s| s.enabled) {
                if let Err(err) = e.nas_connect_saved(&s) {
                    eprintln!("NAS {}: {err}", s.name);
                }
            }
            e.emit("nas-changed", &());
        });
        let thumbs = self.paths.thumbs.clone();
        std::thread::spawn(move || scratch::prune(&thumbs));
    }

    // ------------------------------------------------------------ 探测与分析

    /// Cheap pre-flight for a set of folders: every photo's true displayed size (so the wall can be laid out
    /// once, at final size, before any analysis) and whether it is already cached.
    pub fn probe_folder(self: &Arc<Self>, dirs: &[String], compute: Option<&str>, quality: Option<&str>) -> Result<Probe, String> {
        let tag = scorer::analysis_tag(scorer::resolve(&self.paths.models, compute != Some("cpu"), Quality::parse(quality)));
        // 离开项目时页面会 `scan_cancel`，正在从 NAS 拉照片的探测也就跟着停
        let job = self.scan_job.load(Ordering::Relaxed);
        self.nas_sync_dirs(dirs, job)?;
        // 缓存里认识的文件（没变过）不读文件头：尺寸直接查表，是否"已分析"再按当前模型的 tag 看
        let (files, known): (Vec<PathBuf>, Vec<(Option<(u32, u32)>, bool)>) = self.with_cache(|c| {
            let files = list_all_with(dirs, |p| cache::file_key(p).map_or(false, |(sz, mt)| c.dims(&p.to_string_lossy(), sz, mt).is_some()));
            let known = files
                .iter()
                .map(|p| {
                    let key = p.to_string_lossy();
                    match cache::file_key(p) {
                        Some((sz, mt)) => (c.dims(&key, sz, mt), c.has(&key, sz, mt, &tag)),
                        None => (None, false),
                    }
                })
                .collect();
            (files, known)
        });
        let out: Vec<ProbeFile> = files
            .par_iter()
            .zip(known.par_iter())
            .map(|(p, &(dims, cached))| {
                let (width, height) = dims.or_else(|| oriented_dims(p)).unwrap_or((3, 2));
                ProbeFile { path: p.to_string_lossy().into_owned(), width, height, cached }
            })
            .collect();
        // Size of what still needs analysing, which is what the confirmation screen talks about.
        let bytes = files.iter().zip(&out).filter(|(_, o)| !o.cached).filter_map(|(p, _)| std::fs::metadata(p).ok()).map(|m| m.len()).sum();
        let uncached = out.iter().filter(|f| !f.cached).count();
        Ok(Probe { files: out, bytes, uncached })
    }

    /// Analyses whatever is new or changed, reuses the cache for the rest, then groups and scores every photo
    /// together. `key` names the project (so `decide_photos` can tell whose photos it holds); `learn` blends in the on-device
    /// taste model; `quiet` suppresses progress events (used for instant re-opens and re-scoring); `force` ignores the cache
    /// and re-analyses every photo. A newer scan, or `scan_cancel`, stops this one at the next photo with `Err("cancelled")`;
    /// what it analysed by then is already in the cache.
    pub fn scan_project(self: &Arc<Self>, a: ScanArgs) -> Result<Vec<Photo>, String> {
        let force = a.force;
        let quiet = a.quiet;
        let job = self.scan_job.fetch_add(1, Ordering::Relaxed) + 1;
        let cpu_only = a.compute.as_deref() == Some("cpu");
        let want = Quality::parse(a.quality.as_deref());
        let eye_method = EyeMethod::parse(a.eye_method.as_deref());
        let cache_dir = self.paths.thumbs.clone();
        std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
        let model_dir = self.paths.models.clone();
        let tag = scorer::analysis_tag(scorer::resolve(&model_dir, !cpu_only, want));
        let mp = if eye_method == EyeMethod::Mediapipe { self.mediapipe_eyes() } else { None };
        let fe = self.face_embedder();

        let cancelled = || self.scan_job.load(Ordering::Relaxed) != job;
        // 镜像里可能有探测之后才到 NAS 上的新照片；再对一次远端，没变化时只是几次目录列表
        self.nas_sync_dirs(&a.dirs, job)?;
        let mut hits: Vec<Photo> = Vec::new();
        let mut jobs: Vec<(PathBuf, u64, i64)> = Vec::new();
        let files = self.with_cache(|c| {
            let files = list_all_with(&a.dirs, |p| cache::file_key(p).map_or(false, |(sz, mt)| c.dims(&p.to_string_lossy(), sz, mt).is_some()));
            for f in &files {
                let key = f.to_string_lossy();
                if let Some((sz, mt)) = cache::file_key(f) {
                    match if force { None } else { c.get(&key, sz, mt, &tag) } {
                        Some(p) => hits.push(p),
                        None => jobs.push((f.clone(), sz, mt)),
                    }
                }
            }
            files
        });
        let total = files.len();

        let done = AtomicUsize::new(hits.len());
        if !quiet {
            self.emit("scan-progress", &Progress { done: hits.len(), total });
            // the photos that are already known go over in batches: one event each would flood the page on a big project
            for chunk in hits.chunks(250) {
                self.emit("scan-photos", chunk);
            }
        }

        let mut fresh: Vec<Photo> = Vec::new();
        if !jobs.is_empty() {
            // Models are only loaded when there is real work: re-opening a cached project is instant.
            let scorer = ClipScorer::load(&model_dir, !cpu_only, want).map_err(|e| eprintln!("AI disabled: {e}")).ok();
            let vision = self.start_vision(2, cpu_only);
            if !quiet {
                self.emit(
                    "scan-info",
                    &Info {
                        cpu_only,
                        clip: scorer.as_ref().map(|s| s.backend),
                        model: scorer.as_ref().map(|s| s.model),
                        vision: vision.is_some(),
                        cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
                    },
                );
            }
            // What has been analysed is saved to the cache as it goes (in batches), not only at the end: a big project takes a long time, and
            // closing the app part-way used to lose all of it.
            let pending: Mutex<(Vec<Photo>, std::time::Instant)> = Mutex::new((Vec::new(), std::time::Instant::now()));
            let save_batch = |batch: &[Photo]| {
                self.with_cache(|c| {
                    for p in batch {
                        if let Some((sz, mt)) = cache::file_key(Path::new(&p.path)) {
                            c.put(sz, mt, &tag, p);
                        }
                    }
                    c.save();
                });
            };
            fresh = jobs
                .par_iter()
                .filter_map(|(p, _, _)| {
                    if cancelled() {
                        return None;
                    }
                    let key = p.to_string_lossy().into_owned();
                    let on_stage = |stage: &str, pct: u8, thumb: Option<(&str, u32, u32)>| {
                        if quiet {
                            return;
                        }
                        self.emit("scan-stage", &Stage { path: &key, stage, pct, thumb: thumb.map(|t| t.0), width: thumb.map(|t| t.1), height: thumb.map(|t| t.2) });
                    };
                    let r = analyze::analyze(p, &cache_dir, scorer.as_ref().map(|s| s as &dyn AiScorer), vision.as_deref(), eye_method, mp.as_deref(), fe.as_deref(), &on_stage);
                    let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if !quiet {
                        if let Ok(photo) = &r {
                            self.emit("scan-photo", photo);
                        }
                        self.emit("scan-progress", &Progress { done: d, total });
                    }
                    if let Ok(photo) = &r {
                        // a batch is written once there are 100 and half a minute has passed (writing the file takes longer the bigger the
                        // project), and in any case once a minute: a small project, or a slow format, must not lose everything on a quit
                        let batch = {
                            let mut g = pending.lock().unwrap_or_else(|e| e.into_inner());
                            g.0.push(photo.clone());
                            let age = g.1.elapsed();
                            if (g.0.len() >= 100 && age >= std::time::Duration::from_secs(30)) || age >= std::time::Duration::from_secs(60) {
                                g.1 = std::time::Instant::now();
                                Some(std::mem::take(&mut g.0))
                            } else {
                                None
                            }
                        };
                        if let Some(batch) = batch {
                            save_batch(&batch);
                        }
                    }
                    r.map_err(|e| eprintln!("skip: {e}")).ok()
                })
                .collect();
            let rest = std::mem::take(&mut pending.lock().unwrap_or_else(|e| e.into_inner()).0);
            save_batch(&rest);
        }
        if cancelled() {
            return Err("cancelled".to_string());
        }

        // A photo missing eyes_open (analysed before that signal existed), or whose cached value came from the other
        // method than the one now selected, gets it (re)measured from its saved analysis copy.
        let mut hits = hits;
        if hits.iter().any(|p| p.face_count > 0 && !analyze::eyes_current(p.eye_method.as_deref(), eye_method)) {
            if let Some(v) = self.start_vision(2, cpu_only) {
                let measured: Vec<(usize, (f32, &'static str))> = hits
                    .par_iter()
                    .enumerate()
                    .filter(|(_, p)| p.face_count > 0 && !analyze::eyes_current(p.eye_method.as_deref(), eye_method) && !cancelled())
                    .filter_map(|(i, p)| analyze::measure_eyes(Path::new(&p.path), &cache_dir, &*v, eye_method, mp.as_deref()).map(|e| (i, e)))
                    .collect();
                if !measured.is_empty() {
                    for (i, (e, tag)) in &measured {
                        // no face big enough to measure is remembered as "very open", so it is not asked again
                        hits[*i].eyes_open = Some(*e);
                        hits[*i].eye_method = Some(tag.to_string());
                    }
                    self.with_cache(|c| {
                        for (i, _) in &measured {
                            if let Some((sz, mt)) = cache::file_key(Path::new(&hits[*i].path)) {
                                c.put(sz, mt, &tag, &hits[*i]);
                            }
                        }
                        c.save();
                    });
                }
            }
        }
        // Photos analysed before the recognition model was there get their faces looked at now, from the saved analysis copy.
        // This is a minute of Vision calls for a big project, so it gets a pool of its own (one thread per Vision process):
        // on the shared pool it would starve everything else that needs a worker, such as reading a folder.
        if let (Some(f), Ok(pool)) = (fe.as_deref(), rayon::ThreadPoolBuilder::new().num_threads(2).build()) {
            let total = hits.iter().filter(|p| !p.faces_done).count();
            if total > 0 {
                if let Some(v) = self.start_vision(2, cpu_only) {
                    let counter = AtomicUsize::new(0);
                    let found: Vec<(usize, Vec<[f32; 4]>, Vec<Vec<f32>>)> = pool.install(|| {
                        hits.par_iter()
                            .enumerate()
                            .filter(|(_, p)| !p.faces_done && !cancelled())
                            .filter_map(|(i, p)| {
                                let r = analyze::measure_faces(Path::new(&p.path), &cache_dir, &*v, f).map(|(b, e)| (i, b, e));
                                self.emit("people-progress", &Progress { done: counter.fetch_add(1, Ordering::Relaxed) + 1, total });
                                r
                            })
                            .collect()
                    });
                    if !found.is_empty() {
                        let changed: Vec<usize> = found.iter().map(|(i, _, _)| *i).collect();
                        for (i, boxes, embs) in found {
                            hits[i].face_boxes = boxes;
                            hits[i].face_embs = embs;
                            hits[i].faces_done = true;
                        }
                        self.with_cache(|c| {
                            for i in &changed {
                                if let Some((sz, mt)) = cache::file_key(Path::new(&hits[*i].path)) {
                                    c.put(sz, mt, &tag, &hits[*i]);
                                }
                            }
                            c.save();
                        });
                    }
                }
            }
        }
        // Photos analysed before HDR and motion were looked at: their HDR thumbnail and motion clip are made now (the helper runs one
        // process per photo, so this gets a pool of its own, like the faces above).
        if let Ok(pool) = rayon::ThreadPoolBuilder::new().num_threads(4).build() {
            let total = hits.iter().filter(|p| !p.media_done).count();
            if total > 0 {
                let counter = AtomicUsize::new(0);
                let found: Vec<(usize, media::Media)> = pool.install(|| {
                    hits.par_iter()
                        .enumerate()
                        .filter(|(_, p)| !p.media_done && !cancelled())
                        .map(|(i, p)| {
                            let m = media::probe(Path::new(&p.path), &cache_dir);
                            self.emit("media-progress", &Progress { done: counter.fetch_add(1, Ordering::Relaxed) + 1, total });
                            (i, m)
                        })
                        .collect()
                });
                let changed: Vec<usize> = found.iter().map(|(i, _)| *i).collect();
                for (i, m) in found {
                    hits[i].hdr = m.hdr;
                    hits[i].motion = m.motion;
                    hits[i].media_done = true;
                }
                self.with_cache(|c| {
                    for i in &changed {
                        if let Some((sz, mt)) = cache::file_key(Path::new(&hits[*i].path)) {
                            c.put(sz, mt, &tag, &hits[*i]);
                        }
                    }
                    c.save();
                });
            }
        }
        if cancelled() {
            return Err("cancelled".to_string());
        }
        let mut photos = hits;
        photos.extend(fresh);
        photos.sort_by(|a, b| a.path.cmp(&b.path));
        // 刚分析出来的记录是 0 度；文件没变、用户转过的，缓存里（`put` 时）已经套上了角度，这里把它拿回来
        self.with_cache(|c| {
            for p in photos.iter_mut() {
                if let Some(r) = c.photo(&p.path).map(|q| q.rotation).filter(|r| *r != p.rotation) {
                    let _ = analyze::apply_rotation(p, r, &cache_dir);
                }
            }
        });
        let prefs = if a.learn { Some(self.prefs.lock().unwrap_or_else(|e| e.into_inner()).get(a.style.as_deref()).prefs.clone()) } else { None };
        analyze::group_and_score(&mut photos, prefs.as_ref().filter(|p| p.active()));
        {
            // grouping took a while; a newer scan may have written its project meanwhile, and this one must not replace it
            let mut last = self.last_photos.lock().unwrap_or_else(|e| e.into_inner());
            if self.scan_job.load(Ordering::Relaxed) != job {
                return Err("cancelled".to_string());
            }
            *last = (a.key, photos.clone());
        }
        Ok(photos)
    }

    /// Stops the scan in progress: it ends at the next photo, and what it analysed by then is already in the cache. Called when
    /// the person asks for it, and whenever the page leaves a project (its scan may still be running).
    pub fn scan_cancel(&self) {
        self.scan_job.fetch_add(1, Ordering::Relaxed);
    }

    /// 把一张照片顺时针转到 `deg`（0 / 90 / 180 / 270）。只改缓存里的记录（缩略图、显示尺寸），原文件不动、分析结果不变；
    /// 大图与清晰块按角度另存。返回转过的记录，界面拿它替换手里那张。
    pub fn set_rotation(&self, path: &str, deg: i64) -> Result<Photo, String> {
        let deg = analyze::norm_rotation(deg);
        let cache_dir = self.paths.thumbs.clone();
        let p = self.with_cache(|c| {
            let out = {
                let p = c.photo_mut(path).ok_or_else(|| format!("{path}: not analysed yet"))?;
                analyze::apply_rotation(p, deg, &cache_dir)?;
                p.clone()
            };
            c.save();
            Ok::<Photo, String>(out)
        })?;
        let mut last = self.last_photos.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(q) = last.1.iter_mut().find(|q| q.path == path) {
            q.thumb = p.thumb.clone();
            q.width = p.width;
            q.height = p.height;
            q.rotation = p.rotation;
        }
        Ok(p)
    }

    /// 一张照片现在的角度和显示尺寸（没分析过的按 0 度）。
    fn rotation_of(&self, path: &str) -> (u16, u32, u32) {
        self.with_cache(|c| c.photo(path).map(|p| (p.rotation, p.width, p.height)).unwrap_or((0, 0, 0)))
    }

    /// Keep / review / reject for the project last returned by `scan_project`. Only the small bits that can change between calls
    /// (the keep ratio, and any photos the user overrode by hand) cross the boundary; the photos themselves stay here.
    /// `key` must be the project the photos were scanned for: otherwise the answer would be another project's and is refused.
    pub fn decide_photos(&self, key: &str, keep_ratio: f32, pick_mode: Option<&str>, one_per_group: bool, overrides: &HashMap<String, String>) -> Result<HashMap<String, decide::Decision>, String> {
        let photos = self.last_photos.lock().unwrap_or_else(|e| e.into_inner());
        if photos.0 != key {
            return Err("stale".to_string());
        }
        Ok(decide::decide(&photos.1, keep_ratio, decide::PickMode::parse(pick_mode), one_per_group, overrides))
    }

    // ------------------------------------------------------------ 项目文件

    /// The projects (named folder sets and every decision made on their photos) live in `projects.json` in the app's data folder,
    /// read whole here and written whole, atomically, by `projects_save`. The front end keeps the shape; this only keeps it safe.
    pub fn projects_load(&self) -> Result<Option<String>, String> {
        let path = self.paths.data.join("projects.json");
        match std::fs::read_to_string(&path) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Writes are serialised and go through a temporary file and a rename, so a crash mid-write leaves the previous file intact.
    pub fn projects_save(&self, json: &str) -> Result<(), String> {
        let path = self.paths.data.join("projects.json");
        let _one_at_a_time = self.projects_write.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let tmp = path.with_extension("json.tmp");
        write_synced(&tmp, json.as_bytes()).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Sets aside a `projects.json` the front end could not read (renamed with a timestamp, never deleted), so a fresh one can be started
    /// without destroying what might still be repaired by hand.
    pub fn projects_quarantine(&self) -> Result<String, String> {
        let path = self.paths.data.join("projects.json");
        let _one_at_a_time = self.projects_write.lock().unwrap_or_else(|e| e.into_inner());
        let aside = quarantine_name(&path);
        std::fs::rename(&path, &aside).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(aside.to_string_lossy().into_owned())
    }

    // ------------------------------------------------------------ 我的设置与喜好

    /// "我的设置与喜好"打成一个包（JSON 字节）：导出到文件和备份到云端用的是同一份。
    pub fn taste_pack_bytes(&self, settings: serde_json::Value) -> Result<(Vec<u8>, taste::Summary), String> {
        let styles = self.prefs.lock().unwrap_or_else(|e| e.into_inner());
        let device = self.device_name();
        let (pack, summary) = self.with_cache(|c| taste::pack(&styles, settings, &self.app_version, &device, PLATFORM, |k| c.embedding(k)));
        drop(styles);
        let bytes = serde_json::to_vec_pretty(&pack).map_err(|e| e.to_string())?;
        Ok((bytes, summary))
    }

    /// 把一个包里的喜好并进本机的风格（从文件导入和从云端恢复都走这里），返回摘要和包里带的设置。
    pub fn taste_apply(&self, pack: taste::Pack) -> Result<(taste::Summary, serde_json::Value), String> {
        let (summary, touched) = {
            let mut styles = self.prefs.lock().unwrap_or_else(|e| e.into_inner());
            taste::unpack(&mut styles, &pack)?
        };
        for id in &touched {
            self.retrain_outside_lock(Some(id));
        }
        let pending = self.prefs.lock().unwrap_or_else(|e| e.into_inner()).to_save();
        if let Some(p) = pending {
            p.write();
        }
        Ok((summary, pack.settings))
    }

    /// Writes "my settings and taste" to `path` (see `taste.rs`): the settings object the front end passes, every style's examples as
    /// embeddings, no photos and no paths.
    pub fn taste_export(&self, path: &str, settings: serde_json::Value) -> Result<taste::Summary, String> {
        let (bytes, summary) = self.taste_pack_bytes(settings)?;
        let out = PathBuf::from(path);
        let tmp = out.with_extension("json.tmp");
        write_synced(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &out).map_err(|e| format!("{}: {e}", out.display()))?;
        Ok(summary)
    }

    pub fn taste_import(&self, path: &str) -> Result<(taste::Summary, serde_json::Value), String> {
        let text = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        self.taste_import_bytes(&text).map_err(|e| format!("{path}: {e}"))
    }

    /// 从字节导入（Android 用系统的文件选择器读出来的内容，没有路径）。
    pub fn taste_import_bytes(&self, bytes: &[u8]) -> Result<(taste::Summary, serde_json::Value), String> {
        let pack: taste::Pack = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        self.taste_apply(pack)
    }

    // ------------------------------------------------------------ 云备份

    pub fn backup_status(&self) -> BackupStatus {
        let dir = &self.paths.data;
        let c = r2::load(dir);
        BackupStatus {
            configured: c.configured(),
            enabled: c.enabled,
            keep: c.keep(),
            targets: c
                .targets
                .iter()
                .map(|t| TargetStatus { id: t.id.clone(), name: t.name.clone(), kind: t.kind.clone(), account: t.account.clone(), bucket: t.bucket.clone(), access_key: t.access_key.clone(), has_secret: r2::saved_secret(dir, &t.id).is_some(), last_ok: t.last_ok, last_error: t.last_error.clone() })
                .collect(),
        }
    }

    fn target_secret(&self, t: &r2::Target) -> Result<String, String> {
        r2::saved_secret(&self.paths.data, &t.id).ok_or_else(|| format!("the Secret Access Key of \"{}\" is missing: enter it again in Settings", t.name))
    }

    /// 添加或修改一个备份目标（`id` 为空 = 添加）：先连一次桶，连不上什么都不改。Secret 只进本机加密仓库；修改时留空就沿用。
    pub fn backup_save_target(&self, a: TargetArgs) -> Result<BackupStatus, String> {
        if a.kind != "r2" {
            return Err(format!("unknown backup kind {}", a.kind));
        }
        let dir = &self.paths.data;
        let mut cfg = r2::load(dir);
        let id = a.id.filter(|s| !s.is_empty()).unwrap_or_else(|| format!("t{}", now_millis()));
        let old = cfg.targets.iter().find(|t| t.id == id).cloned();
        let mut t = r2::Target { id: id.clone(), name: a.name.trim().to_string(), kind: a.kind, account: a.account.trim().to_string(), bucket: a.bucket.trim().to_string(), access_key: a.access_key.trim().to_string(), last_ok: old.as_ref().map_or(0, |o| o.last_ok), last_error: None };
        if t.name.is_empty() {
            t.name = format!("Cloudflare R2 · {}", t.bucket);
        }
        if !t.configured() {
            return Err("the endpoint, the bucket and the Access Key ID are all needed".to_string());
        }
        let secret = match a.secret.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(s) => s.to_string(),
            None => r2::saved_secret(dir, &id).ok_or("the Secret Access Key is needed")?,
        };
        r2::check(&t, &secret)?;
        r2::remember_secret(dir, &id, &secret)?;
        match cfg.targets.iter_mut().find(|x| x.id == id) {
            Some(slot) => *slot = t,
            None => cfg.targets.push(t),
        }
        r2::save(dir, &cfg)?;
        Ok(self.backup_status())
    }

    /// 移除一个目标：忘掉它的凭据；桶里已经有的备份不动。
    pub fn backup_remove_target(&self, id: &str) -> Result<BackupStatus, String> {
        let dir = &self.paths.data;
        let mut cfg = r2::load(dir);
        cfg.targets.retain(|t| t.id != id);
        r2::forget_secret(dir, id);
        r2::save(dir, &cfg)?;
        Ok(self.backup_status())
    }

    /// 自动备份的开关和保留份数（目标另外管）。
    pub fn backup_set_options(&self, enabled: Option<bool>, keep: Option<usize>) -> Result<BackupStatus, String> {
        let dir = &self.paths.data;
        let mut cfg = r2::load(dir);
        if let Some(e) = enabled {
            cfg.enabled = e;
        }
        if let Some(k) = keep {
            cfg.keep = k.clamp(1, 1000);
        }
        r2::save(dir, &cfg)?;
        Ok(self.backup_status())
    }

    /// 备份一次到每个目标（自动触发和"立即备份"都是它）："最新"一份，再按天留一份副本并只留最近几份。
    /// 哪个目标失败记在它自己的状态里；全都失败才算错。
    pub fn backup_now(&self, settings: serde_json::Value) -> Result<BackupStatus, String> {
        let dir = &self.paths.data;
        let mut cfg = r2::load(dir);
        if !cfg.configured() {
            return Err("cloud backup is not set up".to_string());
        }
        let (bytes, sum) = self.taste_pack_bytes(settings)?;
        let copy = format!("{}/{}.json", r2::KEY_AUTO, r2::today());
        let keep = cfg.keep();
        let now = now_secs();
        // 说明单独一个小文件：另一端恢复前看一眼是谁、什么时候做的，不用把整个包拉下来
        let meta = serde_json::to_vec(&BackupMeta { device: self.device_name(), platform: PLATFORM.to_string(), exported_at: now, app_version: self.app_version.clone(), styles: sum.styles, examples: sum.examples }).map_err(|e| e.to_string())?;
        let mut failures = Vec::new();
        let total = cfg.targets.iter().filter(|t| t.configured()).count();
        for t in cfg.targets.iter_mut().filter(|t| t.configured()) {
            let r = self.target_secret(t).and_then(|secret| {
                r2::put(t, &secret, r2::KEY_LATEST, &bytes)
                    .and_then(|_| r2::put(t, &secret, r2::KEY_META, &meta))
                    .and_then(|_| r2::put(t, &secret, &copy, &bytes))
                    .and_then(|_| r2::prune_auto(t, &secret, keep).map(|_| ()))
            });
            match r {
                Ok(()) => {
                    t.last_ok = now;
                    t.last_error = None;
                }
                Err(e) => {
                    t.last_error = Some(e.clone());
                    failures.push(format!("{}: {e}", t.name));
                }
            }
        }
        r2::save(dir, &cfg)?;
        if failures.len() == total { Err(failures.join("; ")) } else { Ok(self.backup_status()) }
    }

    /// 某个目标里"最新"备份的说明（谁、什么时候做的）；老版本做的备份没有说明，给 None。
    pub fn backup_peek(&self, id: &str) -> Result<Option<BackupMeta>, String> {
        let cfg = r2::load(&self.paths.data);
        let t = cfg.targets.iter().find(|t| t.id == id).ok_or("unknown backup target")?;
        let secret = self.target_secret(t)?;
        match r2::get(t, &secret, r2::KEY_META)? {
            Some(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|e| format!("the backup's note cannot be read: {e}")),
            None => Ok(None),
        }
    }

    /// 从某个目标的"最新"备份恢复：喜好并进本机的风格，设置交给页面去套用。
    pub fn backup_restore(&self, id: &str) -> Result<(taste::Summary, serde_json::Value), String> {
        let cfg = r2::load(&self.paths.data);
        let t = cfg.targets.iter().find(|t| t.id == id).ok_or("unknown backup target")?;
        let secret = self.target_secret(t)?;
        let bytes = r2::get(t, &secret, r2::KEY_LATEST)?.ok_or("there is no backup in that bucket yet")?;
        let pack: taste::Pack = serde_json::from_slice(&bytes).map_err(|e| format!("the backup cannot be read: {e}"))?;
        self.taste_apply(pack)
    }

    // ------------------------------------------------------------ 网络共享（NAS）

    pub fn nas_list(&self) -> Vec<nas::ShareStatus> {
        let dir = &self.paths.data;
        nas::load(dir)
            .iter()
            .map(|s| nas::ShareStatus { id: s.id.clone(), name: s.name.clone(), host: s.host.clone(), user: s.user.clone(), enabled: s.enabled, local: nas::local_dir(dir, &s.id).to_string_lossy().into_owned(), connected: self.nas.is_connected(&s.id) })
            .collect()
    }

    /// 没连着就用记住的密码连上；没有记住的密码就说明缺什么。停用的不连。
    fn nas_connect_saved(&self, s: &nas::Share) -> Result<(), String> {
        if self.nas.is_connected(&s.id) {
            return Ok(());
        }
        if !s.enabled {
            return Err(format!("the NAS \"{}\" is turned off in Settings", s.name));
        }
        let pw = nas::saved_password(&self.paths.data, s).ok_or_else(|| format!("the NAS \"{}\" needs its password: connect it in Settings", s.name))?;
        self.nas.connect(s, &pw).map_err(|e| format!("{}: {e}", s.name))
    }

    /// 项目文件夹里落在共享镜像上的，先把远端同步进来（进度发 "nas-progress"）；`job` 变了就停（页面离开项目时会 `scan_cancel`）。
    fn nas_sync_dirs(self: &Arc<Self>, dirs: &[String], job: u64) -> Result<(), String> {
        let data = &self.paths.data;
        let shares = nas::load(data);
        for d in dirs {
            let Some((s, rel)) = nas::share_of_path(data, &shares, Path::new(d)) else { continue };
            self.nas_connect_saved(s)?;
            let local = nas::local_dir(data, &s.id);
            // 进度与取消在 NAS 的线程上回调，所以各拿一份引擎的引用
            let (emit, watch) = (self.clone(), self.clone());
            let r = self.nas.sync(&s.id, &rel, &local, move |p| emit.emit("nas-progress", &p), move || watch.scan_job.load(Ordering::Relaxed) != job);
            r.map_err(|e| if e.contains("ancelled") { "cancelled".to_string() } else { format!("{}: {e}", s.name) })?;
        }
        Ok(())
    }

    /// 添加或编辑 NAS 时的"测试连接"：连上这台服务器，返回它的共享名；什么都不记。编辑时密码留空就用记住的。
    pub fn nas_probe(&self, host: &str, user: &str, password: &str, id: Option<&str>) -> Result<Vec<String>, String> {
        let password = match (password.is_empty(), id) {
            (true, Some(id)) => {
                let dir = &self.paths.data;
                let s = nas::load(dir).into_iter().find(|s| s.id == id).ok_or("unknown NAS")?;
                if s.user == user.trim() { nas::saved_password(dir, &s).unwrap_or_default() } else { String::new() }
            }
            _ => password.to_string(),
        };
        self.nas.probe(host, user.trim(), &password)
    }

    /// 编辑一台 NAS：改显示名、地址、账号或密码；先用新的连一次，连不上什么都不改。id 不变，镜像和项目都还在。
    pub fn nas_update(&self, id: &str, name: &str, host: &str, user: &str, password: &str, remember: bool) -> Result<Vec<nas::ShareStatus>, String> {
        let host = nas::check_host(host)?;
        let dir = &self.paths.data;
        let mut list = nas::load(dir);
        let old = list.iter().find(|s| s.id == id).cloned().ok_or("unknown NAS")?;
        if list.iter().any(|s| s.id != id && s.host.eq_ignore_ascii_case(&host) && s.user == user.trim()) {
            return Err("this NAS is already in the list".to_string());
        }
        let next = nas::Share { id: id.to_string(), name: if name.trim().is_empty() { host.clone() } else { name.trim().to_string() }, host, user: user.trim().to_string(), enabled: old.enabled };
        // 密码留空 = 用记住的（账号没变时）
        let same_account = next.host == old.host && next.user == old.user;
        let pw = if password.is_empty() { if same_account { nas::saved_password(dir, &old).unwrap_or_default() } else { String::new() } } else { password.to_string() };
        self.nas.connect(&next, &pw)?;
        if !same_account {
            nas::forget_password(dir, &old);
        }
        if remember && (!same_account || !pw.is_empty()) {
            nas::remember_password(dir, &next, &pw)?;
        }
        if let Some(s) = list.iter_mut().find(|s| s.id == id) {
            *s = next;
        }
        nas::save(dir, &list)?;
        Ok(self.nas_list())
    }

    /// 添加一台 NAS 并马上连接；连不上就什么都不记。共享不用选，新建项目时浏览。
    pub fn nas_add(&self, name: &str, host: &str, user: &str, password: &str, remember: bool) -> Result<Vec<nas::ShareStatus>, String> {
        let host = nas::check_host(host)?;
        let dir = &self.paths.data;
        let mut list = nas::load(dir);
        if list.iter().any(|s| s.host.eq_ignore_ascii_case(&host) && s.user == user.trim()) {
            return Err("this NAS is already in the list".to_string());
        }
        let id = format!("n{}", now_millis());
        let name = if name.trim().is_empty() { host.clone() } else { name.trim().to_string() };
        let share = nas::Share { id, name, host, user: user.trim().to_string(), enabled: true };
        self.nas.connect(&share, password)?;
        if remember {
            nas::remember_password(dir, &share, password)?;
        }
        list.push(share);
        nas::save(dir, &list)?;
        Ok(self.nas_list())
    }

    /// 移除：断开、忘掉密码、删掉镜像里的本地副本（NAS 上的照片不动）。
    pub fn nas_remove(&self, id: &str) -> Result<Vec<nas::ShareStatus>, String> {
        let dir = &self.paths.data;
        let mut list = nas::load(dir);
        if let Some(s) = list.iter().find(|s| s.id == id).cloned() {
            self.nas.disconnect(&s.id);
            nas::forget_password(dir, &s);
            let _ = std::fs::remove_dir_all(nas::local_dir(dir, &s.id));
            list.retain(|x| x.id != id);
            nas::save(dir, &list)?;
        }
        Ok(self.nas_list())
    }

    /// 连接一个记住的共享：用记住的密码，或者现在给的密码（给了就记住）。
    pub fn nas_connect(&self, id: &str, password: Option<&str>) -> Result<Vec<nas::ShareStatus>, String> {
        let dir = &self.paths.data;
        let s = nas::load(dir).into_iter().find(|s| s.id == id).ok_or("unknown share")?;
        match password.filter(|p| !p.is_empty()) {
            Some(pw) => {
                self.nas.connect(&s, pw)?;
                nas::remember_password(dir, &s, pw)?;
            }
            None => self.nas_connect_saved(&s)?,
        }
        Ok(self.nas_list())
    }

    pub fn nas_disconnect(&self, id: &str) -> Vec<nas::ShareStatus> {
        self.nas.disconnect(id);
        self.nas_list()
    }

    /// 启用 / 停用：停用就断开，启用就用记住的密码连上（连不上也先记下开关，状态里能看到）。
    pub fn nas_set_enabled(&self, id: &str, enabled: bool) -> Result<Vec<nas::ShareStatus>, String> {
        let dir = &self.paths.data;
        let mut list = nas::load(dir);
        let s = list.iter_mut().find(|s| s.id == id).ok_or("unknown NAS")?;
        s.enabled = enabled;
        let s = s.clone();
        nas::save(dir, &list)?;
        if enabled {
            let _ = self.nas_connect_saved(&s);
        } else {
            self.nas.disconnect(id);
        }
        Ok(self.nas_list())
    }

    /// 打开项目前：这些文件夹所在的共享先连上，并返回它们的状态，页面好说明哪一个连不上。
    pub fn nas_ensure(&self, dirs: &[String]) -> Vec<nas::ShareStatus> {
        let dir = &self.paths.data;
        let shares = nas::load(dir);
        let mut out = Vec::new();
        for s in &shares {
            if dirs.iter().any(|d| nas::share_of_path(dir, std::slice::from_ref(s), Path::new(d)).is_some()) {
                let _ = self.nas_connect_saved(s);
                out.extend(self.nas_list().into_iter().filter(|x| x.id == s.id));
            }
        }
        out
    }

    pub fn nas_browse(&self, id: &str, path: &str) -> Result<Listing, String> {
        let dir = &self.paths.data;
        let s = nas::load(dir).into_iter().find(|s| s.id == id).ok_or("unknown share")?;
        self.nas_connect_saved(&s)?;
        let rel = path.trim_matches('/').to_string();
        let entries = self.nas.browse(&s.id, &rel)?;
        let local = if rel.is_empty() { String::new() } else { nas::local_dir(dir, &s.id).join(&rel).to_string_lossy().into_owned() };
        Ok(Listing { path: rel, local, entries })
    }

    // ------------------------------------------------------------ 整理

    /// Copies the given photos into `dest`. Originals are never touched and nothing is overwritten.
    pub fn export_photos(&self, paths: &[String], dest: &str) -> Result<ExportResult, String> {
        let dest_dir = PathBuf::from(dest);
        std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
        let total = paths.len();
        let (mut copied, mut skipped) = (0usize, 0usize);
        for (i, p) in paths.iter().enumerate() {
            let src = Path::new(p);
            if analyze::is_image(src) && src.is_file() {
                let target = unique_path(&dest_dir, src.file_name().unwrap_or_default());
                match std::fs::copy(src, &target) {
                    Ok(_) => copied += 1,
                    Err(_) => skipped += 1,
                }
            } else {
                skipped += 1;
            }
            self.emit("export-progress", &Progress { done: i + 1, total });
        }
        Ok(ExportResult { copied, skipped, dest: dest.to_string() })
    }

    /// Moves photos to the trash (recoverable). Only image files are accepted, so this can never be used to trash anything else.
    /// 共享镜像里的照片在 NAS 上移进回收文件夹、本机副本随之删掉；本机的照片交给宿主注册的 `LocalTrash`，没注册就放进 `deferred`。
    pub fn trash_photos(&self, paths: Vec<String>) -> TrashResult {
        let data = &self.paths.data;
        let shares = nas::load(data);
        let mut r = TrashResult::default();
        // 共享镜像里的照片按共享分组：在 NAS 上一次移完，成功的再删掉本地副本
        let mut on_nas: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut local = Vec::new();
        for p in paths {
            let path = PathBuf::from(&p);
            if !(analyze::is_image(&path) && path.is_file()) {
                r.failed.push(p);
                continue;
            }
            match nas::share_of_path(data, &shares, &path) {
                Some((s, rel)) => on_nas.entry(s.id.clone()).or_default().push((p, rel)),
                None => local.push(path),
            }
        }
        for (id, items) in on_nas {
            let Some(s) = shares.iter().find(|s| s.id == id) else { continue };
            let _ = self.nas_connect_saved(s);
            let results = self.nas.recycle(&id, items.iter().map(|(_, rel)| rel.clone()).collect());
            for ((p, _), res) in items.into_iter().zip(results) {
                match res {
                    Ok(target) => {
                        let _ = std::fs::remove_file(&p);
                        r.moved += 1;
                        let at = nas::recycle_url(s, nas::split_share(&target).0);
                        if !r.network_bins.contains(&at) {
                            r.network_bins.push(at);
                        }
                    }
                    Err(e) => {
                        eprintln!("recycle on {}: {e}", s.name);
                        r.failed.push(p);
                    }
                }
            }
        }
        for path in local {
            match crate::platform::local_trash() {
                Some(t) => match t.trash(&path) {
                    Ok(bin) => {
                        r.moved += 1;
                        if let Some(b) = bin {
                            if !r.network_bins.contains(&b) {
                                r.network_bins.push(b);
                            }
                        }
                    }
                    Err(_) => r.failed.push(path.to_string_lossy().into_owned()),
                },
                None => r.deferred.push(path.to_string_lossy().into_owned()),
            }
        }
        r
    }

    // ------------------------------------------------------------ 查看器

    /// Path of a large cached preview for the viewer (created on first request). `showable`: the host can show the original file
    /// itself (on the desktop: the web view may load it).
    pub fn large_preview(&self, path: &str, full: bool, showable: bool) -> Result<String, String> {
        let dir = self.paths.thumbs.join("large");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let src = Path::new(path);
        let (rot, _, _) = self.rotation_of(path);
        // The original file itself whenever the viewer can show it: that is the only way to see what the camera recorded (HDR, wide
        // colour, every pixel). Only formats it cannot show (camera RAW) or enormous files get a rendered preview.
        // Only the full-size viewer gets it (decoding a 15-30 MB original is too heavy for the several previews on screen in compare or single).
        // 转过的照片总是渲染出来（原文件没转）。
        if rot == 0 && full && showable && imageio::web_can_show(src) && std::fs::metadata(src).map_or(false, |m| m.len() < 120 * 1024 * 1024) {
            return Ok(path.to_string());
        }
        let out = analyze::cache_name(src, ".l.jpg", &dir);
        if !out.exists() {
            let tmp = out.with_extension("tmp");
            analyze::large_preview(src, &tmp)?;
            std::fs::rename(&tmp, &out).map_err(|e| e.to_string())?;
        }
        if rot == 0 {
            return Ok(out.to_string_lossy().into_owned());
        }
        let turned = analyze::cache_name(src, &format!(".l.r{rot}.jpg"), &dir);
        if !turned.exists() {
            analyze::rotate_jpeg(&out, &turned, rot, 88)?;
        }
        Ok(turned.to_string_lossy().into_owned())
    }

    /// The sharp part of a photo for the full-size viewer when it is zoomed: the region (fractions of the photo, top-left origin) at up to `out_w`
    /// pixels, as a JPEG next to the thumbnails. Keyed by region, so going back to a place already seen is instant.
    pub fn photo_detail(&self, path: &str, x: f64, y: f64, w: f64, h: f64, out_w: u32) -> Result<String, String> {
        static CROPS: AtomicU64 = AtomicU64::new(0);
        let dir = self.paths.thumbs.join("detail");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let src = Path::new(path);
        let (rot, dw, dh) = self.rotation_of(path);
        let tag = format!(".d{:.3}-{:.3}-{:.3}-{:.3}-{}{}.jpg", x, y, w, h, out_w, if rot == 0 { String::new() } else { format!("-r{rot}") });
        let out = analyze::cache_name(src, &tag, &dir);
        if !out.exists() {
            // 转过的照片：屏幕上那块（转过之后的坐标）先换算回原图的坐标，裁出来再转回去。
            // 输出宽度按原图里那块的宽算：90 / 270 时屏幕上的高对应原图的宽。
            let (region, ow) = match rot {
                0 => ([x, y, w, h], out_w),
                180 => ([1.0 - x - w, 1.0 - y - h, w, h], out_w),
                _ => {
                    let r = if rot == 90 { [y, 1.0 - x - w, h, w] } else { [1.0 - y - h, x, h, w] };
                    let ow = (out_w as f64 * (h * dh as f64) / (w * dw as f64).max(1e-6)).round().max(1.0) as u32;
                    (r, ow)
                }
            };
            let target = if rot == 0 { out.clone() } else { out.with_extension("raw.jpg") };
            if imageio::available() {
                imageio::crop(src, &target, region, ow)?;
            } else {
                crop_native(src, &target, region, ow)?;
            }
            if rot != 0 {
                let r = analyze::rotate_jpeg(&target, &out, rot, 90);
                let _ = std::fs::remove_file(&target);
                r?;
            }
            // every new crop is another ~2 MB: look at the folder's size now and then
            if CROPS.fetch_add(1, Ordering::Relaxed) % 24 == 23 {
                scratch::prune_detail(&self.paths.thumbs);
            }
        }
        Ok(out.to_string_lossy().into_owned())
    }

    /// The video of a motion photo, written next to the thumbnails (once) so the viewer can play it.
    pub fn motion_clip(&self, path: &str) -> Result<String, String> {
        let src = Path::new(path);
        let out = analyze::cache_name(src, ".m.mp4", &self.paths.thumbs);
        if !out.exists() {
            media::extract_motion(src, &out)?;
        }
        Ok(out.to_string_lossy().into_owned())
    }

    /// Basic file facts for the inspector: container format, size, colour space and the shooting parameters stored in EXIF.
    pub fn photo_info(&self, path: &str) -> FileInfo {
        let p = Path::new(path);
        let mut info = FileInfo { bytes: std::fs::metadata(p).map(|m| m.len()).unwrap_or(0), ..Default::default() };
        info.format = image::ImageReader::open(p)
            .ok()
            .and_then(|r| r.with_guessed_format().ok())
            .and_then(|r| r.format())
            .map(|f| format!("{f:?}").to_uppercase())
            .or_else(|| p.extension().map(|e| e.to_string_lossy().to_uppercase()));
        let Ok(file) = std::fs::File::open(p) else { return info };
        let Ok(ex) = exif::Reader::new().read_from_container(&mut std::io::BufReader::new(file)) else { return info };
        let get = |tag: exif::Tag| {
            ex.get_field(tag, exif::In::PRIMARY)
                .map(|f| match &f.value {
                    // Text tags may carry several (often empty) strings; take the first real one.
                    exif::Value::Ascii(parts) => parts.iter().map(|p| String::from_utf8_lossy(p).trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string()).find(|p| !p.is_empty()).unwrap_or_default(),
                    _ => f.display_value().with_unit(&ex).to_string().trim().to_string(),
                })
                .filter(|v| !v.is_empty())
        };
        let make = get(exif::Tag::Make);
        let model = get(exif::Tag::Model);
        info.camera = match (make, model) {
            (Some(a), Some(b)) => Some(if b.starts_with(&a) { b } else { format!("{a} {b}") }),
            (a, b) => a.or(b),
        };
        info.lens = get(exif::Tag::LensModel);
        info.focal = get(exif::Tag::FocalLength);
        info.aperture = get(exif::Tag::FNumber);
        info.shutter = get(exif::Tag::ExposureTime);
        info.iso = get(exif::Tag::PhotographicSensitivity);
        info.software = get(exif::Tag::Software);
        info.taken = get(exif::Tag::DateTimeOriginal).or_else(|| get(exif::Tag::DateTime));
        info.color_space = ex.get_field(exif::Tag::ColorSpace, exif::In::PRIMARY).and_then(|f| f.value.get_uint(0)).map(|v| match v {
            1 => "sRGB".to_string(),
            2 => "Adobe RGB".to_string(),
            0xFFFF => "Uncalibrated".to_string(),
            n => n.to_string(),
        });
        info.bit_depth = get(exif::Tag::BitsPerSample);
        info
    }

    /// Fast preview thumbnails for a handful of photos (same cache location the analysis uses, so nothing is wasted).
    pub fn preview_thumbs(&self, paths: &[String]) -> Vec<Option<String>> {
        let cache_dir = &self.paths.thumbs;
        let _ = std::fs::create_dir_all(cache_dir);
        paths
            .par_iter()
            .map(|p| {
                let src = Path::new(p);
                let out = analyze::cache_name(src, ".jpg", cache_dir);
                if out.exists() || analyze::quick_thumb(src, &out).is_ok() { Some(out.to_string_lossy().into_owned()) } else { None }
            })
            .collect()
    }

    // ------------------------------------------------------------ 照片搜索

    fn image_keys(dirs: &[String]) -> Vec<String> {
        let mut all: Vec<PathBuf> = dirs.iter().flat_map(|d| list_images(d)).collect();
        all.sort();
        all.dedup();
        all.into_iter().map(|p| p.to_string_lossy().into_owned()).collect()
    }

    /// How many of the photos can already be searched. Photos analysed with the quick model always can; the others need the index.
    pub fn search_status(&self, dirs: &[String]) -> SearchStatus {
        let keys = Self::image_keys(dirs);
        let ready = self.with_cache(|c| keys.iter().filter(|k| c.semantic(k).is_some() && c.color(k).is_some()).count());
        SearchStatus { total: keys.len(), ready, available: textsearch::TextEncoder::available(&self.paths.models) }
    }

    /// Builds the search vectors that are missing. It reads the 640 px thumbnails made during analysis, so it is fast (tens of
    /// milliseconds a photo) and never touches the original files. Progress goes out as `search-index-progress`.
    pub fn search_index(&self, dirs: &[String]) -> Result<usize, String> {
        let keys = Self::image_keys(dirs);
        let pending = self.with_cache(|c| c.without_index(&keys));
        if pending.is_empty() {
            return Ok(0);
        }
        // the larger analysis model leaves no ViT-B/32 vector behind, so those photos get one here; the rest only need their colours
        let scorer = if pending.iter().any(|p| p.4) { Some(ClipScorer::load(&self.paths.models, true, Quality::Fast)?) } else { None };
        let total = pending.len();
        let mut done = 0usize;
        self.emit("search-index-progress", &Progress { done, total });
        for chunk in pending.chunks(48) {
            let made: Vec<(String, u64, i64, Option<Vec<f32>>, Vec<u8>)> = chunk
                .par_iter()
                .filter_map(|(key, thumb, size, mtime, need_vec)| {
                    let img = image::open(thumb).ok()?;
                    let sem = match (need_vec, scorer.as_ref()) {
                        (true, Some(s)) => Some(<ClipScorer as AiScorer>::analyze(s, &img).ok()?.embedding),
                        _ => None,
                    };
                    Some((key.clone(), *size, *mtime, sem, colorsig::signature(&img)))
                })
                .collect();
            self.with_cache(|c| {
                for (key, size, mtime, sem, color) in made {
                    c.set_index(&key, size, mtime, sem, color);
                }
            });
            done += chunk.len();
            self.emit("search-index-progress", &Progress { done, total });
        }
        self.with_cache(|c| c.save());
        Ok(total)
    }

    /// Photos that match a description, best first. Words about how many people are in the picture are answered from the face
    /// detector (it counts far better than an image-text model); the rest is compared through the text model.
    pub fn search_photos(&self, dirs: &[String], query: &str) -> Result<Vec<Hit>, String> {
        let q = textsearch::parse(query);
        let qv = match &q.text {
            Some(t) => {
                let encoder = {
                    let mut g = self.text.lock().unwrap_or_else(|e| e.into_inner());
                    if g.is_none() {
                        *g = Some(Arc::new(textsearch::TextEncoder::load(&self.paths.models)?));
                    }
                    g.as_ref().unwrap().clone()
                };
                Some(encoder.encode_query(t)?)
            }
            None => None,
        };
        let keys = Self::image_keys(dirs);
        let hits: Vec<Hit> = self.with_cache(|c| {
            let rows: Vec<(&Photo, Option<&[f32]>, Option<&[u8]>)> = keys.iter().filter_map(|k| c.photo(k).map(|p| (p, c.semantic(k), c.color(k)))).collect();
            let items: Vec<textsearch::Item> = rows.iter().map(|(p, s, col)| textsearch::Item { sem: *s, color: *col, faces: p.face_count }).collect();
            textsearch::rank(&q, qv.as_deref(), &items)
                .into_iter()
                .map(|(i, score)| {
                    let p = rows[i].0;
                    Hit { path: p.path.clone(), name: p.name.clone(), thumb: p.thumb.clone(), width: p.width, height: p.height, score }
                })
                .collect()
        });
        Ok(hits)
    }

    // ------------------------------------------------------------ 场景命名与描述

    /// Names each scene of a finished project. Two steps: first a large vocabulary of everyday and travel places (see scene_vocab.rs),
    /// matched against a few of the scene's photos, which is quick and needs no download; then, for the scenes it could not name and only
    /// if `caption` is set, a sentence written by the captioning model about the place (the people painted out first), in Chinese when
    /// `zh`. Results arrive as `scene-texts` / `scene-text` events; a newer call cancels an older one. Written sentences are remembered on disk.
    pub fn scene_text(&self, scenes: &[SceneReq], zh: bool, caption: bool) -> Result<(), String> {
        let job = self.scene_job.fetch_add(1, Ordering::SeqCst) + 1;
        let dir = self.paths.models.clone();
        let current = || self.scene_job.load(Ordering::SeqCst) == job;

        // 1. the vocabulary: the mean ViT-B/32 embedding of up to six photos of each scene
        const PER_SCENE: usize = 6;
        let known: Vec<Vec<Option<Vec<f32>>>> = self.with_cache(|c| scenes.iter().map(|s| s.paths.iter().take(PER_SCENE).map(|k| c.semantic(k).map(|v| v.to_vec())).collect()).collect());
        let thumbs: Vec<Vec<Option<String>>> = self.with_cache(|c| {
            scenes.iter().zip(&known).map(|(s, kn)| s.paths.iter().take(PER_SCENE).zip(kn).map(|(k, v)| if v.is_some() { None } else { c.photo(k).map(|p| p.thumb.clone()) }).collect()).collect()
        });
        let scorer = if thumbs.iter().flatten().any(|t| t.is_some()) { ClipScorer::load(&dir, true, Quality::Fast).ok() } else { None };
        let mut unnamed: Vec<&SceneReq> = Vec::new();
        let mut names: Vec<SceneText> = Vec::new();
        for (n, scene) in scenes.iter().enumerate() {
            if !current() {
                return Ok(());
            }
            let mut mean = vec![0f32; scene_vocab::DIM];
            let mut count = 0;
            for (v, t) in known[n].iter().zip(&thumbs[n]) {
                let vec = match (v, t, scorer.as_ref()) {
                    (Some(v), _, _) => Some(v.clone()),
                    (None, Some(t), Some(s)) => image::open(t).ok().and_then(|img| <ClipScorer as AiScorer>::analyze(s, &img).ok()).map(|o| o.embedding),
                    _ => None,
                };
                if let Some(v) = vec.filter(|v| v.len() == scene_vocab::DIM) {
                    mean.iter_mut().zip(&v).for_each(|(a, b)| *a += b);
                    count += 1;
                }
            }
            let named = (count > 0)
                .then(|| {
                    let norm = mean.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
                    mean.iter_mut().for_each(|x| *x /= norm);
                    scene_vocab::name(&mean)
                })
                .flatten();
            match named {
                Some(name) => names.push(SceneText { id: scene.id, text: if zh { name.zh.to_string() } else { name.en } }),
                None => unnamed.push(scene),
            }
        }
        // one event for all of them: a project can have a thousand scenes
        if !names.is_empty() && current() {
            self.emit("scene-texts", &names);
        }
        let total = unnamed.len();
        self.emit("scene-text-progress", &Progress { done: 0, total });
        if !caption || total == 0 {
            return Ok(());
        }

        // 2. the scenes the vocabulary could not name: a sentence from the captioning model
        let captioner = {
            let mut g = self.captioner.lock().unwrap_or_else(|e| e.into_inner());
            if g.is_none() {
                if !florence::Captioner::available(&dir) {
                    return Ok(());
                }
                *g = Some(Arc::new(florence::Captioner::load(&dir)?));
            }
            g.clone().unwrap()
        };
        let translator = if zh {
            let mut g = self.translator.lock().unwrap_or_else(|e| e.into_inner());
            if g.is_none() && translate::Translator::available(&dir) {
                *g = translate::Translator::load(&dir).ok().map(Arc::new);
            }
            g.clone()
        } else {
            None
        };
        let store = self.paths.data.join("scene-sentences.json");
        {
            let mut g = self.scene_sentences.lock().unwrap_or_else(|e| e.into_inner());
            if g.is_none() {
                *g = Some(std::fs::read(&store).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default());
            }
        }
        let save = || {
            if let Some(m) = self.scene_sentences.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                let _ = std::fs::write(&store, serde_json::to_vec(m).unwrap_or_default());
            }
        };
        let mut fresh = 0;
        for (n, scene) in unnamed.iter().enumerate() {
            if !current() {
                break;
            }
            // the photo with the fewest faces among the best few: the one with the most place in it
            let pick = self.with_cache(|c| scene.paths.iter().take(4).filter_map(|p| c.photo(p)).min_by_key(|p| p.face_count).map(|p| p.thumb.clone()));
            if let Some(thumb) = pick {
                let known = self.scene_sentences.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&thumb).cloned());
                let sentence = match known {
                    Some(s) => Some(s),
                    None => match image::open(&thumb).map_err(|e| e.to_string()).and_then(|img| captioner.scene(&img)) {
                        Ok(s) if !s.is_empty() => {
                            if let Some(m) = self.scene_sentences.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                                m.insert(thumb.clone(), s.clone());
                            }
                            fresh += 1;
                            Some(s)
                        }
                        _ => None,
                    },
                };
                if let Some(s) = sentence {
                    let s = florence::short(&s);
                    if florence::place_like(&s) {
                        let text = match &translator {
                            Some(t) => t.zh(&s).unwrap_or(s),
                            None => s,
                        };
                        self.emit("scene-text", &SceneText { id: scene.id, text });
                    }
                }
            }
            self.emit("scene-text-progress", &Progress { done: n + 1, total });
            if fresh > 0 && fresh % 8 == 0 {
                save();
            }
        }
        save();
        Ok(())
    }

    // ------------------------------------------------------------ 模型包

    pub fn machine_info(&self) -> Machine {
        Machine { memory_gb: crate::platform::physical_memory() >> 30, cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1), accelerator: crate::onnx::accelerator().to_string() }
    }

    /// 每个模型包装没装；`bytes` 是还要下载的字节数（装好了是 0；`with_sizes` 为假时不读清单，也是 0）。
    pub fn packs_status(&self, with_sizes: bool) -> Vec<modelpacks::PackStatus> {
        let dir = &self.paths.models;
        modelpacks::PACKS
            .iter()
            .map(|p| {
                let installed = modelpacks::installed(dir, p);
                let bytes = if installed || !with_sizes { 0 } else { modelpacks::manifest_sizes(dir, p.id).unwrap_or(0) };
                modelpacks::PackStatus { id: p.id.to_string(), installed, bytes }
            })
            .collect()
    }

    /// How many bytes a pack still needs (0 if it is installed), so the app can say so before asking.
    pub fn pack_size(&self, id: &str) -> Result<u64, String> {
        let dir = &self.paths.models;
        let p = modelpacks::pack(id).ok_or("unknown pack")?;
        if modelpacks::installed(dir, p) {
            return Ok(0);
        }
        modelpacks::manifest_sizes(dir, id)
    }

    /// 下载一个包缺的文件（一次只能下一个包），进度发 `model-progress`；`pack_cancel` 可以中途停，下了一半的文件会删掉。
    pub fn pack_download(&self, id: &str) -> Result<(), String> {
        {
            let mut busy = self.downloads.busy.lock().unwrap_or_else(|e| e.into_inner());
            if busy.is_some() {
                return Err("another download is running".into());
            }
            *busy = Some(id.to_string());
            self.downloads.cancel.store(false, Ordering::Relaxed);
        }
        let result = modelpacks::download(&self.paths.downloads, id, &self.downloads.cancel, &|p| self.emit("model-progress", &p));
        *self.downloads.busy.lock().unwrap_or_else(|e| e.into_inner()) = None;
        // the text model (or the face models) may have just arrived, or a broken one been replaced: load them afresh next time
        self.forget_models();
        result
    }

    pub fn pack_cancel(&self) {
        self.downloads.cancel.store(true, Ordering::Relaxed);
    }

    pub fn pack_remove(&self, id: &str) -> Result<(), String> {
        modelpacks::remove(&self.paths.downloads, id)?;
        self.forget_models();
        Ok(())
    }

    fn forget_models(&self) {
        *self.text.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.mediapipe_eyes.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.face_embedder.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.captioner.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.translator.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    // ------------------------------------------------------------ 喜好学习与风格

    fn retrain_style(&self, styles: &mut prefs::Styles, style: Option<&str>) {
        let p = &mut styles.get_mut(style).prefs;
        self.with_cache(|c| p.retrain(|k| c.embedding(k), || c.pool()));
    }

    /// Records (or clears) one explicit user decision. The label is recorded at once; the retraining it calls for runs a moment later,
    /// once the keys have stopped, so a run of P / X presses retrains once rather than every time, on a copy outside the lock, and the
    /// file is written outside it too.
    pub fn record_label(self: &Arc<Self>, path: &str, label: Option<bool>, style: Option<&str>) -> prefs::PrefsInfo {
        let info = {
            let mut styles = self.prefs.lock().unwrap_or_else(|e| e.into_inner());
            let p = &mut styles.get_mut(style).prefs;
            match label {
                Some(v) => {
                    p.labels.insert(path.to_string(), v);
                }
                None => {
                    p.labels.remove(path);
                }
            }
            p.info()
        };
        let gen = self.retrain_gen.fetch_add(1, Ordering::Relaxed) + 1;
        let e = self.clone();
        let style = style.map(str::to_string);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            if e.retrain_gen.load(Ordering::Relaxed) != gen {
                return; // a newer label will do it
            }
            e.retrain_outside_lock(style.as_deref());
        });
        info
    }

    /// Retrains a style on a copy of its preferences, so the lock is held only to take the copy and to put the weights back, then writes
    /// `styles.json` outside the lock as well.
    fn retrain_outside_lock(&self, style: Option<&str>) {
        let mut copy = self.prefs.lock().unwrap_or_else(|e| e.into_inner()).get(style).prefs.clone();
        self.with_cache(|c| copy.retrain(|k| c.embedding(k), || c.pool()));
        let pending = {
            let mut styles = self.prefs.lock().unwrap_or_else(|e| e.into_inner());
            styles.get_mut(style).prefs.adopt_weights(&copy);
            styles.to_save()
        };
        if let Some(pending) = pending {
            pending.write();
        }
    }

    pub fn prefs_info(&self, style: Option<&str>) -> prefs::PrefsInfo {
        self.prefs.lock().unwrap_or_else(|e| e.into_inner()).get(style).prefs.info()
    }

    pub fn prefs_reset(&self, style: Option<&str>) -> prefs::PrefsInfo {
        let mut styles = self.prefs.lock().unwrap_or_else(|e| e.into_inner());
        styles.get_mut(style).prefs.reset();
        styles.save();
        styles.get(style).prefs.info()
    }

    pub fn styles_list(&self) -> Vec<prefs::StyleInfo> {
        self.prefs.lock().unwrap_or_else(|e| e.into_inner()).infos()
    }

    pub fn style_create(&self, name: &str) -> Vec<prefs::StyleInfo> {
        let mut styles = self.prefs.lock().unwrap_or_else(|e| e.into_inner());
        styles.create(name);
        styles.save();
        styles.infos()
    }

    pub fn style_rename(&self, id: &str, name: &str) -> Vec<prefs::StyleInfo> {
        let mut styles = self.prefs.lock().unwrap_or_else(|e| e.into_inner());
        styles.rename(id, name);
        styles.save();
        styles.infos()
    }

    pub fn style_delete(&self, id: &str) -> Vec<prefs::StyleInfo> {
        let mut styles = self.prefs.lock().unwrap_or_else(|e| e.into_inner());
        styles.delete(id);
        styles.save();
        styles.infos()
    }

    pub fn style_set_default(&self, id: &str) -> Vec<prefs::StyleInfo> {
        let mut styles = self.prefs.lock().unwrap_or_else(|e| e.into_inner());
        styles.set_default(id);
        styles.save();
        styles.infos()
    }

    /// Learns a style from a folder of past picks: the photos there are what this person chooses. Only their embeddings are kept, never the
    /// pictures. Progress goes out as `reference-progress`.
    pub fn learn_from_folder(&self, dir: &str, style: Option<&str>, compute: Option<&str>, quality: Option<&str>) -> Result<prefs::PrefsInfo, String> {
        let want = Quality::parse(quality);
        let accel = compute != Some("cpu");
        let files = list_images(dir);
        if files.is_empty() {
            return Err("no photos in that folder".to_string());
        }
        // the embeddings must be the kind the analysed photos have (they are compared with them), so the same model is used
        let scorer = ClipScorer::load(&self.paths.models, accel, want)?;
        let total = files.len();
        let done = AtomicUsize::new(0);
        self.emit("reference-progress", &Progress { done: 0, total });
        let refs: Vec<prefs::Reference> = files
            .par_iter()
            .filter_map(|p| {
                let r = analyze::load_for_embedding(p).and_then(|img| scorer.embed(&img)).ok().map(|emb| prefs::Reference { stem: p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default(), emb });
                let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                if d % 4 == 0 || d == total {
                    self.emit("reference-progress", &Progress { done: d, total });
                }
                r
            })
            .collect();
        if refs.len() < 6 {
            return Err("at least 6 readable photos are needed to learn from a folder".to_string());
        }
        let mut styles = self.prefs.lock().unwrap_or_else(|e| e.into_inner());
        {
            let p = &mut styles.get_mut(style).prefs;
            p.refs = refs;
            p.refs_from = dir.to_string();
        }
        self.retrain_style(&mut styles, style);
        styles.save();
        Ok(styles.get(style).prefs.info())
    }

    // ------------------------------------------------------------ 缓存

    /// Walks tens of thousands of thumbnails on a big install.
    pub fn cache_size(&self) -> u64 {
        WalkDir::new(&self.paths.thumbs).into_iter().filter_map(Result::ok).filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum()
    }

    pub fn clear_cache(&self) -> Result<(), String> {
        let dir = &self.paths.thumbs;
        if dir.exists() {
            std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())
    }
}

/// 没有系统解码器时的局部：`image` crate 解出摆正的原图，裁出那一块，宽不超过 `out_w`。
fn crop_native(src: &Path, out: &Path, f: [f64; 4], out_w: u32) -> Result<(), String> {
    let img = analyze::load_oriented(src)?;
    let (w, h) = (img.width() as f64, img.height() as f64);
    let x0 = (f[0].clamp(0.0, 1.0) * w).round() as u32;
    let y0 = (f[1].clamp(0.0, 1.0) * h).round() as u32;
    let cw = ((f[2].clamp(0.0, 1.0) * w).round() as u32).clamp(1, img.width().saturating_sub(x0).max(1));
    let ch = ((f[3].clamp(0.0, 1.0) * h).round() as u32).clamp(1, img.height().saturating_sub(y0).max(1));
    let mut part = img.crop_imm(x0, y0, cw, ch);
    if part.width() > out_w.max(1) {
        part = part.resize(out_w, u32::MAX, image::imageops::FilterType::CatmullRom);
    }
    let file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(file), 90).encode_image(&part.to_rgb8()).map_err(|e| e.to_string())
}

fn now_millis() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Collect(Mutex<Vec<(String, serde_json::Value)>>);
    impl Events for Arc<Collect> {
        fn emit(&self, e: &str, v: serde_json::Value) {
            self.0.lock().unwrap().push((e.to_string(), v));
        }
    }

    fn engine(tag: &str) -> (Arc<Engine>, PathBuf, Arc<Collect>) {
        let root = std::env::temp_dir().join(format!("fg-engine-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let events = Arc::new(Collect(Mutex::new(Vec::new())));
        let paths = Paths { data: root.join("data"), thumbs: root.join("thumbs"), models: root.join("models"), downloads: root.join("models") };
        (Engine::new(paths, Box::new(events.clone()), "0.0.0-test"), root, events)
    }

    fn photos(dir: &Path, n: usize) {
        std::fs::create_dir_all(dir).unwrap();
        for i in 0..n {
            let img = image::RgbImage::from_fn(640, 480, |x, y| image::Rgb([((x + i as u32 * 40) % 256) as u8, (y % 256) as u8, ((x * y) % 256) as u8]));
            let img = if i == n - 1 { image::imageops::blur(&img, 6.0) } else { img };
            img.save(dir.join(format!("p{i}.jpg"))).unwrap();
        }
    }

    /// 探测、分析（第二次走缓存）、取舍、项目文件：桌面端与 Android 走的就是这一条。
    #[test]
    fn a_project_is_probed_scanned_from_cache_and_decided() {
        let (e, root, events) = engine("scan");
        let dir = root.join("photos");
        photos(&dir, 4);
        let dirs = vec![dir.to_string_lossy().into_owned()];
        let probe = e.probe_folder(&dirs, None, Some("fast")).unwrap();
        assert_eq!((probe.files.len(), probe.uncached), (4, 4));
        assert!(probe.files.iter().all(|f| (f.width, f.height) == (640, 480)));
        let args = ScanArgs { key: "p1".into(), dirs: dirs.clone(), quality: Some("fast".into()), ..Default::default() };
        let first = e.scan_project(args.clone()).unwrap();
        assert_eq!(first.len(), 4);
        assert!(events.0.lock().unwrap().iter().any(|(n, _)| n == "scan-photo"));
        // 第二次全部命中缓存
        let probe = e.probe_folder(&dirs, None, Some("fast")).unwrap();
        assert_eq!(probe.uncached, 0);
        events.0.lock().unwrap().clear();
        let again = e.scan_project(args).unwrap();
        assert_eq!(again.len(), 4);
        assert!(!events.0.lock().unwrap().iter().any(|(n, _)| n == "scan-photo"), "nothing was analysed again");
        let d = e.decide_photos("p1", 1.0, None, false, &HashMap::new()).unwrap();
        assert_eq!(d.len(), 4);
        assert!(e.decide_photos("other", 1.0, None, false, &HashMap::new()).is_err(), "another project's photos are refused");
        // 项目文件：写了能读回，读不出来的能挪到一旁
        assert_eq!(e.projects_load().unwrap(), None);
        e.projects_save(r#"{"projects":[],"activeId":null}"#).unwrap();
        assert!(e.projects_load().unwrap().unwrap().contains("projects"));
        let aside = e.projects_quarantine().unwrap();
        assert!(Path::new(&aside).exists() && e.projects_load().unwrap().is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 转照片：记录换缩略图和显示尺寸，原文件不动；大图与清晰块按角度另存；强制重新分析也记得角度。
    #[test]
    fn a_photo_can_be_turned_and_stays_turned() {
        let (e, root, _) = engine("rotate");
        let dir = root.join("photos");
        photos(&dir, 2);
        let dirs = vec![dir.to_string_lossy().into_owned()];
        let args = ScanArgs { key: "p".into(), dirs: dirs.clone(), quality: Some("fast".into()), ..Default::default() };
        let first = e.scan_project(args.clone()).unwrap();
        let p0 = first.iter().find(|p| p.name == "p0.jpg").unwrap().clone();
        assert_eq!((p0.width, p0.height, p0.rotation), (640, 480, 0));
        let turned = e.set_rotation(&p0.path, 90).unwrap();
        assert_eq!((turned.width, turned.height, turned.rotation), (480, 640, 90));
        assert_ne!(turned.thumb, p0.thumb);
        let th = image::image_dimensions(&turned.thumb).unwrap();
        assert!(th.0 < th.1, "the thumbnail stands upright now: {th:?}");
        // 大图：转过的照片总是渲染出来，不给原文件
        let big = e.large_preview(&p0.path, true, true).unwrap();
        assert_ne!(big, p0.path);
        let bd = image::image_dimensions(&big).unwrap();
        assert!(bd.0 < bd.1, "{bd:?}");
        // 清晰块：屏幕上右上角那一块（转过之后的坐标），裁出来是竖的
        let crop = e.photo_detail(&p0.path, 0.5, 0.0, 0.5, 0.5, 200).unwrap();
        let cd = image::image_dimensions(&crop).unwrap();
        assert!((190..=210).contains(&cd.0) && cd.1 > cd.0, "{cd:?}");
        // 强制重新分析：文件没变，角度留着
        let again = e.scan_project(ScanArgs { force: true, ..args.clone() }).unwrap();
        let q = again.iter().find(|p| p.path == p0.path).unwrap();
        assert_eq!((q.width, q.height, q.rotation), (480, 640, 90));
        // 转回去就是原来的缩略图
        let back = e.set_rotation(&p0.path, 0).unwrap();
        assert_eq!((back.width, back.height, back.rotation, back.thumb.as_str()), (640, 480, 0, p0.thumb.as_str()));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 取消：分析中途换了任务号就停下，返回 "cancelled"。
    #[test]
    fn a_cancelled_scan_says_so() {
        let (e, root, _) = engine("cancel");
        let dir = root.join("photos");
        photos(&dir, 3);
        let e2 = e.clone();
        // 在另一个线程里一直取消，直到分析返回
        let stop = Arc::new(AtomicBool::new(false));
        let s2 = stop.clone();
        let t = std::thread::spawn(move || {
            while !s2.load(Ordering::Relaxed) {
                e2.scan_cancel();
                std::thread::yield_now();
            }
        });
        let r = e.scan_project(ScanArgs { key: "k".into(), dirs: vec![dir.to_string_lossy().into_owned()], ..Default::default() });
        stop.store(true, Ordering::Relaxed);
        t.join().unwrap();
        assert_eq!(r.err().as_deref(), Some("cancelled"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 回收：只收照片；本机照片在没有注册废纸篓的平台上交给界面（`deferred`）。
    #[test]
    fn trash_accepts_only_photos_and_defers_local_ones_without_a_trash() {
        let (e, root, _) = engine("trash");
        let dir = root.join("t");
        std::fs::create_dir_all(&dir).unwrap();
        let photo = dir.join("fg-test-photo.jpg");
        let other = dir.join("notes.txt");
        std::fs::write(&photo, b"not really a jpeg").unwrap();
        std::fs::write(&other, b"keep me").unwrap();
        let r = e.trash_photos(vec![photo.to_string_lossy().into(), other.to_string_lossy().into()]);
        assert_eq!(r.failed, vec![other.to_string_lossy().into_owned()]);
        if crate::platform::local_trash().is_none() {
            assert_eq!(r.deferred, vec![photo.to_string_lossy().into_owned()]);
            assert!(photo.exists(), "nothing is touched when the host does it");
        }
        assert!(other.exists(), "a non-photo must never be touched");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 我的设置与喜好：导出的字节能原样导回来；风格与标注走引擎。
    #[test]
    fn taste_round_trips_and_styles_are_kept() {
        let (e, root, _) = engine("taste");
        let id = e.style_create("Street").into_iter().find(|s| s.name == "Street").unwrap().id;
        assert!(e.styles_list().iter().any(|s| s.name == "Street"));
        let info = e.record_label("/x/a.jpg", Some(true), Some(&id));
        assert_eq!(info.keeps, 1);
        let (bytes, _) = e.taste_pack_bytes(serde_json::json!({"theme": "dark"})).unwrap();
        let (_, settings) = e.taste_import_bytes(&bytes).unwrap();
        assert_eq!(settings["theme"], "dark");
        assert!(e.taste_import_bytes(b"not json").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 查看器：大图、局部（没有系统解码器时用 `image` crate 裁）、文件信息。
    #[test]
    fn the_viewer_gets_a_large_preview_a_detail_and_file_facts() {
        let (e, root, _) = engine("viewer");
        let dir = root.join("v");
        photos(&dir, 1);
        let p = dir.join("p0.jpg").to_string_lossy().into_owned();
        let large = e.large_preview(&p, false, false).unwrap();
        assert!(Path::new(&large).exists() && large != p);
        assert_eq!(e.large_preview(&p, true, true).unwrap(), p, "a JPEG the viewer can show is shown as it is");
        let detail = e.photo_detail(&p, 0.25, 0.25, 0.5, 0.5, 200).unwrap();
        let d = image::open(&detail).unwrap();
        assert!(d.width() <= 320 && d.width() >= 150, "{}", d.width());
        let info = e.photo_info(&p);
        assert_eq!(info.format.as_deref(), Some("JPEG"));
        assert!(info.bytes > 0);
        assert!(e.cache_size() > 0);
        e.clear_cache().unwrap();
        assert_eq!(e.cache_size(), 0);
        let _ = std::fs::remove_dir_all(&root);
    }
}
