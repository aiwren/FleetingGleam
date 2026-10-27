//! macOS 桌面端：Tauri 命令、窗口与菜单、macOS 的边车（Apple Vision、ImageIO、HDR 叠层、Metal 上的 CLIP）。
//! 全部应用流程在共用核心库的引擎里（`fg_core::engine`，Android 用的是同一个）；这里的命令只是把它放到后台线程、把事件交给 WebView。
//! 命令名和参数与以前一样，前端不用动。

mod clipgpu;
mod hdrview;
mod imageio_macos;
mod mounts;
mod power;
mod proxy;
mod samples;
mod vision;
#[cfg(test)]
mod bench;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use fg_core::analyze::Photo;
use fg_core::engine::{self, BackupMeta, BackupStatus, Engine, ExportResult, FileInfo, Hit, Listing, Machine, Probe, ScanArgs, SceneReq, SearchStatus, TargetArgs, TrashResult};
use fg_core::{decide, modelfiles, modelpacks, nas, prefs, taste};
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};

/// 引擎的事件原样交给 WebView（事件名和载荷就是前端监听的那些）。
struct WebviewEvents(AppHandle);

impl engine::Events for WebviewEvents {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        if event == "scan-photo" || event == "scan-photos" {
            remember_sent(&payload);
        }
        let _ = self.0.emit(event, payload);
    }
}

/// 分析中经 "scan-photo" / "scan-photos" 送到前端的照片（按路径）：scan_project 结束时只回传送过之后又变了的字段
/// （分组、打分、补量的眼睛和人脸……），不再把几千张照片整包再传一遍（审查第二批）。只在有真正的分析在跑时记。
#[derive(Default)]
struct Sent {
    scans: usize,
    photos: HashMap<String, serde_json::Map<String, serde_json::Value>>,
}

fn sent() -> &'static Mutex<Sent> {
    static SENT: std::sync::OnceLock<Mutex<Sent>> = std::sync::OnceLock::new();
    SENT.get_or_init(Mutex::default)
}

fn remember_sent(payload: &serde_json::Value) {
    let mut s = sent().lock().unwrap_or_else(|e| e.into_inner());
    if s.scans == 0 {
        return;
    }
    let one = |s: &mut Sent, v: &serde_json::Value| {
        if let Some(o) = v.as_object() {
            if let Some(path) = o.get("path").and_then(|p| p.as_str()) {
                s.photos.insert(path.to_string(), o.clone());
            }
        }
    };
    match payload {
        serde_json::Value::Array(list) => list.iter().for_each(|v| one(&mut s, v)),
        v => one(&mut s, v),
    }
}

/// 一次真正的分析期间（含被取消、出错的那些）记着送过的照片；最后一个结束时清掉。
struct SentScan;

impl SentScan {
    fn begin() -> Self {
        sent().lock().unwrap_or_else(|e| e.into_inner()).scans += 1;
        SentScan
    }
}

impl Drop for SentScan {
    fn drop(&mut self) {
        let mut s = sent().lock().unwrap_or_else(|e| e.into_inner());
        s.scans = s.scans.saturating_sub(1);
        if s.scans == 0 {
            s.photos = HashMap::new();
        }
    }
}

/// 一张照片的最终记录，减去事件里送过、而且没变的字段（`path` 总在）；没送过的照整张给。
fn slim(full: serde_json::Value, sent: Option<&serde_json::Map<String, serde_json::Value>>) -> serde_json::Value {
    match (full, sent) {
        (serde_json::Value::Object(full), Some(sent)) => serde_json::Value::Object(full.into_iter().filter(|(k, v)| k == "path" || sent.get(k) != Some(v)).collect()),
        (full, _) => full,
    }
}

fn data_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("fleetinggleam"))
}

fn thumbs_dir(app: &AppHandle) -> PathBuf {
    app.path().app_cache_dir().unwrap_or_else(|_| std::env::temp_dir().join("fleetinggleam-cache")).join("thumbs")
}

/// Where downloaded models live.
fn downloaded_models_dir(app: &AppHandle) -> PathBuf {
    data_dir(app).join("models")
}

/// Models are downloaded on demand into the app's data folder. Only a debug build may use the repo's `models/` folder instead
/// (set FG_FORCE_DOWNLOAD=1 to test the download path); a release build carries no development paths at all.
fn models_dir(app: &AppHandle) -> PathBuf {
    #[cfg(debug_assertions)]
    {
        let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../models");
        if std::env::var("FG_FORCE_DOWNLOAD").is_err() && modelfiles::has_model(&dev, "clip_vit_b32_vision_int8.onnx") {
            return dev;
        }
    }
    downloaded_models_dir(app)
}

fn image_bin(app: &AppHandle) -> PathBuf {
    let bundled = app.path().resource_dir().map(|d| d.join("bin/fg-image")).unwrap_or_default();
    #[cfg(debug_assertions)]
    {
        if !bundled.exists() {
            return PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/fg-image");
        }
    }
    bundled
}

fn vision_bin(app: &AppHandle) -> PathBuf {
    let bundled = app.path().resource_dir().map(|d| d.join("bin/fg-vision")).unwrap_or_default();
    #[cfg(debug_assertions)]
    {
        if !bundled.exists() {
            return PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/fg-vision");
        }
    }
    bundled
}

fn engine(app: &AppHandle) -> Arc<Engine> {
    app.state::<Arc<Engine>>().inner().clone()
}

/// 引擎的方法都是阻塞的：放到后台线程上跑，命令本身不占主线程。
async fn blocking<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> Result<R, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- 探测、分析、取舍

/// 彻底删除项目时：忘掉这些文件夹里照片的分析结果与缩略图（`keep` 里还被别的项目用着的除外）。
#[tauri::command]
async fn forget_folders(app: AppHandle, dirs: Vec<String>, keep: Vec<String>) -> Result<usize, String> {
    let e = engine(&app);
    blocking(move || e.forget_folders(&dirs, &keep)).await
}

#[tauri::command]
async fn probe_folder(app: AppHandle, dirs: Vec<String>, compute: Option<String>, quality: Option<String>) -> Result<Probe, String> {
    let e = engine(&app);
    blocking(move || e.probe_folder(&dirs, compute.as_deref(), quality.as_deref())).await?
}

#[tauri::command]
/// 安静读取（从缓存打开）回完整的照片；真正的分析只回每张在事件之后又变了的字段（见 `slim`），前端和事件攒下的合起来。
async fn scan_project(app: AppHandle, key: String, dirs: Vec<String>, compute: Option<String>, quality: Option<String>, learn: bool, quiet: bool, style: Option<String>, force: Option<bool>, eye_method: Option<String>, live: Option<fg_core::engine::LiveArgs>) -> Result<Vec<serde_json::Value>, String> {
    let e = engine(&app);
    let args = ScanArgs { key, dirs, compute, quality, learn, quiet, style, force: force.unwrap_or(false), eye_method, live };
    let _awake = power::Awake::begin("FleetingGleam: analysing photos");
    let _sent = (!quiet).then(SentScan::begin);
    let photos: Vec<Photo> = blocking(move || e.scan_project(args)).await??;
    let mut s = sent().lock().unwrap_or_else(|e| e.into_inner());
    Ok(photos
        .into_iter()
        .map(|p| {
            let v = serde_json::to_value(&p).unwrap_or_default();
            if quiet {
                v
            } else {
                slim(v, s.photos.remove(&p.path).as_ref())
            }
        })
        .collect())
}

/// 带 key 只停那个项目的分析 / 读取（离开项目时它自己的停，后台分析的那个照跑）；不带就全停。
#[tauri::command]
fn scan_cancel(app: AppHandle, key: Option<String>) {
    match key {
        Some(k) => engine(&app).scan_cancel_key(&k),
        None => engine(&app).scan_cancel(),
    }
}

/// 最近一次 decide_photos 算出的整张判定表：理由按需从这里取（`decide_reasons`），路径顺序带一个版本号，没变就不再整份发给前端。
#[derive(Default)]
struct DecideCache {
    key: String,
    rev: u64,
    paths: Vec<String>,
    table: HashMap<String, decide::Decision>,
}

#[derive(Default)]
struct DecideState(Mutex<DecideCache>);

/// decide_photos 回给前端的紧凑格式（审查第二批：以前每点一次整张表连理由来回一趟，万张项目几 MB JSON）。
#[derive(serde::Serialize, Debug, PartialEq)]
struct CompactDecisions {
    /// 路径表的版本：前端带着上次拿到的来，一样就不再发 `paths`
    rev: u64,
    /// 按路径排好的全部照片；只在和前端手里的版本不同时给
    #[serde(skip_serializing_if = "Option::is_none")]
    paths: Option<Vec<String>>,
    /// 每张一个字符，顺序同 `paths`：k = 保留、r = 待复查、x = 淘汰
    states: String,
    /// 手动标过的那些在 `paths` 里的下标
    manual: Vec<u32>,
}

/// 把核心的判定表转成紧凑格式，并记进缓存（核心不动，转换只在这层封装里）。
fn compact(cache: &mut DecideCache, key: &str, table: HashMap<String, decide::Decision>, known_rev: Option<u64>) -> CompactDecisions {
    let mut paths: Vec<String> = table.keys().cloned().collect();
    paths.sort_unstable();
    if cache.key != key || cache.paths != paths {
        cache.rev += 1;
        cache.key = key.to_string();
        cache.paths = paths;
    }
    let states = cache
        .paths
        .iter()
        .map(|p| match table[p].state {
            decide::State::Keep => 'k',
            decide::State::Review => 'r',
            decide::State::Reject => 'x',
        })
        .collect();
    let manual = cache.paths.iter().enumerate().filter(|(_, p)| table[*p].manual).map(|(i, _)| i as u32).collect();
    cache.table = table;
    CompactDecisions { rev: cache.rev, paths: (known_rev != Some(cache.rev)).then(|| cache.paths.clone()), states, manual }
}

/// Deciding is seconds on a very large project, so it runs off the main thread.
/// `known_rev`：前端手里那份路径表的版本（见 `CompactDecisions`）。
#[tauri::command]
async fn decide_photos(app: AppHandle, key: String, keep_ratio: f32, pick_mode: Option<String>, one_per_group: Option<bool>, overrides: HashMap<String, String>, known_rev: Option<u64>) -> Result<CompactDecisions, String> {
    let e = engine(&app);
    blocking(move || {
        let table = e.decide_photos(&key, keep_ratio, pick_mode.as_deref(), one_per_group.unwrap_or(false), &overrides)?;
        let state = app.state::<DecideState>();
        let mut cache = state.0.lock().unwrap_or_else(|e| e.into_inner());
        Ok(compact(&mut cache, &key, table, known_rev))
    })
    .await?
}

/// 一张照片的判定理由（检查器里那一句），取自最近一次 decide_photos；那次不是这个项目的就是 null。
#[tauri::command]
fn decide_reasons(app: AppHandle, key: String, path: String) -> Option<Vec<decide::Reason>> {
    let state = app.state::<DecideState>();
    let cache = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if cache.key != key {
        return None;
    }
    cache.table.get(&path).map(|d| d.reasons.clone())
}

// ---------------------------------------------------------------- 项目文件

#[tauri::command]
async fn projects_load(app: AppHandle) -> Result<Option<String>, String> {
    let e = engine(&app);
    blocking(move || e.projects_load()).await?
}

#[tauri::command]
async fn projects_save(app: AppHandle, json: String) -> Result<(), String> {
    let e = engine(&app);
    blocking(move || e.projects_save(&json)).await?
}

#[tauri::command]
async fn projects_quarantine(app: AppHandle) -> Result<String, String> {
    let e = engine(&app);
    blocking(move || e.projects_quarantine()).await?
}

// ---------------------------------------------------------------- 我的设置与喜好、云备份

#[tauri::command]
async fn taste_export(app: AppHandle, path: String, settings: serde_json::Value) -> Result<taste::Summary, String> {
    let e = engine(&app);
    blocking(move || e.taste_export(&path, settings)).await?
}

#[tauri::command]
async fn taste_import(app: AppHandle, path: String) -> Result<(taste::Summary, serde_json::Value), String> {
    let e = engine(&app);
    blocking(move || e.taste_import(&path)).await?
}

#[tauri::command]
fn backup_status(app: AppHandle) -> BackupStatus {
    engine(&app).backup_status()
}

#[tauri::command]
async fn backup_save_target(app: AppHandle, id: Option<String>, name: String, kind: String, account: String, bucket: String, access_key: String, secret: Option<String>) -> Result<BackupStatus, String> {
    let e = engine(&app);
    blocking(move || e.backup_save_target(TargetArgs { id, name, kind, account, bucket, access_key, secret })).await?
}

#[tauri::command]
async fn backup_remove_target(app: AppHandle, id: String) -> Result<BackupStatus, String> {
    let e = engine(&app);
    blocking(move || e.backup_remove_target(&id)).await?
}

#[tauri::command]
async fn backup_set_options(app: AppHandle, enabled: Option<bool>, keep: Option<usize>) -> Result<BackupStatus, String> {
    let e = engine(&app);
    blocking(move || e.backup_set_options(enabled, keep)).await?
}

#[tauri::command]
async fn backup_now(app: AppHandle, settings: serde_json::Value) -> Result<BackupStatus, String> {
    let e = engine(&app);
    blocking(move || e.backup_now(settings)).await?
}

#[tauri::command]
async fn backup_restore(app: AppHandle, id: String) -> Result<(taste::Summary, serde_json::Value), String> {
    let e = engine(&app);
    blocking(move || e.backup_restore(&id)).await?
}

// ---------------------------------------------------------------- 网络共享（NAS）

#[tauri::command]
async fn nas_probe(app: AppHandle, host: String, user: String, password: String, id: Option<String>) -> Result<Vec<String>, String> {
    let e = engine(&app);
    blocking(move || e.nas_probe(&host, &user, &password, id.as_deref())).await?
}

#[tauri::command]
async fn nas_update(app: AppHandle, id: String, name: String, host: String, user: String, password: String, remember: bool) -> Result<Vec<nas::ShareStatus>, String> {
    let e = engine(&app);
    blocking(move || e.nas_update(&id, &name, &host, &user, &password, remember)).await?
}

#[tauri::command]
async fn nas_list(app: AppHandle) -> Vec<nas::ShareStatus> {
    let e = engine(&app);
    blocking(move || e.nas_list()).await.unwrap_or_default()
}

#[tauri::command]
async fn nas_add(app: AppHandle, name: String, host: String, user: String, password: String, remember: bool) -> Result<Vec<nas::ShareStatus>, String> {
    let e = engine(&app);
    blocking(move || e.nas_add(&name, &host, &user, &password, remember)).await?
}

#[tauri::command]
async fn nas_remove(app: AppHandle, id: String) -> Result<Vec<nas::ShareStatus>, String> {
    let e = engine(&app);
    blocking(move || e.nas_remove(&id)).await?
}

#[tauri::command]
async fn nas_connect(app: AppHandle, id: String, password: Option<String>) -> Result<Vec<nas::ShareStatus>, String> {
    let e = engine(&app);
    blocking(move || e.nas_connect(&id, password.as_deref())).await?
}

#[tauri::command]
async fn nas_disconnect(app: AppHandle, id: String) -> Result<Vec<nas::ShareStatus>, String> {
    let e = engine(&app);
    blocking(move || e.nas_disconnect(&id)).await
}

#[tauri::command]
async fn nas_set_enabled(app: AppHandle, id: String, enabled: bool) -> Result<Vec<nas::ShareStatus>, String> {
    let e = engine(&app);
    blocking(move || e.nas_set_enabled(&id, enabled)).await?
}

#[tauri::command]
async fn nas_ensure(app: AppHandle, dirs: Vec<String>) -> Vec<nas::ShareStatus> {
    let e = engine(&app);
    blocking(move || e.nas_ensure(&dirs)).await.unwrap_or_default()
}

#[tauri::command]
async fn nas_browse(app: AppHandle, id: String, path: String) -> Result<Listing, String> {
    let e = engine(&app);
    blocking(move || e.nas_browse(&id, &path)).await?
}

/// 每台 NAS 的本机副本占多少。
#[tauri::command]
async fn nas_mirror_usage(app: AppHandle) -> Result<Vec<fg_core::engine::MirrorUsage>, String> {
    let e = engine(&app);
    blocking(move || e.nas_mirror_usage()).await
}

/// 清掉一台 NAS 的本机副本，返回释放的字节数。
#[tauri::command]
async fn nas_mirror_clear(app: AppHandle, id: String) -> Result<u64, String> {
    let e = engine(&app);
    blocking(move || e.nas_mirror_clear(&id)).await?
}

// ---------------------------------------------------------------- 整理

#[tauri::command]
async fn export_photos(app: AppHandle, paths: Vec<String>, dest: String) -> Result<ExportResult, String> {
    let e = engine(&app);
    blocking(move || e.export_photos(&paths, &dest)).await?
}

/// 让正在进行的导出做完当前这张就停。
#[tauri::command]
fn export_cancel(app: AppHandle) {
    engine(&app).export_cancel();
}

/// Moves photos to the system trash (recoverable from Finder). Only image files are accepted, so this can never be used to trash anything else.
#[tauri::command]
async fn trash_photos(app: AppHandle, paths: Vec<String>) -> Result<TrashResult, String> {
    let e = engine(&app);
    blocking(move || e.trash_photos(paths)).await
}

/// 拖进窗口的东西里能当项目文件夹的：文件夹本身；拖的是照片文件就取它所在的文件夹。去重，保持拖入的顺序。
#[tauri::command]
fn dropped_folders(paths: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in paths {
        let path = std::path::Path::new(&p);
        let dir = if path.is_dir() {
            Some(path.to_path_buf())
        } else if path.is_file() && fg_core::analyze::is_image(path) {
            path.parent().map(|d| d.to_path_buf())
        } else {
            None
        };
        if let Some(d) = dir.map(|d| d.to_string_lossy().trim_end_matches('/').to_string()).filter(|d| !d.is_empty()) {
            if !out.contains(&d) {
                out.push(d);
            }
        }
    }
    out
}

/// Shows a file (or folder) in Finder / Explorer.
#[tauri::command]
fn reveal(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").args(["-R", &path]).spawn();
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("explorer").arg(format!("/select,{path}")).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let r = std::process::Command::new("xdg-open").arg(std::path::Path::new(&path).parent().unwrap_or(std::path::Path::new("/"))).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- 查看器

#[tauri::command]
async fn preview_thumbs(app: AppHandle, paths: Vec<String>) -> Result<Vec<Option<String>>, String> {
    let e = engine(&app);
    blocking(move || e.preview_thumbs(&paths)).await
}

#[tauri::command]
async fn photo_info(app: AppHandle, path: String) -> Result<FileInfo, String> {
    let e = engine(&app);
    blocking(move || e.photo_info(&path)).await
}

/// Path of a large cached preview for the viewer (created on first request).
#[tauri::command]
async fn large_preview(app: AppHandle, path: String, full: Option<bool>) -> Result<String, String> {
    // the web view may only load files inside the asset scope; anything else (a mounted share under /private, a symlink target) gets a copy
    let showable = app.asset_protocol_scope().is_allowed(&path);
    let e = engine(&app);
    blocking(move || e.large_preview(&path, full.unwrap_or(false), showable)).await?
}

#[tauri::command]
async fn photo_detail(app: AppHandle, path: String, x: f64, y: f64, w: f64, h: f64, out_w: u32) -> Result<String, String> {
    let e = engine(&app);
    blocking(move || e.photo_detail(&path, x, y, w, h, out_w)).await?
}

/// 把一张照片转到 `deg`（顺时针，0 / 90 / 180 / 270）；返回转过的记录。
#[tauri::command]
async fn set_rotation(app: AppHandle, path: String, deg: i64) -> Result<Photo, String> {
    let e = engine(&app);
    blocking(move || e.set_rotation(&path, deg)).await?
}

/// 某个备份目标里最新一份的说明（哪台设备、什么时候）。
#[tauri::command]
async fn backup_peek(app: AppHandle, id: String) -> Result<Option<BackupMeta>, String> {
    let e = engine(&app);
    blocking(move || e.backup_peek(&id)).await?
}

/// 这台电脑的名字（系统偏好里的"电脑名称"），写进备份让另一端认得出来。
fn device_name() -> String {
    let run = |c: &str, args: &[&str]| std::process::Command::new(c).args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|s| !s.is_empty());
    run("scutil", &["--get", "ComputerName"]).or_else(|| run("hostname", &[])).unwrap_or_else(|| "Mac".to_string())
}

#[tauri::command]
async fn motion_clip(app: AppHandle, path: String) -> Result<String, String> {
    let e = engine(&app);
    blocking(move || e.motion_clip(&path)).await?
}

// ---------------------------------------------------------------- 搜索与场景

#[tauri::command]
async fn search_status(app: AppHandle, dirs: Vec<String>) -> Result<SearchStatus, String> {
    let e = engine(&app);
    blocking(move || e.search_status(&dirs)).await
}

#[tauri::command]
async fn search_index(app: AppHandle, dirs: Vec<String>) -> Result<usize, String> {
    let e = engine(&app);
    blocking(move || e.search_index(&dirs)).await?
}

#[tauri::command]
async fn search_photos(app: AppHandle, dirs: Vec<String>, query: String) -> Result<Vec<Hit>, String> {
    let e = engine(&app);
    blocking(move || e.search_photos(&dirs, &query)).await?
}

#[tauri::command]
async fn scene_text(app: AppHandle, scenes: Vec<SceneReq>, zh: bool, caption: bool) -> Result<(), String> {
    let e = engine(&app);
    blocking(move || e.scene_text(&scenes, zh, caption)).await?
}

// ---------------------------------------------------------------- 模型包与机器

#[tauri::command]
fn machine_info(app: AppHandle) -> Machine {
    engine(&app).machine_info()
}

#[tauri::command]
async fn packs_status(app: AppHandle) -> Result<Vec<modelpacks::PackStatus>, String> {
    let e = engine(&app);
    // `detect`（人脸检测器）是给没有系统人脸检测的平台（Android）用的，Mac 端用 Apple Vision，设置页不列它
    blocking(move || e.packs_status(false).into_iter().filter(|p| p.id != "detect").collect()).await
}

#[tauri::command]
async fn pack_size(app: AppHandle, id: String) -> Result<u64, String> {
    let e = engine(&app);
    blocking(move || e.pack_size(&id)).await?
}

#[tauri::command]
async fn pack_download(app: AppHandle, id: String) -> Result<(), String> {
    let e = engine(&app);
    blocking(move || e.pack_download(&id)).await?
}

#[tauri::command]
fn pack_cancel(app: AppHandle) {
    engine(&app).pack_cancel();
}

#[tauri::command]
async fn pack_remove(app: AppHandle, id: String) -> Result<(), String> {
    let e = engine(&app);
    blocking(move || e.pack_remove(&id)).await?
}

// ---------------------------------------------------------------- 喜好与风格

#[tauri::command]
async fn record_label(app: AppHandle, path: String, label: Option<bool>, style: Option<String>) -> Result<prefs::PrefsInfo, String> {
    Ok(engine(&app).record_label(&path, label, style.as_deref()))
}

#[tauri::command]
fn prefs_info(app: AppHandle, style: Option<String>) -> prefs::PrefsInfo {
    engine(&app).prefs_info(style.as_deref())
}

#[tauri::command]
fn prefs_reset(app: AppHandle, style: Option<String>) -> prefs::PrefsInfo {
    engine(&app).prefs_reset(style.as_deref())
}

#[tauri::command]
fn styles_list(app: AppHandle) -> Vec<prefs::StyleInfo> {
    engine(&app).styles_list()
}

#[tauri::command]
fn style_create(app: AppHandle, name: String) -> Vec<prefs::StyleInfo> {
    engine(&app).style_create(&name)
}

#[tauri::command]
fn style_rename(app: AppHandle, id: String, name: String) -> Vec<prefs::StyleInfo> {
    engine(&app).style_rename(&id, &name)
}

#[tauri::command]
fn style_delete(app: AppHandle, id: String) -> Vec<prefs::StyleInfo> {
    engine(&app).style_delete(&id)
}

#[tauri::command]
fn style_set_default(app: AppHandle, id: String) -> Vec<prefs::StyleInfo> {
    engine(&app).style_set_default(&id)
}

#[tauri::command]
async fn learn_from_folder(app: AppHandle, dir: String, style: Option<String>, compute: Option<String>, quality: Option<String>) -> Result<prefs::PrefsInfo, String> {
    let e = engine(&app);
    let _awake = power::Awake::begin("FleetingGleam: learning from a folder");
    blocking(move || e.learn_from_folder(&dir, style.as_deref(), compute.as_deref(), quality.as_deref())).await?
}

// ---------------------------------------------------------------- 缓存、菜单

#[tauri::command]
async fn cache_size(app: AppHandle) -> Result<u64, String> {
    let e = engine(&app);
    blocking(move || e.cache_size()).await
}

#[tauri::command]
async fn clear_cache(app: AppHandle) -> Result<(), String> {
    let e = engine(&app);
    blocking(move || e.clear_cache()).await?
}

struct MenuText {
    settings: &'static str,
    file: &'static str,
    edit: &'static str,
    view: &'static str,
    window: &'static str,
    open: &'static str,
    sidebar: &'static str,
    inspector: &'static str,
}

fn menu_text(locale: &str) -> MenuText {
    if locale.starts_with("zh") {
        MenuText { settings: "设置…", file: "文件", edit: "编辑", view: "视图", window: "窗口", open: "打开文件夹…", sidebar: "显示/隐藏侧边栏", inspector: "显示/隐藏检查器" }
    } else {
        MenuText { settings: "Settings…", file: "File", edit: "Edit", view: "View", window: "Window", open: "Open Folder…", sidebar: "Toggle Sidebar", inspector: "Toggle Inspector" }
    }
}

fn build_menu(app: &AppHandle, locale: &str) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let tx = menu_text(locale);
    let open = MenuItemBuilder::with_id("open-folder", tx.open).accelerator("CmdOrCtrl+O").build(app)?;
    let sidebar = MenuItemBuilder::with_id("toggle-sidebar", tx.sidebar).accelerator("CmdOrCtrl+Alt+S").build(app)?;
    let inspector = MenuItemBuilder::with_id("toggle-inspector", tx.inspector).accelerator("CmdOrCtrl+Alt+I").build(app)?;
    let settings = MenuItemBuilder::with_id("open-settings", tx.settings).accelerator("CmdOrCtrl+,").build(app)?;
    let app_menu = SubmenuBuilder::new(app, "FleetingGleam")
        .about(None)
        .separator()
        .item(&settings)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;
    let file = SubmenuBuilder::new(app, tx.file).item(&open).separator().close_window().build()?;
    let edit = SubmenuBuilder::new(app, tx.edit)
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let view = SubmenuBuilder::new(app, tx.view).item(&sidebar).item(&inspector).separator().fullscreen().build()?;
    let window = SubmenuBuilder::new(app, tx.window).minimize().maximize().build()?;
    MenuBuilder::new(app).items(&[&app_menu, &file, &edit, &view, &window]).build()
}

/// The frontend owns the language; it tells us so the native menu bar follows.
#[tauri::command]
fn set_locale(app: AppHandle, locale: String) -> Result<(), String> {
    let menu = build_menu(&app, &locale).map_err(|e| e.to_string())?;
    app.set_menu(menu).map(|_| ()).map_err(|e| e.to_string())
}

/// 导出诊断信息（版本、系统、设置、日志、崩溃记录，去掉一切密码令牌）到 `out`，返回写出的路径。
#[tauri::command]
async fn diagnostics_export(app: AppHandle, out: String) -> Result<String, String> {
    let e = engine(&app);
    blocking(move || e.diagnostics_export(&out)).await?
}

/// 日志与崩溃记录的目录。
#[tauri::command]
fn diagnostics_dir(app: AppHandle) -> String {
    engine(&app).diagnostics_dir()
}

// ---------------------------------------------------------------- 退出前冲刷

/// 前端把还在防抖里的 projects.json 写完了（或等太久不等了）：可以真的退出。
static QUIT_READY: AtomicBool = AtomicBool::new(false);
/// 已经请前端冲刷过一次（重复的 ⌘Q / 关窗不再重复发）。
static QUIT_ASKED: AtomicBool = AtomicBool::new(false);

/// 要退出（⌘Q、关掉最后一个窗口）时先拦下来，请前端把还没写盘的项目写完；前端没反应时 2 秒后照样退出。
fn ask_flush(app: &AppHandle) {
    if QUIT_ASKED.swap(true, Ordering::SeqCst) {
        return;
    }
    if app.emit("flush-before-quit", ()).is_err() {
        quit_now(app);
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        quit_now(&app);
    });
}

fn quit_now(app: &AppHandle) {
    if !QUIT_READY.swap(true, Ordering::SeqCst) {
        app.exit(0);
    }
}

/// 前端冲刷完了。
#[tauri::command]
fn quit_ready(app: AppHandle) {
    quit_now(&app);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 单实例必须第一个注册：第二次启动（再点一次图标、开发版与正式版交替）不再起第二个进程，而是把已有的窗口叫到前面
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        // 窗口的大小、位置、最大化与全屏（不管可见性和标题栏样式，那两样由 tauri.conf.json 定）
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(tauri_plugin_window_state::StateFlags::SIZE | tauri_plugin_window_state::StateFlags::POSITION | tauri_plugin_window_state::StateFlags::MAXIMIZED | tauri_plugin_window_state::StateFlags::FULLSCREEN)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            // macOS 的平台能力：ImageIO 边车、Apple Vision 边车、Metal 上的 CLIP、访达的废纸篓
            imageio_macos::Helper::register(image_bin(&handle));
            fg_core::platform::set_vision(Box::new(vision::Factory { bin: vision_bin(&handle) }));
            fg_core::platform::set_gpu_clip(Box::new(clipgpu::Factory));
            fg_core::platform::set_local_trash(Box::new(mounts::DesktopTrash));
            proxy::init(data_dir(&handle));
            let paths = engine::Paths { data: data_dir(&handle), thumbs: thumbs_dir(&handle), models: models_dir(&handle), downloads: downloaded_models_dir(&handle) };
            let version = handle.package_info().version.to_string();
            let e = Engine::new(paths, Box::new(WebviewEvents(handle.clone())), &version);
            e.set_device(&device_name());
            // 启用的 NAS 在后台连上；缩略图旁边的草稿文件（大图、局部、动态照片的视频）修剪到上限以内
            e.start_background();
            app.manage(e);
            app.manage(DecideState::default());
            let menu = build_menu(&handle, "en")?;
            app.set_menu(menu)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            let _ = app.emit("menu", event.id().0.clone());
        })
        // 关窗（这个应用只有一个窗口，关掉就是退出）：先请前端把项目写完
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if !QUIT_READY.load(Ordering::SeqCst) {
                    api.prevent_close();
                    ask_flush(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![forget_folders, probe_folder, scan_project, scan_cancel, decide_photos, decide_reasons, projects_load, projects_save, projects_quarantine, taste_export, taste_import, nas_list, nas_add, nas_remove, nas_connect, nas_disconnect, nas_ensure, nas_browse, nas_probe, nas_update, nas_set_enabled, nas_mirror_usage, nas_mirror_clear, backup_status, backup_save_target, backup_remove_target, backup_set_options, backup_now, backup_restore, backup_peek, set_rotation, set_locale, cache_size, clear_cache, record_label, prefs_info, prefs_reset, styles_list, style_create, style_rename, style_delete, style_set_default, learn_from_folder, export_photos, export_cancel, trash_photos, reveal, preview_thumbs, photo_info, large_preview, motion_clip, hdrview::hdr_overlay_supported, hdrview::hdr_overlay_show, hdrview::hdr_overlay_move, hdrview::hdr_overlay_hide, hdrview::hdr_overlay_detail, hdrview::hdr_overlay_detail_hide, photo_detail, machine_info, search_status, search_index, search_photos, scene_text, packs_status, pack_size, pack_download, pack_cancel, pack_remove, diagnostics_export, diagnostics_dir, proxy::proxy_info, proxy::proxy_set, samples::samples_folder, quit_ready, dropped_folders])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // ⌘Q / 菜单里的退出：同样先冲刷
            if let RunEvent::ExitRequested { api, .. } = event {
                if !QUIT_READY.load(Ordering::SeqCst) {
                    api.prevent_exit();
                    ask_flush(app);
                }
            }
        });
}

#[cfg(test)]
mod compact_tests {
    use super::*;

    #[test]
    fn slim_keeps_only_what_changed_since_the_event() {
        let full = serde_json::json!({ "path": "/a.jpg", "name": "a.jpg", "thumb": "/t/a.jpg", "score": 0.8, "group": 3, "people": [0] });
        let sent = serde_json::json!({ "path": "/a.jpg", "name": "a.jpg", "thumb": "/t/a.jpg", "score": 0.6, "group": 0, "people": [] });
        let out = slim(full.clone(), sent.as_object());
        assert_eq!(out, serde_json::json!({ "path": "/a.jpg", "score": 0.8, "group": 3, "people": [0] }));
        // 没送过的照整张给
        assert_eq!(slim(full.clone(), None), full);
    }

    #[test]
    fn sent_photos_are_only_kept_while_a_scan_runs() {
        remember_sent(&serde_json::json!({ "path": "/x.jpg" }));
        assert!(!sent().lock().unwrap().photos.contains_key("/x.jpg"), "没有分析在跑时不记");
        let guard = SentScan::begin();
        remember_sent(&serde_json::json!([{ "path": "/x.jpg" }, { "path": "/y.jpg" }]));
        assert!(sent().lock().unwrap().photos.contains_key("/y.jpg"));
        drop(guard);
        assert!(sent().lock().unwrap().photos.is_empty());
    }

    fn d(state: decide::State, manual: bool) -> decide::Decision {
        decide::Decision { state, manual, reasons: Vec::new() }
    }

    #[test]
    fn compact_decisions_send_paths_only_when_they_change() {
        let mut cache = DecideCache::default();
        let table = || HashMap::from([("/b.jpg".to_string(), d(decide::State::Reject, true)), ("/a.jpg".to_string(), d(decide::State::Keep, false)), ("/c.jpg".to_string(), d(decide::State::Review, false))]);
        let first = compact(&mut cache, "p1", table(), None);
        assert_eq!(first.paths.as_deref(), Some(&["/a.jpg".to_string(), "/b.jpg".to_string(), "/c.jpg".to_string()][..]));
        assert_eq!(first.states, "kxr");
        assert_eq!(first.manual, vec![1]);
        // 同一批照片、前端带着这个版本：不再发路径
        let again = compact(&mut cache, "p1", table(), Some(first.rev));
        assert_eq!(again.rev, first.rev);
        assert_eq!(again.paths, None);
        // 前端手里是别的版本：照发
        assert!(compact(&mut cache, "p1", table(), Some(first.rev + 7)).paths.is_some());
        // 换了项目（哪怕路径一样）或照片变了：新版本
        let other = compact(&mut cache, "p2", table(), Some(first.rev));
        assert!(other.rev > first.rev && other.paths.is_some());
        let mut fewer = table();
        fewer.remove("/c.jpg");
        let shrunk = compact(&mut cache, "p2", fewer, Some(other.rev));
        assert!(shrunk.rev > other.rev);
        assert_eq!(shrunk.states, "kx");
    }
}

#[cfg(test)]
mod probe_timing {
    use super::*;
    use fg_core::{cache, list_images, oriented_dims};
    use rayon::prelude::*;

    /// 探测阶段各步的耗时：`FG_PROBE_DIR=/path/to/photos cargo test --lib probe_timing -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn where_the_time_goes() {
        let Ok(dir) = std::env::var("FG_PROBE_DIR") else { return };
        let t = std::time::Instant::now();
        let files = list_images(&dir);
        println!("list_images: {} files in {:?}", files.len(), t.elapsed());
        let t = std::time::Instant::now();
        let ok: Vec<&PathBuf> = files.par_iter().filter(|p| oriented_dims(p).is_some()).collect();
        println!("oriented_dims (parallel, 1st pass): {} ok in {:?}", ok.len(), t.elapsed());
        let t = std::time::Instant::now();
        let n = ok.par_iter().filter(|p| oriented_dims(p).is_some()).count();
        println!("oriented_dims (parallel, 2nd pass, warm cache): {n} in {:?}", t.elapsed());
        let t = std::time::Instant::now();
        let keys: Vec<_> = ok.iter().filter_map(|p| cache::file_key(p)).collect();
        println!("file_key (metadata): {} in {:?}", keys.len(), t.elapsed());
        let t = std::time::Instant::now();
        let first = ok.first().unwrap();
        let bytes_read = {
            use std::io::Read;
            let mut f = std::fs::File::open(first).unwrap();
            let mut buf = vec![0u8; 2 * 1024 * 1024];
            f.read(&mut buf).unwrap()
        };
        println!("read first 2MB of one file: {bytes_read} bytes in {:?}", t.elapsed());
    }
}

