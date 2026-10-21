//! macOS 桌面端：Tauri 命令、窗口与菜单、macOS 的边车（Apple Vision、ImageIO、HDR 叠层、Metal 上的 CLIP）。
//! 全部应用流程在共用核心库的引擎里（`fg_core::engine`，Android 用的是同一个）；这里的命令只是把它放到后台线程、把事件交给 WebView。
//! 命令名和参数与以前一样，前端不用动。

mod clipgpu;
mod hdrview;
mod imageio_macos;
mod mounts;
mod vision;
#[cfg(test)]
mod bench;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use fg_core::analyze::Photo;
use fg_core::engine::{self, BackupStatus, Engine, ExportResult, FileInfo, Hit, Listing, Machine, Probe, ScanArgs, SceneReq, SearchStatus, TargetArgs, TrashResult};
use fg_core::{decide, modelfiles, modelpacks, nas, prefs, taste};
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager};

/// 引擎的事件原样交给 WebView（事件名和载荷就是前端监听的那些）。
struct WebviewEvents(AppHandle);

impl engine::Events for WebviewEvents {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        let _ = self.0.emit(event, payload);
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

#[tauri::command]
async fn probe_folder(app: AppHandle, dirs: Vec<String>, compute: Option<String>, quality: Option<String>) -> Result<Probe, String> {
    let e = engine(&app);
    blocking(move || e.probe_folder(&dirs, compute.as_deref(), quality.as_deref())).await?
}

#[tauri::command]
async fn scan_project(app: AppHandle, key: String, dirs: Vec<String>, compute: Option<String>, quality: Option<String>, learn: bool, quiet: bool, style: Option<String>, force: Option<bool>, eye_method: Option<String>) -> Result<Vec<Photo>, String> {
    let e = engine(&app);
    let args = ScanArgs { key, dirs, compute, quality, learn, quiet, style, force: force.unwrap_or(false), eye_method };
    blocking(move || e.scan_project(args)).await?
}

#[tauri::command]
fn scan_cancel(app: AppHandle) {
    engine(&app).scan_cancel();
}

/// Deciding is seconds on a very large project, so it runs off the main thread.
#[tauri::command]
async fn decide_photos(app: AppHandle, key: String, keep_ratio: f32, pick_mode: Option<String>, one_per_group: Option<bool>, overrides: HashMap<String, String>) -> Result<HashMap<String, decide::Decision>, String> {
    let e = engine(&app);
    blocking(move || e.decide_photos(&key, keep_ratio, pick_mode.as_deref(), one_per_group.unwrap_or(false), &overrides)).await?
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

// ---------------------------------------------------------------- 整理

#[tauri::command]
async fn export_photos(app: AppHandle, paths: Vec<String>, dest: String) -> Result<ExportResult, String> {
    let e = engine(&app);
    blocking(move || e.export_photos(&paths, &dest)).await?
}

/// Moves photos to the system trash (recoverable from Finder). Only image files are accepted, so this can never be used to trash anything else.
#[tauri::command]
async fn trash_photos(app: AppHandle, paths: Vec<String>) -> Result<TrashResult, String> {
    let e = engine(&app);
    blocking(move || e.trash_photos(paths)).await
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            // macOS 的平台能力：ImageIO 边车、Apple Vision 边车、Metal 上的 CLIP、访达的废纸篓
            imageio_macos::Helper::register(image_bin(&handle));
            fg_core::platform::set_vision(Box::new(vision::Factory { bin: vision_bin(&handle) }));
            fg_core::platform::set_gpu_clip(Box::new(clipgpu::Factory));
            fg_core::platform::set_local_trash(Box::new(mounts::DesktopTrash));
            let paths = engine::Paths { data: data_dir(&handle), thumbs: thumbs_dir(&handle), models: models_dir(&handle), downloads: downloaded_models_dir(&handle) };
            let version = handle.package_info().version.to_string();
            let e = Engine::new(paths, Box::new(WebviewEvents(handle.clone())), &version);
            // 启用的 NAS 在后台连上；缩略图旁边的草稿文件（大图、局部、动态照片的视频）修剪到上限以内
            e.start_background();
            app.manage(e);
            let menu = build_menu(&handle, "en")?;
            app.set_menu(menu)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            let _ = app.emit("menu", event.id().0.clone());
        })
        .invoke_handler(tauri::generate_handler![probe_folder, scan_project, scan_cancel, decide_photos, projects_load, projects_save, projects_quarantine, taste_export, taste_import, nas_list, nas_add, nas_remove, nas_connect, nas_disconnect, nas_ensure, nas_browse, nas_probe, nas_update, nas_set_enabled, backup_status, backup_save_target, backup_remove_target, backup_set_options, backup_now, backup_restore, set_locale, cache_size, clear_cache, record_label, prefs_info, prefs_reset, styles_list, style_create, style_rename, style_delete, style_set_default, learn_from_folder, export_photos, trash_photos, reveal, preview_thumbs, photo_info, large_preview, motion_clip, hdrview::hdr_overlay_supported, hdrview::hdr_overlay_show, hdrview::hdr_overlay_move, hdrview::hdr_overlay_hide, hdrview::hdr_overlay_detail, hdrview::hdr_overlay_detail_hide, photo_detail, machine_info, search_status, search_index, search_photos, scene_text, packs_status, pack_size, pack_download, pack_cancel, pack_remove])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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

