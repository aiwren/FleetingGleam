//! FleetingGleam 核心库给 Kotlin 的接口（UniFFI）。
//!
//! 和桌面端一样，全部应用流程在核心库的引擎里（`fg_core::engine`）。这里只做一件事：把引擎的方法按名字暴露出去——
//! `FgEngine.call("probe_folder", "{\"dirs\": [...]}")` 对应桌面端的 `invoke("probe_folder", { dirs })`，方法名、参数名、返回的 JSON
//! 都和 Tauri 命令一样（参数名用 snake_case）。引擎的进度事件经 `EventSink` 交给 Kotlin，事件名和载荷也和桌面端一样。
//! 这样 Kotlin 的界面层可以照着桌面端的 TypeScript 一行行对过去，功能不会漏。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine as _;
use fg_core::engine::{self, Engine, Paths, ScanArgs, SceneReq, TargetArgs};
use serde::Deserialize;
use serde_json::{json, Value};

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

/// 引擎的事件出口，Kotlin 侧实现：`payload` 是 JSON 文本。会从后台线程调用。
#[uniffi::export(foreign)]
pub trait EventSink: Send + Sync {
    fn on_event(&self, name: String, payload: String);
}

struct SinkEvents(Arc<dyn EventSink>);

/// Android 的图片解码器，Kotlin 侧用 BitmapFactory / BitmapRegionDecoder 实现：按需缩放解码、分块解码，两亿像素的照片也不用整张进内存；
/// 顺带认识 HEIC / AVIF。失败的调用返回错误文字，成功返回空串（UniFFI 的回调接口不方便抛错）。
#[uniffi::export(foreign)]
pub trait ImageHelper: Send + Sync {
    /// 认识的扩展名（小写、不带点）。
    fn supports(&self, ext: String) -> bool;
    /// 显示尺寸（方向已套用），"宽,高"；打不开给空串。
    fn info(&self, path: String) -> String;
    /// 一张摆正、长边不超过 `max` 的 JPEG 写到 `out`。
    fn render(&self, path: String, out: String, max: u32) -> String;
    /// 一块区域（摆正后的比例坐标，左上为原点），宽至多 `out_w`，存成 JPEG。
    fn crop(&self, path: String, out: String, x: f64, y: f64, w: f64, h: f64, out_w: u32) -> String;
    /// 直接解到内存：摆正、长边不超过 `max` 的像素，连同原图的显示尺寸。分析走这条路，不再先压一张 JPEG 写盘、再由核心库解回来。
    /// 解不了给 null，核心库退回 `render`。
    fn decode(&self, path: String, max: u32) -> Option<Pixels>;
}

/// `ImageHelper::decode` 的回答：一张解到内存的照片。
#[derive(uniffi::Record)]
pub struct Pixels {
    /// 这张像素的宽高（摆正后，长边不超过请求的 `max`）。
    pub width: u32,
    pub height: u32,
    /// 原图的显示尺寸（方向已套用）。
    pub full_width: u32,
    pub full_height: u32,
    /// `width * height * 4` 字节 RGBA：Android 的 ARGB_8888 位图在内存里就是这个顺序，Kotlin 侧原样拷出来、不逐像素转换；
    /// 透明度在这边丢掉（照片都不透明）。
    pub rgba: Vec<u8>,
}

/// RGBA 原地收成 RGB（不另分配）：第 i 个像素从 4i 挪到 3i，3i 总不在还没读的 4i 之后。
fn rgba_to_rgb(mut v: Vec<u8>, w: u32, h: u32) -> Option<image::RgbImage> {
    let n = w as usize * h as usize;
    if n == 0 || v.len() < n * 4 {
        return None;
    }
    for i in 0..n {
        let (s, d) = (i * 4, i * 3);
        v[d] = v[s];
        v[d + 1] = v[s + 1];
        v[d + 2] = v[s + 2];
    }
    v.truncate(n * 3);
    image::RgbImage::from_raw(w, h, v)
}

struct AndroidImages(Arc<dyn ImageHelper>);

impl fg_core::platform::SystemImages for AndroidImages {
    fn supports(&self, ext: &str) -> bool {
        self.0.supports(ext.to_string())
    }
    fn info(&self, path: &Path, _with_hdr: bool) -> Option<fg_core::platform::ImageInfo> {
        let s = self.0.info(path.to_string_lossy().into_owned());
        let (w, h) = s.split_once(',')?;
        Some(fg_core::platform::ImageInfo { w: w.trim().parse().ok()?, h: h.trim().parse().ok()?, hdr: false })
    }
    fn render(&self, src: &Path, out: &Path, max: u32, _sdr: bool) -> Result<(), String> {
        let e = self.0.render(src.to_string_lossy().into_owned(), out.to_string_lossy().into_owned(), max);
        if e.is_empty() { Ok(()) } else { Err(e) }
    }
    fn crop(&self, src: &Path, out: &Path, f: [f64; 4], out_w: u32) -> Result<(), String> {
        let e = self.0.crop(src.to_string_lossy().into_owned(), out.to_string_lossy().into_owned(), f[0], f[1], f[2], f[3], out_w);
        if e.is_empty() { Ok(()) } else { Err(e) }
    }
    fn prefers_native(&self) -> bool {
        true
    }
    /// 手机上 HEIC / AVIF 的大图也只要 2560：整张解出来几百 MB，查看器要原图时直接显示原文件（见 `large_preview` 的 `full`）。
    fn preview_max(&self) -> u32 {
        2560
    }
    fn decode(&self, src: &Path, max: u32) -> Option<(image::DynamicImage, u32, u32)> {
        let p = self.0.decode(src.to_string_lossy().into_owned(), max)?;
        let img = rgba_to_rgb(p.rgba, p.width, p.height)?;
        Some((image::DynamicImage::ImageRgb8(img), p.full_width, p.full_height))
    }
}

/// 注册 Android 的图片解码器；建引擎之前调一次。
#[uniffi::export]
pub fn set_image_helper(helper: Arc<dyn ImageHelper>) {
    fg_core::platform::set_images(Box::new(AndroidImages(helper)));
}

impl engine::Events for SinkEvents {
    fn emit(&self, event: &str, payload: Value) {
        self.0.on_event(event.to_string(), payload.to_string());
    }
}

/// 建引擎要的目录与环境（应用启动时由 Kotlin 给出）。
#[derive(Deserialize)]
struct Config {
    /// 应用私有的数据目录（`filesDir`）。
    data: String,
    /// 缩略图缓存（`cacheDir/thumbs`）。
    thumbs: String,
    /// 模型目录（`filesDir/models`）。
    models: String,
    /// 临时文件（`cacheDir/scratch`）：Android 上系统的临时目录应用写不了。
    scratch: String,
    /// 系统的 HTTP 代理（`http://主机:端口`），没有就不写。
    #[serde(default)]
    proxy: Option<String>,
    /// 应用版本，写进"我的设置与喜好"。
    #[serde(default)]
    version: String,
}

/// 核心库的版本（与应用版本一致）。
#[uniffi::export]
pub fn core_version() -> String {
    fg_core::VERSION.to_string()
}

#[derive(uniffi::Object)]
pub struct FgEngine {
    engine: Arc<Engine>,
}

#[uniffi::export]
impl FgEngine {
    /// 建引擎：注册 Android 的平台能力（人脸流水线作为系统视觉信号；没有系统图像解码器、没有 GPU 编码器、本机回收交给界面），
    /// 启动后台的 NAS 连接与草稿修剪。整个进程只建一个。
    #[uniffi::constructor]
    pub fn new(config: String, sink: Arc<dyn EventSink>) -> Result<Arc<Self>, FgError> {
        let c: Config = serde_json::from_str(&config).map_err(|e| format!("config: {e}"))?;
        fg_core::platform::set_scratch_dir(PathBuf::from(&c.scratch));
        fg_core::platform::set_proxy(c.proxy);
        fg_core::platform::set_vision(Box::new(fg_core::facepipe::Factory));
        let paths = Paths { data: c.data.into(), thumbs: c.thumbs.into(), models: PathBuf::from(&c.models), downloads: PathBuf::from(&c.models) };
        let version = if c.version.is_empty() { fg_core::VERSION.to_string() } else { c.version };
        let engine = Engine::new(paths, Box::new(SinkEvents(sink)), &version);
        engine.start_background();
        Ok(Arc::new(FgEngine { engine }))
    }

    /// 调一个引擎方法：`method` 是桌面端的命令名，`args` 是参数的 JSON 对象（snake_case），返回结果的 JSON。阻塞，Kotlin 放到 IO 线程上调。
    pub fn call(&self, method: String, args: String) -> Result<String, FgError> {
        let a: Value = if args.trim().is_empty() { json!({}) } else { serde_json::from_str(&args).map_err(|e| format!("{method}: bad arguments: {e}"))? };
        dispatch(&self.engine, &method, &a).map(|v| v.to_string()).map_err(FgError::from)
    }

    /// 改系统代理（用户在系统设置里换了代理，或换了网络）。
    pub fn set_proxy(&self, proxy: Option<String>) {
        fg_core::platform::set_proxy(proxy);
    }
}

// ---------------------------------------------------------------- 参数

fn s(a: &Value, k: &str) -> Result<String, String> {
    a.get(k).and_then(Value::as_str).map(str::to_string).ok_or_else(|| format!("missing argument `{k}`"))
}

fn os(a: &Value, k: &str) -> Option<String> {
    a.get(k).and_then(Value::as_str).map(str::to_string)
}

fn b(a: &Value, k: &str) -> bool {
    a.get(k).and_then(Value::as_bool).unwrap_or(false)
}

fn ob(a: &Value, k: &str) -> Option<bool> {
    a.get(k).and_then(Value::as_bool)
}

fn f(a: &Value, k: &str) -> f64 {
    a.get(k).and_then(Value::as_f64).unwrap_or(0.0)
}

fn strings(a: &Value, k: &str) -> Vec<String> {
    a.get(k).and_then(Value::as_array).map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

fn parse<T: serde::de::DeserializeOwned>(a: &Value, k: &str) -> Result<T, String> {
    serde_json::from_value(a.get(k).cloned().unwrap_or(Value::Null)).map_err(|e| format!("argument `{k}`: {e}"))
}

fn out<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

/// 方法名 → 引擎方法。名字和参数与桌面端的 Tauri 命令一一对应（见 apps/desktop/src-tauri/src/lib.rs），另加几个 Android 用的：
/// `taste_export_bytes` / `taste_import_bytes`（系统的文件选择器只给字节，没有路径）、`packs_status` 可以带上要下载的字节数。
fn dispatch(e: &Arc<Engine>, method: &str, a: &Value) -> Result<Value, String> {
    let dirs = || strings(a, "dirs");
    match method {
        // 探测、分析、取舍
        "probe_folder" => out(e.probe_folder(&dirs(), os(a, "compute").as_deref(), os(a, "quality").as_deref())?),
        "scan_project" => out(e.scan_project(ScanArgs {
            key: s(a, "key")?,
            dirs: dirs(),
            compute: os(a, "compute"),
            quality: os(a, "quality"),
            learn: b(a, "learn"),
            quiet: b(a, "quiet"),
            style: os(a, "style"),
            force: b(a, "force"),
            eye_method: os(a, "eye_method"),
            // 边分析边判：有 "live" 就把它整个交给引擎
            live: a.get("live").filter(|v| !v.is_null()).cloned().map(serde_json::from_value).transpose().map_err(|e| e.to_string())?,
        })?),
        // 带 key 只停那个项目的分析（手机上后台分析的项目不受打开别的项目影响）；不带就全停
        "scan_cancel" => {
            match os(a, "key") {
                Some(k) => e.scan_cancel_key(&k),
                None => e.scan_cancel(),
            }
            Ok(Value::Null)
        }
        "set_rotation" => out(e.set_rotation(&s(a, "path")?, a.get("deg").and_then(Value::as_i64).unwrap_or(0))?),
        "set_device" => {
            e.set_device(&s(a, "name")?);
            Ok(Value::Null)
        }
        "decide_photos" => {
            let overrides: HashMap<String, String> = parse(a, "overrides").unwrap_or_default();
            out(e.decide_photos(&s(a, "key")?, f(a, "keep_ratio") as f32, os(a, "pick_mode").as_deref(), b(a, "one_per_group"), &overrides)?)
        }
        // 项目文件
        "projects_load" => out(e.projects_load()?),
        "projects_save" => out(e.projects_save(&s(a, "json")?)?),
        "projects_quarantine" => out(e.projects_quarantine()?),
        // 我的设置与喜好
        "taste_export" => out(e.taste_export(&s(a, "path")?, a.get("settings").cloned().unwrap_or(Value::Null))?),
        "taste_import" => out(e.taste_import(&s(a, "path")?)?),
        "taste_export_bytes" => {
            let (bytes, summary) = e.taste_pack_bytes(a.get("settings").cloned().unwrap_or(Value::Null))?;
            Ok(json!({ "base64": base64::engine::general_purpose::STANDARD.encode(bytes), "summary": summary }))
        }
        "taste_import_bytes" => {
            let bytes = base64::engine::general_purpose::STANDARD.decode(s(a, "base64")?).map_err(|e| e.to_string())?;
            out(e.taste_import_bytes(&bytes)?)
        }
        // 云备份
        "backup_status" => out(e.backup_status()),
        "backup_save_target" => out(e.backup_save_target(TargetArgs {
            id: os(a, "id"),
            name: os(a, "name").unwrap_or_default(),
            kind: os(a, "kind").unwrap_or_else(|| "r2".into()),
            account: s(a, "account")?,
            bucket: s(a, "bucket")?,
            access_key: s(a, "access_key")?,
            secret: os(a, "secret"),
        })?),
        "backup_remove_target" => out(e.backup_remove_target(&s(a, "id")?)?),
        "backup_set_options" => out(e.backup_set_options(ob(a, "enabled"), a.get("keep").and_then(Value::as_u64).map(|n| n as usize))?),
        "backup_now" => out(e.backup_now(a.get("settings").cloned().unwrap_or(Value::Null))?),
        "backup_restore" => out(e.backup_restore(&s(a, "id")?)?),
        "backup_peek" => out(e.backup_peek(&s(a, "id")?)?),
        // NAS
        "nas_list" => out(e.nas_list()),
        "nas_probe" => out(e.nas_probe(&s(a, "host")?, &os(a, "user").unwrap_or_default(), &os(a, "password").unwrap_or_default(), os(a, "id").as_deref())?),
        "nas_add" => out(e.nas_add(&os(a, "name").unwrap_or_default(), &s(a, "host")?, &os(a, "user").unwrap_or_default(), &os(a, "password").unwrap_or_default(), b(a, "remember"))?),
        "nas_update" => out(e.nas_update(&s(a, "id")?, &os(a, "name").unwrap_or_default(), &s(a, "host")?, &os(a, "user").unwrap_or_default(), &os(a, "password").unwrap_or_default(), b(a, "remember"))?),
        "nas_remove" => out(e.nas_remove(&s(a, "id")?)?),
        "nas_connect" => out(e.nas_connect(&s(a, "id")?, os(a, "password").as_deref())?),
        "nas_disconnect" => out(e.nas_disconnect(&s(a, "id")?)),
        "nas_set_enabled" => out(e.nas_set_enabled(&s(a, "id")?, b(a, "enabled"))?),
        "nas_ensure" => out(e.nas_ensure(&dirs())),
        "nas_browse" => out(e.nas_browse(&s(a, "id")?, &os(a, "path").unwrap_or_default())?),
        "nas_mirror_usage" => out(e.nas_mirror_usage()),
        "nas_mirror_clear" => out(e.nas_mirror_clear(&s(a, "id")?)?),
        // 整理
        "export_photos" => out(e.export_photos(&strings(a, "paths"), &s(a, "dest")?)?),
        "export_cancel" => {
            e.export_cancel();
            Ok(Value::Null)
        }
        "trash_photos" => out(e.trash_photos(strings(a, "paths"))),
        // 查看器
        "preview_thumbs" => out(e.preview_thumbs(&strings(a, "paths"))),
        "photo_info" => out(e.photo_info(&s(a, "path")?)),
        "large_preview" => out(e.large_preview(&s(a, "path")?, b(a, "full"), b(a, "showable"))?),
        "photo_detail" => out(e.photo_detail(&s(a, "path")?, f(a, "x"), f(a, "y"), f(a, "w"), f(a, "h"), f(a, "out_w") as u32)?),
        "motion_clip" => out(e.motion_clip(&s(a, "path")?)?),
        // 搜索与场景
        "search_status" => out(e.search_status(&dirs())),
        "search_index" => out(e.search_index(&dirs())?),
        "search_photos" => out(e.search_photos(&dirs(), &s(a, "query")?)?),
        "scene_text" => {
            let scenes: Vec<SceneReq> = parse(a, "scenes")?;
            out(e.scene_text(&scenes, b(a, "zh"), b(a, "caption"))?)
        }
        // 模型包与机器
        "machine_info" => out(e.machine_info()),
        "accel_probe" => out(e.accel_probe(&s(a, "file")?)?),
        "forget_folders" => out(e.forget_folders(&strings(a, "dirs"), &strings(a, "keep"))),
        "packs_status" => out(e.packs_status(b(a, "with_sizes"))),
        "pack_size" => out(e.pack_size(&s(a, "id")?)?),
        "pack_download" => out(e.pack_download(&s(a, "id")?)?),
        "pack_cancel" => {
            e.pack_cancel();
            Ok(Value::Null)
        }
        "pack_remove" => out(e.pack_remove(&s(a, "id")?)?),
        // 喜好与风格
        "record_label" => out(e.record_label(&s(a, "path")?, ob(a, "label"), os(a, "style").as_deref())),
        "prefs_info" => out(e.prefs_info(os(a, "style").as_deref())),
        "prefs_reset" => out(e.prefs_reset(os(a, "style").as_deref())),
        "styles_list" => out(e.styles_list()),
        "style_create" => out(e.style_create(&s(a, "name")?)),
        "style_rename" => out(e.style_rename(&s(a, "id")?, &s(a, "name")?)),
        "style_delete" => out(e.style_delete(&s(a, "id")?)),
        "style_set_default" => out(e.style_set_default(&s(a, "id")?)),
        "learn_from_folder" => out(e.learn_from_folder(&s(a, "dir")?, os(a, "style").as_deref(), os(a, "compute").as_deref(), os(a, "quality").as_deref())?),
        // 缓存
        "cache_size" => out(e.cache_size()),
        "clear_cache" => out(e.clear_cache()?),
        // 诊断
        "diagnostics_export" => out(e.diagnostics_export(&s(a, "out")?)?),
        "diagnostics_dir" => out(e.diagnostics_dir()),
        other => Err(format!("unknown method `{other}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Collect(Mutex<Vec<String>>);
    impl EventSink for Collect {
        fn on_event(&self, name: String, _payload: String) {
            self.0.lock().unwrap().push(name);
        }
    }

    /// 没有任何模型、没有任何平台信号时，一个文件夹也能经 `call` 探测、分析、取舍：这正是 Android 端第一次跑起来时的状态。
    #[test]
    fn the_engine_is_reached_by_method_name() {
        let dir = std::env::temp_dir().join(format!("fg-android-smoke-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("photos")).unwrap();
        for (i, blur) in [0.0f32, 0.0, 6.0].iter().enumerate() {
            let img = image::RgbImage::from_fn(640, 480, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x * y) % 256) as u8]));
            let img = if *blur > 0.0 { image::imageops::blur(&img, *blur) } else { img };
            img.save(dir.join("photos").join(format!("p{i}.jpg"))).unwrap();
        }
        let p = |x: &str| dir.join(x).to_string_lossy().into_owned();
        let config = json!({ "data": p("data"), "thumbs": p("thumbs"), "models": p("models"), "scratch": p("scratch"), "version": "test" }).to_string();
        let sink = Arc::new(Collect(Mutex::new(Vec::new())));
        let e = FgEngine::new(config, sink.clone()).unwrap();
        let dirs = json!({ "dirs": [p("photos")] }).to_string();
        let probe: Value = serde_json::from_str(&e.call("probe_folder".into(), dirs.clone()).unwrap()).unwrap();
        assert_eq!(probe["uncached"], 3);
        let photos: Value = serde_json::from_str(&e.call("scan_project".into(), json!({ "key": "k", "dirs": [p("photos")], "eye_method": "mediapipe", "live": { "keep_ratio": 1.0 } }).to_string()).unwrap()).unwrap();
        assert_eq!(photos.as_array().unwrap().len(), 3);
        assert!(sink.0.lock().unwrap().iter().any(|n| n == "scan-photo"));
        // 边分析边判：分析中就发过判定
        assert!(sink.0.lock().unwrap().iter().any(|n| n == "scan-decide"));
        let d: Value = serde_json::from_str(&e.call("decide_photos".into(), json!({ "key": "k", "keep_ratio": 1.0 }).to_string()).unwrap()).unwrap();
        assert_eq!(d.as_object().unwrap().len(), 3);
        // 本机照片的回收交给界面
        let photo = p("photos/p0.jpg");
        let t: Value = serde_json::from_str(&e.call("trash_photos".into(), json!({ "paths": [photo] }).to_string()).unwrap()).unwrap();
        assert_eq!(t["deferred"].as_array().unwrap().len(), 1);
        // 我的设置与喜好：字节往返
        let x: Value = serde_json::from_str(&e.call("taste_export_bytes".into(), json!({ "settings": { "theme": "dark" } }).to_string()).unwrap()).unwrap();
        let back: Value = serde_json::from_str(&e.call("taste_import_bytes".into(), json!({ "base64": x["base64"] }).to_string()).unwrap()).unwrap();
        assert_eq!(back[1]["theme"], "dark");
        // 模型包：随应用的（眼睛、人脸检测）已经在；其余一个都没装，每个都说要下载多少
        let packs: Value = serde_json::from_str(&e.call("packs_status".into(), json!({ "with_sizes": true }).to_string()).unwrap()).unwrap();
        for p in packs.as_array().unwrap() {
            if p["bundled"] == true {
                assert_eq!(p["installed"], true, "{p}");
            } else {
                assert!(p["installed"] == false && p["bytes"].as_u64().unwrap() > 0, "{p}");
            }
        }
        assert!(packs.as_array().unwrap().iter().filter(|p| p["bundled"] == true).count() == 2);
        assert!(e.call("nope".into(), "{}".into()).is_err());
        assert!(e.call("probe_folder".into(), "not json".into()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Kotlin 交过来的 RGBA 原地收成 RGB：每个像素的颜色不变，透明度丢掉；字节不够的不收。
    #[test]
    fn rgba_pixels_become_rgb() {
        let (w, h) = (5u32, 3u32);
        let rgba: Vec<u8> = (0..w * h).flat_map(|i| [i as u8, (i * 7) as u8, (i * 13) as u8, 200]).collect();
        let rgb = rgba_to_rgb(rgba.clone(), w, h).unwrap();
        for (i, p) in rgb.pixels().enumerate() {
            assert_eq!(p.0, [rgba[i * 4], rgba[i * 4 + 1], rgba[i * 4 + 2]]);
        }
        assert!(rgba_to_rgb(rgba[..10].to_vec(), w, h).is_none());
        assert!(rgba_to_rgb(Vec::new(), 0, 0).is_none());
    }

    /// 手机上量第 8 条的核心库这一半（Kotlin 那一半用 app_process 跑的小程序量）：老路每张要把 Kotlin 写的 1024px JPEG 解回来再删掉，
    /// 新路只把交过来的 RGBA 收成 RGB。`FG_JPEGS=<一批 1024px 的 JPEG>`；文件先拷一份再量，原文件不动。
    #[test]
    #[ignore = "timing only, on a device: FG_JPEGS"]
    fn decode_handoff_cost() {
        let Ok(dir) = std::env::var("FG_JPEGS") else { return };
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().map_or(false, |e| e == "jpg")).collect();
        files.sort();
        let (mut old, mut new, mut n) = (0f64, 0f64, 0usize);
        for f in &files {
            let copy = f.with_extension("copy");
            std::fs::copy(f, &copy).unwrap();
            let t = std::time::Instant::now();
            let img = image::open(&copy).unwrap();
            let _ = std::fs::remove_file(&copy);
            old += t.elapsed().as_secs_f64() * 1000.0;
            let rgba = img.to_rgba8().into_raw();
            let t = std::time::Instant::now();
            let rgb = rgba_to_rgb(rgba, img.width(), img.height()).unwrap();
            new += t.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(rgb.as_raw(), img.to_rgb8().as_raw());
            n += 1;
        }
        println!("{n} photos: old (open the JPEG + delete) {:.1} ms/photo, new (RGBA to RGB) {:.1} ms/photo", old / n as f64, new / n as f64);
    }

    /// 两端一致性的固定用例（路线图阶段 B 第 6 步）：和核心库的同名测试比同一份基准。在 Mac 上 `cargo test -p fleetinggleam-android` 跑一次，
    /// 在 Android（arm64）上由 apps/android/scripts/consistency.sh 编成测试程序推到手机 / 模拟器上跑——两边都通过，才说明分组与取舍两端一致。
    #[test]
    fn consistency_matches_the_golden() {
        let work = std::env::temp_dir().join(format!("fg-consistency-android-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        let r = fg_core::consistency::check(&work, &work.join("no-models"), fg_core::consistency::GOLDEN, None);
        let _ = std::fs::remove_dir_all(&work);
        if let Err(e) = r { panic!("没有模型时和基准不一致：\n{e}"); }
    }

    /// 装了标准档模型时（`FG_MODELS` 指向模型文件夹；没设就跳过）。
    #[test]
    fn consistency_with_models_matches_the_golden() {
        let Ok(models) = std::env::var("FG_MODELS") else { return };
        let work = std::env::temp_dir().join(format!("fg-consistency-android-m-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        let r = fg_core::consistency::check(&work, std::path::Path::new(&models), fg_core::consistency::GOLDEN_MODELS, None);
        let _ = std::fs::remove_dir_all(&work);
        if let Err(e) = r { panic!("装了标准档模型时和基准不一致：\n{e}"); }
    }

    /// 我的设置与喜好两端互导：Mac 的界面（Tauri）用 `taste_export` 写文件、Android 用 `taste_export_bytes` 给字节交给系统存；
    /// 两种都是同一份 `fleetinggleam-taste/1`。这里一个引擎扮 Mac（写文件、带 Mac 的设置字段），另一个扮 Android（按字节读），再反过来。
    #[test]
    fn taste_files_go_both_ways_between_mac_and_android() {
        let root = std::env::temp_dir().join(format!("fg-taste-2way-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let engine = |side: &str| {
            let d = root.join(side);
            let p = |x: &str| d.join(x).to_string_lossy().into_owned();
            let config = json!({ "data": p("data"), "thumbs": p("thumbs"), "models": p("models"), "scratch": p("scratch"), "version": side }).to_string();
            FgEngine::new(config, Arc::new(Collect(Mutex::new(Vec::new())))).unwrap()
        };
        let (mac, android) = (engine("mac"), engine("android"));
        // Mac：一个叫"街拍"的风格，桌面端前端存的设置（camelCase、眼睛用 Vision）
        mac.call("style_create".into(), json!({ "name": "街拍" }).to_string()).unwrap();
        let mac_settings = json!({ "compute": "auto", "theme": "dark", "learn": false, "quality": "high", "search": true, "customStyles": true, "eyeMethod": "vision", "tier": "high", "tierRev": 2 });
        let file = root.join("fleetinggleam-taste.json").to_string_lossy().into_owned();
        mac.call("taste_export".into(), json!({ "path": file, "settings": mac_settings }).to_string()).unwrap();
        // Android：系统选择器给的是字节
        let bytes = std::fs::read(&file).unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let back: Value = serde_json::from_str(&android.call("taste_import_bytes".into(), json!({ "base64": b64 }).to_string()).unwrap()).unwrap();
        assert_eq!(back[1], mac_settings, "设置原样带过来（哪些采用由界面决定：Android 只取各平台通用的几项，眼睛固定 MediaPipe）");
        let styles: Value = serde_json::from_str(&android.call("styles_list".into(), "{}".into()).unwrap()).unwrap();
        assert!(styles.as_array().unwrap().iter().any(|s| s["name"] == "街拍"), "风格带过来了：{styles}");
        // 反过来：Android 导出（字节）→ Mac 从文件导入
        let android_settings = json!({ "theme": "light", "learn": true, "eyeMethod": "mediapipe", "tier": "fast" });
        let x: Value = serde_json::from_str(&android.call("taste_export_bytes".into(), json!({ "settings": android_settings }).to_string()).unwrap()).unwrap();
        let file2 = root.join("from-android.json");
        std::fs::write(&file2, base64::engine::general_purpose::STANDARD.decode(x["base64"].as_str().unwrap()).unwrap()).unwrap();
        let back2: Value = serde_json::from_str(&mac.call("taste_import".into(), json!({ "path": file2.to_string_lossy() }).to_string()).unwrap()).unwrap();
        assert_eq!(back2[1], android_settings);
        let raw: Value = serde_json::from_slice(&std::fs::read(&file2).unwrap()).unwrap();
        assert_eq!(raw["format"], "fleetinggleam-taste/1");
        let _ = std::fs::remove_dir_all(&root);
    }
}
