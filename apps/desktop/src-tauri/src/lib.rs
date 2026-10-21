mod analyze;
mod cache;
mod clipgpu;
mod decide;
mod hdrview;
mod imageio;
mod media;
mod scratch;
mod scene_vocab;
mod florence;
mod translate;
mod faces;
mod prefs;
pub mod modelfiles;
mod scorer;
mod colorsig;
mod mediapipe;
mod modelpacks;
mod textsearch;
mod scene;
mod scoring;
mod vision;
mod taste;
mod nas;
mod r2;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

use analyze::Photo;
use rayon::prelude::*;
use serde::Serialize;
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager};
use walkdir::WalkDir;

#[derive(Serialize, Clone)]
struct Progress {
    done: usize,
    total: usize,
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

#[derive(Serialize, Clone)]
struct Info {
    cpu_only: bool,
    clip: Option<&'static str>,
    model: Option<&'static str>,
    vision: bool,
    cores: usize,
}


#[derive(Serialize)]
struct ProbeFile {
    path: String,
    /// Displayed size (EXIF orientation applied), read from the file header without decoding pixels.
    width: u32,
    height: u32,
    /// True when a valid analysis for this exact file is already cached.
    cached: bool,
}

#[derive(Serialize)]
struct Probe {
    files: Vec<ProbeFile>,
    bytes: u64,
    uncached: usize,
}

/// Shared, lazily loaded state: the analysis cache and the on-device taste model.
struct AppState {
    text: Mutex<Option<std::sync::Arc<textsearch::TextEncoder>>>,
    cache: Mutex<Option<cache::Cache>>,
    prefs: Mutex<prefs::Styles>,
    /// Loaded once, on first use: the eye-openness models are a few MB each, not worth reopening per scan.
    mediapipe_eyes: Mutex<Option<std::sync::Arc<mediapipe::MediapipeEyes>>>,
    /// Loaded once, on first use: the face-recognition network is tens of MB.
    face_embedder: Mutex<Option<std::sync::Arc<faces::FaceEmbedder>>>,
    /// The last project's photos (with the project key they belong to), kept so `decide_photos` can re-run keep/review/reject
    /// on a plain override change without the whole (possibly large) list crossing the IPC boundary again. The key stops a
    /// scan that finished late (of a project since left) from answering for the project now open.
    last_photos: Mutex<(String, Vec<Photo>)>,
    /// Bumped by every scan and by `scan_cancel`, so a scan that is no longer the current one stops at the next photo.
    scan_job: AtomicU64,
    /// Bumped by every recorded label; the retraining scheduled by a label only runs if no newer label came in the meantime.
    retrain_gen: AtomicU64,
    /// Loaded once, on first use: the scene-description model (a few hundred MB) and the English-to-Chinese one.
    captioner: Mutex<Option<std::sync::Arc<florence::Captioner>>>,
    translator: Mutex<Option<std::sync::Arc<translate::Translator>>>,
    /// The English sentence for each photo already described (keyed by its thumbnail), kept in memory and on disk.
    scene_sentences: Mutex<Option<std::collections::HashMap<String, String>>>,
    /// Bumped by every request to describe scenes, so an older one stops as soon as a newer one arrives.
    scene_job: AtomicU64,
    /// NAS 共享的连接，住在自己的线程里（见 nas.rs）。
    nas: nas::Pool,
}

fn data_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("fleetinggleam"))
}

fn with_cache<R>(app: &AppHandle, f: impl FnOnce(&mut cache::Cache) -> R) -> R {
    let st = app.state::<AppState>();
    let mut guard = st.cache.lock().unwrap();
    let c = guard.get_or_insert_with(|| cache::Cache::load(data_dir(app).join("analysis-cache.json")));
    f(c)
}

/// The MediaPipe eye models, loaded once and kept: `None` only when the pack has not been downloaded, or failed to load.
fn mediapipe_eyes(app: &AppHandle, model_dir: &Path) -> Option<std::sync::Arc<mediapipe::MediapipeEyes>> {
    let st = app.state::<AppState>();
    let mut guard = st.mediapipe_eyes.lock().unwrap();
    if guard.is_none() && mediapipe::available(model_dir) {
        match mediapipe::MediapipeEyes::load(model_dir) {
            Ok(m) => *guard = Some(std::sync::Arc::new(m)),
            Err(e) => eprintln!("MediaPipe eyes disabled: {e}"),
        }
    }
    guard.clone()
}

/// The face-recognition network, loaded once and kept: `None` when it has not been downloaded or failed to load.
fn face_embedder(app: &AppHandle, model_dir: &Path) -> Option<std::sync::Arc<faces::FaceEmbedder>> {
    let st = app.state::<AppState>();
    let mut guard = st.face_embedder.lock().unwrap();
    if guard.is_none() && faces::available(model_dir) {
        match faces::FaceEmbedder::load(model_dir) {
            Ok(m) => *guard = Some(std::sync::Arc::new(m)),
            Err(e) => eprintln!("face recognition disabled: {e}"),
        }
    }
    guard.clone()
}

/// Names that are never the user's photos: hidden files (this covers macOS `._IMG.jpg` resource forks and `.DS_Store`),
/// and folders the system or a NAS keeps beside real content.
pub(crate) fn is_junk(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "__MACOSX" | "@eaDir" | "$RECYCLE.BIN" | "System Volume Information" | "Thumbs.db" | "desktop.ini")
}

/// Photos below this size are icons or thumbnails, not photos.
pub(crate) const MIN_PHOTO_BYTES: u64 = 8 * 1024;

fn list_images(dir: &str) -> Vec<PathBuf> {
    let mut files: Vec<_> = WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !is_junk(&e.file_name().to_string_lossy()))
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && analyze::is_image(e.path()) && e.metadata().map_or(false, |m| m.len() >= MIN_PHOTO_BYTES))
        .map(|e| e.into_path())
        .collect();
    files.sort();
    files
}

/// Every photo in the folders, without duplicates. A file whose header cannot be read (corrupt, or not really an image
/// despite its extension) is left out here, so the count the user sees, the wall and the analysis all agree.
fn list_all(dirs: &[String]) -> Vec<PathBuf> {
    list_all_with(dirs, |_| false)
}

/// 同 `list_all`，但 `known(path)` 为真的文件（分析缓存里已经有、文件没变过）不再读文件头验证：打开一个分析过的项目时，
/// 几百个 20 MB 的 JPEG 头一张张读要一两秒，这是"正在读取文件夹"卡住的主要原因。
fn list_all_with(dirs: &[String], known: impl Fn(&Path) -> bool + Sync) -> Vec<PathBuf> {
    let mut all: Vec<PathBuf> = dirs.iter().flat_map(|d| list_images(d)).collect();
    all.sort();
    all.dedup();
    all.into_par_iter().filter(|p| known(p) || oriented_dims(p).is_some()).collect()
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

fn oriented_dims(p: &Path) -> Option<(u32, u32)> {
    use image::metadata::Orientation as O;
    if !imageio::is_native(p) {
        return imageio::info(p, false).map(|i| (i.w, i.h));
    }
    use image::ImageDecoder;
    let reader = image::ImageReader::open(p).ok()?.with_guessed_format().ok()?;
    let mut dec = reader.into_decoder().ok()?;
    let (w, h) = dec.dimensions();
    Some(match dec.orientation().unwrap_or(O::NoTransforms) {
        O::Rotate90 | O::Rotate270 | O::Rotate90FlipH | O::Rotate270FlipH => (h, w),
        _ => (w, h),
    })
}

/// Cheap pre-flight for a set of folders: every photo's true displayed size (so the wall can be laid out
/// once, at final size, before any analysis) and whether it is already cached.
#[tauri::command]
async fn probe_folder(app: AppHandle, dirs: Vec<String>, compute: Option<String>, quality: Option<String>) -> Result<Probe, String> {
    let tag = scorer::analysis_tag(scorer::resolve(&models_dir(&app), compute.as_deref() != Some("cpu"), scorer::Quality::parse(quality.as_deref())));
    // 离开项目时页面会 `scan_cancel`，正在从 NAS 拉照片的探测也就跟着停
    let job = app.state::<AppState>().scan_job.load(Ordering::Relaxed);
    tauri::async_runtime::spawn_blocking(move || {
        nas_sync_dirs(&app, &dirs, job)?;
        // 缓存里认识的文件（没变过）不读文件头：尺寸直接查表，是否"已分析"再按当前模型的 tag 看
        let (files, known): (Vec<PathBuf>, Vec<(Option<(u32, u32)>, bool)>) = with_cache(&app, |c| {
            let files = list_all_with(&dirs, |p| cache::file_key(p).map_or(false, |(sz, mt)| c.dims(&p.to_string_lossy(), sz, mt).is_some()));
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
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Analyses whatever is new or changed, reuses the cache for the rest, then groups and scores every photo
/// together. `key` names the project (so `decide_photos` can tell whose photos it holds); `learn` blends in the on-device
/// taste model; `quiet` suppresses progress events (used for instant re-opens and re-scoring); `force` ignores the cache
/// and re-analyses every photo. A newer scan, or `scan_cancel`, stops this one at the next photo with `Err("cancelled")`;
/// what it analysed by then is already in the cache.
#[tauri::command]
async fn scan_project(app: AppHandle, key: String, dirs: Vec<String>, compute: Option<String>, quality: Option<String>, learn: bool, quiet: bool, style: Option<String>, force: Option<bool>, eye_method: Option<String>) -> Result<Vec<Photo>, String> {
    let force = force.unwrap_or(false);
    let job = app.state::<AppState>().scan_job.fetch_add(1, Ordering::Relaxed) + 1;
    let cpu_only = compute.as_deref() == Some("cpu");
    let want = scorer::Quality::parse(quality.as_deref());
    let eye_method = analyze::EyeMethod::parse(eye_method.as_deref());
    let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("thumbs");
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    let model_dir = models_dir(&app);
    let vision_bin = vision_bin(&app);
    let tag = scorer::analysis_tag(scorer::resolve(&model_dir, !cpu_only, want));
    let mp = if eye_method == analyze::EyeMethod::Mediapipe { mediapipe_eyes(&app, &model_dir) } else { None };
    let fe = face_embedder(&app, &model_dir);

    tauri::async_runtime::spawn_blocking(move || {
        let cancelled = || app.state::<AppState>().scan_job.load(Ordering::Relaxed) != job;
        // 镜像里可能有探测之后才到 NAS 上的新照片；再对一次远端，没变化时只是几次目录列表
        nas_sync_dirs(&app, &dirs, job)?;
        let mut hits: Vec<Photo> = Vec::new();
        let mut jobs: Vec<(PathBuf, u64, i64)> = Vec::new();
        let files = with_cache(&app, |c| {
            let files = list_all_with(&dirs, |p| cache::file_key(p).map_or(false, |(sz, mt)| c.dims(&p.to_string_lossy(), sz, mt).is_some()));
            for f in &files {
                let key = f.to_string_lossy();
                match cache::file_key(f) {
                    Some((sz, mt)) => match if force { None } else { c.get(&key, sz, mt, &tag) } {
                        Some(p) => hits.push(p),
                        None => jobs.push((f.clone(), sz, mt)),
                    },
                    None => {}
                }
            }
            files
        });
        let total = files.len();

        let done = AtomicUsize::new(hits.len());
        if !quiet {
            let _ = app.emit("scan-progress", Progress { done: hits.len(), total });
            // the photos that are already known go over in batches: one event each would flood the page on a big project
            for chunk in hits.chunks(250) {
                let _ = app.emit("scan-photos", chunk);
            }
        }

        let mut fresh: Vec<Photo> = Vec::new();
        if !jobs.is_empty() {
            // Models are only loaded when there is real work: re-opening a cached project is instant.
            let scorer = scorer::ClipScorer::load(&model_dir, !cpu_only, want)
                .map_err(|e| eprintln!("AI disabled: {e}"))
                .ok();
            let vision = vision::Vision::start(&vision_bin, 2, cpu_only);
            if !quiet {
                let _ = app.emit(
                    "scan-info",
                    Info {
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
                with_cache(&app, |c| {
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
                        let _ = app.emit(
                            "scan-stage",
                            Stage { path: &key, stage, pct, thumb: thumb.map(|t| t.0), width: thumb.map(|t| t.1), height: thumb.map(|t| t.2) },
                        );
                    };
                    let r = analyze::analyze(p, &cache_dir, scorer.as_ref().map(|s| s as &dyn scorer::AiScorer), vision.as_ref(), eye_method, mp.as_deref(), fe.as_deref(), &on_stage);
                    let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if !quiet {
                        if let Ok(photo) = &r {
                            let _ = app.emit("scan-photo", photo);
                        }
                        let _ = app.emit("scan-progress", Progress { done: d, total });
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
            if let Some(v) = vision::Vision::start(&vision_bin, 2, cpu_only) {
                let measured: Vec<(usize, (f32, &'static str))> = hits
                    .par_iter()
                    .enumerate()
                    .filter(|(_, p)| p.face_count > 0 && !analyze::eyes_current(p.eye_method.as_deref(), eye_method) && !cancelled())
                    .filter_map(|(i, p)| analyze::measure_eyes(Path::new(&p.path), &cache_dir, &v, eye_method, mp.as_deref()).map(|e| (i, e)))
                    .collect();
                if !measured.is_empty() {
                    for (i, (e, tag)) in &measured {
                        // no face big enough to measure is remembered as "very open", so it is not asked again
                        hits[*i].eyes_open = Some(*e);
                        hits[*i].eye_method = Some(tag.to_string());
                    }
                    with_cache(&app, |c| {
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
                if let Some(v) = vision::Vision::start(&vision_bin, 2, cpu_only) {
                    let counter = AtomicUsize::new(0);
                    let found: Vec<(usize, Vec<[f32; 4]>, Vec<Vec<f32>>)> = pool.install(|| {
                        hits.par_iter()
                            .enumerate()
                            .filter(|(_, p)| !p.faces_done && !cancelled())
                            .filter_map(|(i, p)| {
                                let r = analyze::measure_faces(Path::new(&p.path), &cache_dir, &v, f).map(|(b, e)| (i, b, e));
                                let _ = app.emit("people-progress", Progress { done: counter.fetch_add(1, Ordering::Relaxed) + 1, total });
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
                        with_cache(&app, |c| {
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
                            let _ = app.emit("media-progress", Progress { done: counter.fetch_add(1, Ordering::Relaxed) + 1, total });
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
                with_cache(&app, |c| {
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
        let prefs = if learn { Some(app.state::<AppState>().prefs.lock().unwrap().get(style.as_deref()).prefs.clone()) } else { None };
        analyze::group_and_score(&mut photos, prefs.as_ref().filter(|p| p.active()));
        {
            // grouping took a while; a newer scan may have written its project meanwhile, and this one must not replace it
            let st = app.state::<AppState>();
            let mut last = st.last_photos.lock().unwrap();
            if st.scan_job.load(Ordering::Relaxed) != job {
                return Err("cancelled".to_string());
            }
            *last = (key, photos.clone());
        }
        Ok(photos)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Keep / review / reject for the project last returned by `scan_project`, re-run on the backend so the ranking
/// logic never ships as readable front-end JS. Only the small bits that can change between calls (the keep ratio,
/// and any photos the user overrode by hand) cross the IPC boundary; the photos themselves stay cached in `AppState`.
/// `key` must be the project the photos were scanned for: otherwise the answer would be another project's and is refused.
/// Deciding is seconds on a very large project, so it runs off the main thread.
#[tauri::command]
async fn decide_photos(app: AppHandle, key: String, keep_ratio: f32, pick_mode: Option<String>, one_per_group: Option<bool>, overrides: std::collections::HashMap<String, String>) -> Result<std::collections::HashMap<String, decide::Decision>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let photos = st.last_photos.lock().unwrap();
        if photos.0 != key {
            return Err("stale".to_string());
        }
        Ok(decide::decide(&photos.1, keep_ratio, decide::PickMode::parse(pick_mode.as_deref()), one_per_group.unwrap_or(false), &overrides))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The projects (named folder sets and every decision made on their photos) live in `projects.json` in the app's data folder,
/// read whole here and written whole, atomically, by `projects_save`. The frontend keeps the shape; the backend only keeps it safe.
#[tauri::command]
async fn projects_load(app: AppHandle) -> Result<Option<String>, String> {
    let path = data_dir(&app).join("projects.json");
    tauri::async_runtime::spawn_blocking(move || match std::fs::read_to_string(&path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Writes are serialised and go through a temporary file and a rename, so a crash mid-write leaves the previous file intact.
static PROJECTS_WRITE: Mutex<()> = Mutex::new(());

#[tauri::command]
async fn projects_save(app: AppHandle, json: String) -> Result<(), String> {
    let path = data_dir(&app).join("projects.json");
    tauri::async_runtime::spawn_blocking(move || {
        let _one_at_a_time = PROJECTS_WRITE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let tmp = path.with_extension("json.tmp");
        write_synced(&tmp, json.as_bytes()).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Writes the bytes and waits until they are on disk, so the rename that follows can never point at an empty or half-written file
/// after a power cut.
pub(crate) fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

/// Sets aside a `projects.json` the frontend could not read (renamed with a timestamp, never deleted), so a fresh one can be started
/// without destroying what might still be repaired by hand.
#[tauri::command]
async fn projects_quarantine(app: AppHandle) -> Result<String, String> {
    let path = data_dir(&app).join("projects.json");
    tauri::async_runtime::spawn_blocking(move || {
        let _one_at_a_time = PROJECTS_WRITE.lock().unwrap_or_else(|e| e.into_inner());
        let aside = quarantine_name(&path);
        std::fs::rename(&path, &aside).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(aside.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// `name.json` → `name.json.corrupt-<unix seconds>`.
pub fn quarantine_name(path: &Path) -> PathBuf {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(format!(".corrupt-{secs}"));
    path.with_file_name(name)
}

/// Writes "my settings and taste" to `path` (see `taste.rs`): the settings object the frontend passes, every style's examples as
/// embeddings, no photos and no paths.
/// "我的设置与喜好"打成一个包（JSON 字节）：导出到文件和备份到云端用的是同一份。
fn taste_pack_bytes(app: &AppHandle, settings: serde_json::Value) -> Result<(Vec<u8>, taste::Summary), String> {
    let version = app.package_info().version.to_string();
    let st = app.state::<AppState>();
    let styles = st.prefs.lock().unwrap();
    let (pack, summary) = with_cache(app, |c| taste::pack(&styles, settings, &version, |k| c.embedding(k)));
    drop(styles);
    let bytes = serde_json::to_vec_pretty(&pack).map_err(|e| e.to_string())?;
    Ok((bytes, summary))
}

/// 把一个包里的喜好并进本机的风格（从文件导入和从云端恢复都走这里），返回摘要和包里带的设置。
fn taste_apply(app: &AppHandle, pack: taste::Pack) -> Result<(taste::Summary, serde_json::Value), String> {
    let st = app.state::<AppState>();
    let (summary, touched) = {
        let mut styles = st.prefs.lock().unwrap();
        taste::unpack(&mut styles, &pack)?
    };
    for id in &touched {
        retrain_outside_lock(app, Some(id));
    }
    let pending = st.prefs.lock().unwrap().to_save();
    if let Some(p) = pending {
        p.write();
    }
    Ok((summary, pack.settings))
}

#[tauri::command]
async fn taste_export(app: AppHandle, path: String, settings: serde_json::Value) -> Result<taste::Summary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (bytes, summary) = taste_pack_bytes(&app, settings)?;
        let out = PathBuf::from(&path);
        let tmp = out.with_extension("json.tmp");
        write_synced(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &out).map_err(|e| format!("{}: {e}", out.display()))?;
        Ok(summary)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn taste_import(app: AppHandle, path: String) -> Result<(taste::Summary, serde_json::Value), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let text = std::fs::read(&path).map_err(|e| format!("{path}: {e}"))?;
        let pack: taste::Pack = serde_json::from_slice(&text).map_err(|e| format!("{path}: {e}"))?;
        taste_apply(&app, pack)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- 云备份

/// 设置页看到的一个备份目标；从不带 Secret。
#[derive(Serialize, Clone)]
struct TargetStatus {
    id: String,
    name: String,
    kind: String,
    account: String,
    bucket: String,
    access_key: String,
    has_secret: bool,
    last_ok: u64,
    last_error: Option<String>,
}

#[derive(Serialize, Clone)]
struct BackupStatus {
    /// 至少有一个能用的目标。
    configured: bool,
    enabled: bool,
    /// 自动备份保留的份数。
    keep: usize,
    targets: Vec<TargetStatus>,
}

fn backup_status_of(dir: &Path) -> BackupStatus {
    let c = r2::load(dir);
    BackupStatus {
        configured: c.configured(),
        enabled: c.enabled,
        keep: c.keep(),
        targets: c.targets.iter().map(|t| TargetStatus { id: t.id.clone(), name: t.name.clone(), kind: t.kind.clone(), account: t.account.clone(), bucket: t.bucket.clone(), access_key: t.access_key.clone(), has_secret: r2::saved_secret(&t.id).is_some(), last_ok: t.last_ok, last_error: t.last_error.clone() }).collect(),
    }
}

fn target_secret(t: &r2::Target) -> Result<String, String> {
    r2::saved_secret(&t.id).ok_or_else(|| format!("the Secret Access Key of \"{}\" is not in the Keychain: enter it again in Settings", t.name))
}

#[tauri::command]
fn backup_status(app: AppHandle) -> BackupStatus {
    backup_status_of(&data_dir(&app))
}

/// 添加或修改一个备份目标（`id` 为空 = 添加）：先连一次桶，连不上什么都不改。Secret 只进钥匙串；修改时留空就沿用。
#[tauri::command]
async fn backup_save_target(app: AppHandle, id: Option<String>, name: String, kind: String, account: String, bucket: String, access_key: String, secret: Option<String>) -> Result<BackupStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if kind != "r2" {
            return Err(format!("unknown backup kind {kind}"));
        }
        let dir = data_dir(&app);
        let mut cfg = r2::load(&dir);
        let id = id.filter(|s| !s.is_empty()).unwrap_or_else(|| format!("t{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)));
        let old = cfg.targets.iter().find(|t| t.id == id).cloned();
        let mut t = r2::Target { id: id.clone(), name: name.trim().to_string(), kind, account: account.trim().to_string(), bucket: bucket.trim().to_string(), access_key: access_key.trim().to_string(), last_ok: old.as_ref().map_or(0, |o| o.last_ok), last_error: None };
        if t.name.is_empty() {
            t.name = format!("Cloudflare R2 · {}", t.bucket);
        }
        if !t.configured() {
            return Err("the endpoint, the bucket and the Access Key ID are all needed".to_string());
        }
        let secret = match secret.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(s) => s.to_string(),
            None => r2::saved_secret(&id).ok_or("the Secret Access Key is needed")?,
        };
        r2::check(&t, &secret)?;
        r2::remember_secret(&id, &secret)?;
        match cfg.targets.iter_mut().find(|x| x.id == id) {
            Some(slot) => *slot = t,
            None => cfg.targets.push(t),
        }
        r2::save(&dir, &cfg)?;
        Ok(backup_status_of(&dir))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 移除一个目标：忘掉它的凭据；桶里已经有的备份不动。
#[tauri::command]
async fn backup_remove_target(app: AppHandle, id: String) -> Result<BackupStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = data_dir(&app);
        let mut cfg = r2::load(&dir);
        cfg.targets.retain(|t| t.id != id);
        r2::forget_secret(&id);
        r2::save(&dir, &cfg)?;
        Ok(backup_status_of(&dir))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 自动备份的开关和保留份数（目标另外管）。
#[tauri::command]
async fn backup_set_options(app: AppHandle, enabled: Option<bool>, keep: Option<usize>) -> Result<BackupStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = data_dir(&app);
        let mut cfg = r2::load(&dir);
        if let Some(e) = enabled {
            cfg.enabled = e;
        }
        if let Some(k) = keep {
            cfg.keep = k.clamp(1, 1000);
        }
        r2::save(&dir, &cfg)?;
        Ok(backup_status_of(&dir))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 备份一次到每个目标（自动触发和"立即备份"都是它）："最新"一份，再按天留一份副本并只留最近几份。
/// 哪个目标失败记在它自己的状态里；全都失败才算错。
#[tauri::command]
async fn backup_now(app: AppHandle, settings: serde_json::Value) -> Result<BackupStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = data_dir(&app);
        let mut cfg = r2::load(&dir);
        if !cfg.configured() {
            return Err("cloud backup is not set up".to_string());
        }
        let (bytes, _) = taste_pack_bytes(&app, settings)?;
        let copy = format!("{}/{}.json", r2::KEY_AUTO, r2::today());
        let keep = cfg.keep();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let mut failures = Vec::new();
        let total = cfg.targets.iter().filter(|t| t.configured()).count();
        for t in cfg.targets.iter_mut().filter(|t| t.configured()) {
            let r = target_secret(t)
                .and_then(|secret| r2::put(t, &secret, r2::KEY_LATEST, &bytes).and_then(|_| r2::put(t, &secret, &copy, &bytes)).and_then(|_| r2::prune_auto(t, &secret, keep).map(|_| ())));
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
        r2::save(&dir, &cfg)?;
        if failures.len() == total { Err(failures.join("; ")) } else { Ok(backup_status_of(&dir)) }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 从某个目标的"最新"备份恢复：喜好并进本机的风格，设置交给页面去套用。
#[tauri::command]
async fn backup_restore(app: AppHandle, id: String) -> Result<(taste::Summary, serde_json::Value), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = data_dir(&app);
        let cfg = r2::load(&dir);
        let t = cfg.targets.iter().find(|t| t.id == id).ok_or("unknown backup target")?;
        let secret = target_secret(t)?;
        let bytes = r2::get(t, &secret, r2::KEY_LATEST)?.ok_or("there is no backup in that bucket yet")?;
        let pack: taste::Pack = serde_json::from_slice(&bytes).map_err(|e| format!("the backup cannot be read: {e}"))?;
        taste_apply(&app, pack)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- 网络共享（NAS）

fn nas_statuses(app: &AppHandle) -> Vec<nas::ShareStatus> {
    let dir = data_dir(app);
    let st = app.state::<AppState>();
    nas::load(&dir)
        .iter()
        .map(|s| nas::ShareStatus { id: s.id.clone(), name: s.name.clone(), host: s.host.clone(), user: s.user.clone(), enabled: s.enabled, local: nas::local_dir(&dir, &s.id).to_string_lossy().into_owned(), connected: st.nas.is_connected(&s.id) })
        .collect()
}

/// 没连着就用记住的密码连上；没有记住的密码就说明缺什么。停用的不连。
fn nas_connect_saved(app: &AppHandle, s: &nas::Share) -> Result<(), String> {
    let st = app.state::<AppState>();
    if st.nas.is_connected(&s.id) {
        return Ok(());
    }
    if !s.enabled {
        return Err(format!("the NAS \"{}\" is turned off in Settings", s.name));
    }
    let pw = nas::saved_password(s).ok_or_else(|| format!("the NAS \"{}\" needs its password: connect it in Settings", s.name))?;
    st.nas.connect(s, &pw).map_err(|e| format!("{}: {e}", s.name))
}

/// 项目文件夹里落在共享镜像上的，先把远端同步进来（进度发 "nas-progress"）；`job` 变了就停（页面离开项目时会 `scan_cancel`）。
fn nas_sync_dirs(app: &AppHandle, dirs: &[String], job: u64) -> Result<(), String> {
    let data = data_dir(app);
    let shares = nas::load(&data);
    let st = app.state::<AppState>();
    for d in dirs {
        let Some((s, rel)) = nas::share_of_path(&data, &shares, Path::new(d)) else { continue };
        nas_connect_saved(app, s)?;
        let emit = app.clone();
        let watch = app.clone();
        let local = nas::local_dir(&data, &s.id);
        st.nas
            .sync(&s.id, &rel, &local, move |p| { let _ = emit.emit("nas-progress", p); }, move || watch.state::<AppState>().scan_job.load(Ordering::Relaxed) != job)
            .map_err(|e| if e.contains("ancelled") { "cancelled".to_string() } else { format!("{}: {e}", s.name) })?;
    }
    Ok(())
}

/// 添加或编辑 NAS 时的"测试连接"：连上这台服务器，返回它的共享名；什么都不记。编辑时密码留空就用记住的。
#[tauri::command]
async fn nas_probe(app: AppHandle, host: String, user: String, password: String, id: Option<String>) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let password = match (password.is_empty(), id) {
            (true, Some(id)) => {
                let dir = data_dir(&app);
                let s = nas::load(&dir).into_iter().find(|s| s.id == id).ok_or("unknown NAS")?;
                if s.user == user.trim() { nas::saved_password(&s).unwrap_or_default() } else { String::new() }
            }
            _ => password,
        };
        app.state::<AppState>().nas.probe(&host, user.trim(), &password)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 编辑一台 NAS：改显示名、地址、账号或密码；先用新的连一次，连不上什么都不改。id 不变，镜像和项目都还在。
#[tauri::command]
async fn nas_update(app: AppHandle, id: String, name: String, host: String, user: String, password: String, remember: bool) -> Result<Vec<nas::ShareStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let host = nas::check_host(&host)?;
        let dir = data_dir(&app);
        let mut list = nas::load(&dir);
        let old = list.iter().find(|s| s.id == id).cloned().ok_or("unknown NAS")?;
        if list.iter().any(|s| s.id != id && s.host.eq_ignore_ascii_case(&host) && s.user == user.trim()) {
            return Err("this NAS is already in the list".to_string());
        }
        let next = nas::Share { id: id.clone(), name: if name.trim().is_empty() { host.clone() } else { name.trim().to_string() }, host, user: user.trim().to_string(), enabled: old.enabled };
        // 密码留空 = 用记住的（账号没变时）
        let same_account = next.host == old.host && next.user == old.user;
        let pw = if password.is_empty() { if same_account { nas::saved_password(&old).unwrap_or_default() } else { String::new() } } else { password };
        app.state::<AppState>().nas.connect(&next, &pw)?;
        if !same_account {
            nas::forget_password(&old);
        }
        if remember && (!same_account || !pw.is_empty()) {
            nas::remember_password(&next, &pw)?;
        }
        if let Some(s) = list.iter_mut().find(|s| s.id == id) {
            *s = next;
        }
        nas::save(&dir, &list)?;
        Ok(nas_statuses(&app))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn nas_list(app: AppHandle) -> Vec<nas::ShareStatus> {
    tauri::async_runtime::spawn_blocking(move || nas_statuses(&app)).await.unwrap_or_default()
}

/// 添加一台 NAS 并马上连接；连不上就什么都不记。共享不用选，新建项目时浏览。
#[tauri::command]
async fn nas_add(app: AppHandle, name: String, host: String, user: String, password: String, remember: bool) -> Result<Vec<nas::ShareStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let host = nas::check_host(&host)?;
        let dir = data_dir(&app);
        let mut list = nas::load(&dir);
        if list.iter().any(|s| s.host.eq_ignore_ascii_case(&host) && s.user == user.trim()) {
            return Err("this NAS is already in the list".to_string());
        }
        let id = format!("n{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
        let name = if name.trim().is_empty() { host.clone() } else { name.trim().to_string() };
        let share = nas::Share { id, name, host, user: user.trim().to_string(), enabled: true };
        app.state::<AppState>().nas.connect(&share, &password)?;
        if remember {
            nas::remember_password(&share, &password)?;
        }
        list.push(share);
        nas::save(&dir, &list)?;
        Ok(nas_statuses(&app))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 移除：断开、忘掉密码、删掉镜像里的本地副本（NAS 上的照片不动）。
#[tauri::command]
async fn nas_remove(app: AppHandle, id: String) -> Result<Vec<nas::ShareStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = data_dir(&app);
        let mut list = nas::load(&dir);
        if let Some(s) = list.iter().find(|s| s.id == id).cloned() {
            app.state::<AppState>().nas.disconnect(&s.id);
            nas::forget_password(&s);
            let _ = std::fs::remove_dir_all(nas::local_dir(&dir, &s.id));
            list.retain(|x| x.id != id);
            nas::save(&dir, &list)?;
        }
        Ok(nas_statuses(&app))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 连接一个记住的共享：用记住的密码，或者现在给的密码（给了就记住）。
#[tauri::command]
async fn nas_connect(app: AppHandle, id: String, password: Option<String>) -> Result<Vec<nas::ShareStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = data_dir(&app);
        let s = nas::load(&dir).into_iter().find(|s| s.id == id).ok_or("unknown share")?;
        match password.as_deref().filter(|p| !p.is_empty()) {
            Some(pw) => {
                app.state::<AppState>().nas.connect(&s, pw)?;
                nas::remember_password(&s, pw)?;
            }
            None => nas_connect_saved(&app, &s)?,
        }
        Ok(nas_statuses(&app))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn nas_disconnect(app: AppHandle, id: String) -> Result<Vec<nas::ShareStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<AppState>().nas.disconnect(&id);
        Ok(nas_statuses(&app))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 启用 / 停用：停用就断开，启用就用记住的密码连上（连不上也先记下开关，状态里能看到）。
#[tauri::command]
async fn nas_set_enabled(app: AppHandle, id: String, enabled: bool) -> Result<Vec<nas::ShareStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = data_dir(&app);
        let mut list = nas::load(&dir);
        let s = list.iter_mut().find(|s| s.id == id).ok_or("unknown NAS")?;
        s.enabled = enabled;
        let s = s.clone();
        nas::save(&dir, &list)?;
        if enabled {
            let _ = nas_connect_saved(&app, &s);
        } else {
            app.state::<AppState>().nas.disconnect(&id);
        }
        Ok(nas_statuses(&app))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 启动时在后台把启用的 NAS 都连上，打开设置或新建项目时就不用等。
fn nas_connect_all(app: AppHandle) {
    std::thread::spawn(move || {
        let dir = data_dir(&app);
        for s in nas::load(&dir).into_iter().filter(|s| s.enabled) {
            if let Err(e) = nas_connect_saved(&app, &s) {
                eprintln!("NAS {}: {e}", s.name);
            }
        }
        let _ = app.emit("nas-changed", ());
    });
}

/// 打开项目前：这些文件夹所在的共享先连上，并返回它们的状态，页面好说明哪一个连不上。
#[tauri::command]
async fn nas_ensure(app: AppHandle, dirs: Vec<String>) -> Vec<nas::ShareStatus> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = data_dir(&app);
        let shares = nas::load(&dir);
        let mut out = Vec::new();
        for s in &shares {
            if dirs.iter().any(|d| nas::share_of_path(&dir, std::slice::from_ref(s), Path::new(d)).is_some()) {
                let _ = nas_connect_saved(&app, s);
                out.extend(nas_statuses(&app).into_iter().filter(|x| x.id == s.id));
            }
        }
        out
    })
    .await
    .unwrap_or_default()
}

/// 浏览 NAS 上的一个位置（新建项目时选文件夹用）：根是共享列表。
#[derive(Serialize)]
struct Listing {
    /// `共享/文件夹/…`（`/` 分隔，根是空串）。
    path: String,
    /// 这个文件夹在镜像里的本地路径：选了它，项目的文件夹就是这个。根（还没选共享）时为空，不能当项目文件夹。
    local: String,
    entries: Vec<nas::Entry>,
}

#[tauri::command]
async fn nas_browse(app: AppHandle, id: String, path: String) -> Result<Listing, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = data_dir(&app);
        let s = nas::load(&dir).into_iter().find(|s| s.id == id).ok_or("unknown share")?;
        nas_connect_saved(&app, &s)?;
        let rel = path.trim_matches('/').to_string();
        let entries = app.state::<AppState>().nas.browse(&s.id, &rel)?;
        let local = if rel.is_empty() { String::new() } else { nas::local_dir(&dir, &s.id).join(&rel).to_string_lossy().into_owned() };
        Ok(Listing { path: rel, local, entries })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stops the scan in progress: it ends at the next photo, and what it analysed by then is already in the cache. Called when
/// the person asks for it, and whenever the page leaves a project (its scan may still be running).
#[tauri::command]
fn scan_cancel(app: AppHandle) {
    app.state::<AppState>().scan_job.fetch_add(1, Ordering::Relaxed);
}

#[derive(Serialize)]
struct ExportResult {
    copied: usize,
    skipped: usize,
    dest: String,
}

/// A path in `dir` that does not exist yet: `name.jpg`, then `name (2).jpg`, `name (3).jpg`, ...
fn unique_path(dir: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let p = Path::new(name);
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|c| !c.exists())
        .unwrap()
}

/// Copies the given photos into `dest`. Originals are never touched and nothing is overwritten.
#[tauri::command]
async fn export_photos(app: AppHandle, paths: Vec<String>, dest: String) -> Result<ExportResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dest_dir = PathBuf::from(&dest);
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
            let _ = app.emit("export-progress", Progress { done: i + 1, total });
        }
        Ok(ExportResult { copied, skipped, dest })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
struct TrashResult {
    moved: usize,
    failed: Vec<String>,
    /// Where photos on network volumes went instead of the Trash (one entry per volume), for the message.
    network_bins: Vec<String>,
}

/// Moves photos to the system trash (recoverable from Finder / Explorer). Only image files are accepted,
/// so this can never be used to trash anything else.
#[tauri::command]
async fn trash_photos(app: AppHandle, paths: Vec<String>) -> Result<TrashResult, String> {
    tauri::async_runtime::spawn_blocking(move || trash_photos_in(Some(&app), paths)).await.map_err(|e| e.to_string())
}

/// 真正的搬运；`app` 为 `None` 时（单元测试）不认识任何 NAS 共享，照片都按本机的处理。
fn trash_photos_in(app: Option<&AppHandle>, paths: Vec<String>) -> TrashResult {
    #[allow(unused_mut)]
    let mut ctx = trash::TrashContext::default();
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        // NSFileManager needs no Finder automation permission prompt.
        ctx.set_delete_method(DeleteMethod::NsFileManager);
    }
    let data = app.map(data_dir).unwrap_or_default();
    let shares = app.map(|_| nas::load(&data)).unwrap_or_default();
    let (mut moved, mut failed, mut network_bins) = (0usize, Vec::new(), Vec::<String>::new());
    // 共享镜像里的照片按共享分组：在 NAS 上一次移完，成功的再删掉本地副本
    let mut on_nas: std::collections::HashMap<String, Vec<(String, String)>> = std::collections::HashMap::new();
    let mut local = Vec::new();
    for p in paths {
        let path = PathBuf::from(&p);
        if !(analyze::is_image(&path) && path.is_file()) {
            failed.push(p);
            continue;
        }
        match nas::share_of_path(&data, &shares, &path) {
            Some((s, rel)) => on_nas.entry(s.id.clone()).or_default().push((p, rel)),
            None => local.push(path),
        }
    }
    if let Some(app) = app {
        let st = app.state::<AppState>();
        for (id, items) in on_nas {
            let Some(s) = shares.iter().find(|s| s.id == id) else { continue };
            let _ = nas_connect_saved(app, s);
            let results = st.nas.recycle(&id, items.iter().map(|(_, rel)| rel.clone()).collect());
            for ((p, _), r) in items.into_iter().zip(results) {
                match r {
                    Ok(target) => {
                        let _ = std::fs::remove_file(&p);
                        moved += 1;
                        let at = nas::recycle_url(s, nas::split_share(&target).0);
                        if !network_bins.contains(&at) {
                            network_bins.push(at);
                        }
                    }
                    Err(e) => {
                        eprintln!("recycle on {}: {e}", s.name);
                        failed.push(p);
                    }
                }
            }
        }
    }
    for path in local {
        // a network volume the person mounted in the Finder has no Trash: the photo goes to a recycle folder at the root of that volume instead
        let ok = match nas::volume_of(&path).filter(|(_, fstype)| nas::is_network_fs(fstype)) {
            Some((root, _)) => {
                let target = nas::network_bin_target(&root, &path);
                let bin = target.parent().and_then(|d| d.parent()).map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
                let done = target.parent().map(std::fs::create_dir_all).map_or(false, |r| r.is_ok()) && std::fs::rename(&path, &target).is_ok();
                if done && !network_bins.contains(&bin) {
                    network_bins.push(bin);
                }
                done
            }
            None => ctx.delete(&path).is_ok(),
        };
        if ok {
            moved += 1;
        } else {
            failed.push(path.to_string_lossy().into_owned());
        }
    }
    TrashResult { moved, failed, network_bins }
}

/// Path of a large cached preview for the viewer (created on first request).
#[tauri::command]
async fn large_preview(app: AppHandle, path: String, full: Option<bool>) -> Result<String, String> {
    let dir = thumbs_dir(&app)?.join("large");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // the web view may only load files inside the asset scope; anything else (a mounted share under /private, a symlink target) gets a copy
    let showable = app.asset_protocol_scope().is_allowed(&path);
    tauri::async_runtime::spawn_blocking(move || {
        let src = Path::new(&path);
        // The original file itself whenever the web view can show it: that is the only way to see what the camera recorded (HDR, wide
        // colour, every pixel). Only formats it cannot show (camera RAW) or enormous files get a rendered preview.
        // Only the full-size viewer gets it (decoding a 15-30 MB original is too heavy for the several previews on screen in compare or single).
        if full.unwrap_or(false) && showable && imageio::web_can_show(src) && std::fs::metadata(src).map_or(false, |m| m.len() < 120 * 1024 * 1024) {
            return Ok(path);
        }
        let out = analyze::cache_name(src, ".l.jpg", &dir);
        if !out.exists() {
            let tmp = out.with_extension("tmp");
            analyze::large_preview(src, &tmp)?;
            std::fs::rename(&tmp, &out).map_err(|e| e.to_string())?;
        }
        Ok(out.to_string_lossy().into_owned())
    }).await.map_err(|e| e.to_string())?
}

/// The sharp part of a photo for the full-size viewer when it is zoomed: the region (fractions of the photo, top-left origin) at up to `out_w`
/// pixels, as a JPEG next to the thumbnails. Keyed by region, so going back to a place already seen is instant.
#[tauri::command]
async fn photo_detail(app: AppHandle, path: String, x: f64, y: f64, w: f64, h: f64, out_w: u32) -> Result<String, String> {
    let dir = thumbs_dir(&app)?.join("detail");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let src = Path::new(&path);
        let tag = format!(".d{:.3}-{:.3}-{:.3}-{:.3}-{}.jpg", x, y, w, h, out_w);
        let out = analyze::cache_name(src, &tag, &dir);
        if !out.exists() {
            imageio::crop(src, &out, [x, y, w, h], out_w)?;
            // every new crop is another ~2 MB: look at the folder's size now and then
            static CROPS: AtomicU64 = AtomicU64::new(0);
            if CROPS.fetch_add(1, Ordering::Relaxed) % 24 == 23 {
                if let Some(thumbs) = dir.parent() {
                    scratch::prune_detail(thumbs);
                }
            }
        }
        Ok(out.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The video of a motion photo, written next to the thumbnails (once) so the viewer can play it.
#[tauri::command]
async fn motion_clip(app: AppHandle, path: String) -> Result<String, String> {
    let dir = thumbs_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let src = Path::new(&path);
        let out = analyze::cache_name(src, ".m.mp4", &dir);
        if !out.exists() {
            media::extract_motion(src, &out)?;
        }
        Ok(out.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize, Default)]
struct FileInfo {
    format: Option<String>,
    bytes: u64,
    color_space: Option<String>,
    bit_depth: Option<String>,
    camera: Option<String>,
    lens: Option<String>,
    focal: Option<String>,
    aperture: Option<String>,
    shutter: Option<String>,
    iso: Option<String>,
    software: Option<String>,
    taken: Option<String>,
}

/// Basic file facts for the inspector: container format, size, colour space and the shooting parameters stored in EXIF.
#[tauri::command]
async fn photo_info(path: String) -> Result<FileInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = Path::new(&path);
        let mut info = FileInfo { bytes: std::fs::metadata(p).map(|m| m.len()).unwrap_or(0), ..Default::default() };
        info.format = image::ImageReader::open(p).ok().and_then(|r| r.with_guessed_format().ok()).and_then(|r| r.format())
            .map(|f| format!("{f:?}").to_uppercase())
            .or_else(|| p.extension().map(|e| e.to_string_lossy().to_uppercase()));
        let Ok(file) = std::fs::File::open(p) else { return info; };
        let Ok(ex) = exif::Reader::new().read_from_container(&mut std::io::BufReader::new(file)) else { return info; };
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
        info.color_space = ex.get_field(exif::Tag::ColorSpace, exif::In::PRIMARY).and_then(|f| f.value.get_uint(0)).map(|v| match v { 1 => "sRGB".to_string(), 2 => "Adobe RGB".to_string(), 0xFFFF => "Uncalibrated".to_string(), n => n.to_string() });
        info.bit_depth = get(exif::Tag::BitsPerSample);
        info
    }).await.map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- search

#[derive(Serialize)]
struct SearchStatus {
    total: usize,
    ready: usize,
    /// The text model is present, so searching is possible at all.
    available: bool,
}

fn image_keys(dirs: &[String]) -> Vec<String> {
    let mut all: Vec<PathBuf> = dirs.iter().flat_map(|d| list_images(d)).collect();
    all.sort();
    all.dedup();
    all.into_iter().map(|p| p.to_string_lossy().into_owned()).collect()
}

/// How many of the photos can already be searched. Photos analysed with the quick model always can; the others need the index.
#[tauri::command]
async fn search_status(app: AppHandle, dirs: Vec<String>) -> Result<SearchStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let keys = image_keys(&dirs);
        let ready = with_cache(&app, |c| keys.iter().filter(|k| c.semantic(k).is_some() && c.color(k).is_some()).count());
        SearchStatus { total: keys.len(), ready, available: textsearch::TextEncoder::available(&models_dir(&app)) }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Builds the search vectors that are missing. It reads the 640 px thumbnails made during analysis, so it is fast (tens of
/// milliseconds a photo) and never touches the original files. Progress goes out as `search-index-progress`.
#[tauri::command]
async fn search_index(app: AppHandle, dirs: Vec<String>) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let keys = image_keys(&dirs);
        let pending = with_cache(&app, |c| c.without_index(&keys));
        if pending.is_empty() {
            return Ok(0);
        }
        // the larger analysis model leaves no ViT-B/32 vector behind, so those photos get one here; the rest only need their colours
        let scorer = if pending.iter().any(|p| p.4) { Some(scorer::ClipScorer::load(&models_dir(&app), true, scorer::Quality::Fast)?) } else { None };
        let total = pending.len();
        let mut done = 0usize;
        let _ = app.emit("search-index-progress", Progress { done, total });
        for chunk in pending.chunks(48) {
            let made: Vec<(String, u64, i64, Option<Vec<f32>>, Vec<u8>)> = chunk
                .par_iter()
                .filter_map(|(key, thumb, size, mtime, need_vec)| {
                    let img = image::open(thumb).ok()?;
                    let sem = match (need_vec, scorer.as_ref()) {
                        (true, Some(s)) => Some(<scorer::ClipScorer as scorer::AiScorer>::analyze(s, &img).ok()?.embedding),
                        _ => None,
                    };
                    Some((key.clone(), *size, *mtime, sem, colorsig::signature(&img)))
                })
                .collect();
            with_cache(&app, |c| {
                for (key, size, mtime, sem, color) in made {
                    c.set_index(&key, size, mtime, sem, color);
                }
            });
            done += chunk.len();
            let _ = app.emit("search-index-progress", Progress { done, total });
        }
        with_cache(&app, |c| c.save());
        Ok(total)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
struct Hit {
    path: String,
    name: String,
    thumb: String,
    width: u32,
    height: u32,
    score: f32,
}

/// Photos that match a description, best first. Words about how many people are in the picture are answered from the face
/// detector (it counts far better than an image-text model); the rest is compared through the text model.
#[tauri::command]
async fn search_photos(app: AppHandle, dirs: Vec<String>, query: String) -> Result<Vec<Hit>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let q = textsearch::parse(&query);
        let qv = match &q.text {
            Some(t) => {
                let encoder = {
                    let st = app.state::<AppState>();
                    let mut g = st.text.lock().unwrap();
                    if g.is_none() {
                        *g = Some(std::sync::Arc::new(textsearch::TextEncoder::load(&models_dir(&app))?));
                    }
                    g.as_ref().unwrap().clone()
                };
                Some(encoder.encode_query(t)?)
            }
            None => None,
        };
        let keys = image_keys(&dirs);
        let hits: Vec<Hit> = with_cache(&app, |c| {
            let rows: Vec<(&Photo, Option<&[f32]>, Option<&[u8]>)> =
                keys.iter().filter_map(|k| c.photo(k).map(|p| (p, c.semantic(k), c.color(k)))).collect();
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
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Deserialize)]
struct SceneReq {
    id: usize,
    /// The scene's photos, best first.
    paths: Vec<String>,
}
#[derive(Serialize, Clone)]
struct SceneText {
    id: usize,
    text: String,
}
#[derive(Serialize, Clone)]
struct SceneProgress {
    done: usize,
    total: usize,
}

/// Names each scene of a finished project. Two steps: first a large vocabulary of everyday and travel places (see scene_vocab.rs),
/// matched against a few of the scene's photos, which is quick and needs no download; then, for the scenes it could not name and only
/// if `caption` is set, a sentence written by the captioning model about the place (the people painted out first), in Chinese when
/// `zh`. Results arrive one by one as `scene-text` events; a newer call cancels an older one. Written sentences are remembered on disk.
#[tauri::command]
async fn scene_text(app: AppHandle, scenes: Vec<SceneReq>, zh: bool, caption: bool) -> Result<(), String> {
    let job = app.state::<AppState>().scene_job.fetch_add(1, Ordering::SeqCst) + 1;
    tauri::async_runtime::spawn_blocking(move || {
        let dir = models_dir(&app);
        let st = app.state::<AppState>();
        let current = || st.scene_job.load(Ordering::SeqCst) == job;

        // 1. the vocabulary: the mean ViT-B/32 embedding of up to six photos of each scene
        const PER_SCENE: usize = 6;
        let known: Vec<Vec<Option<Vec<f32>>>> = with_cache(&app, |c| scenes.iter().map(|s| s.paths.iter().take(PER_SCENE).map(|k| c.semantic(k).map(|v| v.to_vec())).collect()).collect());
        let thumbs: Vec<Vec<Option<String>>> = with_cache(&app, |c| {
            scenes.iter().zip(&known).map(|(s, kn)| s.paths.iter().take(PER_SCENE).zip(kn).map(|(k, v)| if v.is_some() { None } else { c.photo(k).map(|p| p.thumb.clone()) }).collect()).collect()
        });
        let scorer = if thumbs.iter().flatten().any(|t| t.is_some()) { scorer::ClipScorer::load(&dir, true, scorer::Quality::Fast).ok() } else { None };
        let mut unnamed: Vec<&SceneReq> = Vec::new();
        let mut names: Vec<SceneText> = Vec::new();
        for (n, scene) in scenes.iter().enumerate() {
            if !current() { return Ok(()); }
            let mut mean = vec![0f32; scene_vocab::DIM];
            let mut count = 0;
            for (v, t) in known[n].iter().zip(&thumbs[n]) {
                let vec = match (v, t, scorer.as_ref()) {
                    (Some(v), _, _) => Some(v.clone()),
                    (None, Some(t), Some(s)) => image::open(t).ok().and_then(|img| <scorer::ClipScorer as scorer::AiScorer>::analyze(s, &img).ok()).map(|o| o.embedding),
                    _ => None,
                };
                if let Some(v) = vec.filter(|v| v.len() == scene_vocab::DIM) {
                    mean.iter_mut().zip(&v).for_each(|(a, b)| *a += b);
                    count += 1;
                }
            }
            let named = (count > 0).then(|| {
                let norm = mean.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
                mean.iter_mut().for_each(|x| *x /= norm);
                scene_vocab::name(&mean)
            }).flatten();
            match named {
                Some(name) => names.push(SceneText { id: scene.id, text: if zh { name.zh.to_string() } else { name.en } }),
                None => unnamed.push(scene),
            }
        }
        // one event for all of them: a project can have a thousand scenes
        if !names.is_empty() && current() { let _ = app.emit("scene-texts", &names); }
        let total = unnamed.len();
        let _ = app.emit("scene-text-progress", SceneProgress { done: 0, total });
        if !caption || total == 0 { return Ok(()); }

        // 2. the scenes the vocabulary could not name: a sentence from the captioning model
        let captioner = {
            let mut g = st.captioner.lock().unwrap();
            if g.is_none() {
                if !florence::Captioner::available(&dir) { return Ok(()); }
                *g = Some(std::sync::Arc::new(florence::Captioner::load(&dir)?));
            }
            g.clone().unwrap()
        };
        let translator = if zh {
            let mut g = st.translator.lock().unwrap();
            if g.is_none() && translate::Translator::available(&dir) {
                *g = translate::Translator::load(&dir).ok().map(std::sync::Arc::new);
            }
            g.clone()
        } else {
            None
        };
        let store = data_dir(&app).join("scene-sentences.json");
        {
            let mut g = st.scene_sentences.lock().unwrap();
            if g.is_none() {
                *g = Some(std::fs::read(&store).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default());
            }
        }
        let save = || { if let Some(m) = st.scene_sentences.lock().unwrap().as_ref() { let _ = std::fs::write(&store, serde_json::to_vec(m).unwrap_or_default()); } };
        let mut fresh = 0;
        for (n, scene) in unnamed.iter().enumerate() {
            if !current() { break; }
            // the photo with the fewest faces among the best few: the one with the most place in it
            let pick = with_cache(&app, |c| scene.paths.iter().take(4).filter_map(|p| c.photo(p)).min_by_key(|p| p.face_count).map(|p| p.thumb.clone()));
            if let Some(thumb) = pick {
                let known = st.scene_sentences.lock().unwrap().as_ref().and_then(|m| m.get(&thumb).cloned());
                let sentence = match known {
                    Some(s) => Some(s),
                    None => match image::open(&thumb).map_err(|e| e.to_string()).and_then(|img| captioner.scene(&img)) {
                        Ok(s) if !s.is_empty() => {
                            st.scene_sentences.lock().unwrap().as_mut().unwrap().insert(thumb.clone(), s.clone());
                            fresh += 1;
                            Some(s)
                        }
                        _ => None,
                    },
                };
                if let Some(s) = sentence {
                    let s = florence::short(&s);
                    if florence::place_like(&s) {
                        let text = match &translator { Some(t) => t.zh(&s).unwrap_or(s), None => s };
                        let _ = app.emit("scene-text", SceneText { id: scene.id, text });
                    }
                }
            }
            let _ = app.emit("scene-text-progress", SceneProgress { done: n + 1, total });
            if fresh > 0 && fresh % 8 == 0 { save(); }
        }
        save();
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- model packs (downloaded on demand)

#[derive(Default)]
struct Downloads {
    cancel: std::sync::atomic::AtomicBool,
    busy: Mutex<Option<String>>,
}

/// What the machine is, so that the app can pick the analysis that suits it without asking.
#[derive(Serialize)]
struct Machine {
    memory_gb: u64,
    cores: usize,
}

#[tauri::command]
fn machine_info() -> Machine {
    Machine { memory_gb: scorer::physical_memory() >> 30, cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) }
}

#[tauri::command]
async fn packs_status(app: AppHandle) -> Result<Vec<modelpacks::PackStatus>, String> {
    let dir = models_dir(&app);
    Ok(modelpacks::PACKS.iter().map(|p| modelpacks::PackStatus { id: p.id.to_string(), installed: modelpacks::installed(&dir, p), bytes: 0 }).collect())
}

/// How many bytes a pack still needs (0 if it is installed), so the app can say so before asking.
#[tauri::command]
async fn pack_size(app: AppHandle, id: String) -> Result<u64, String> {
    let dir = models_dir(&app);
    let p = modelpacks::pack(&id).ok_or("unknown pack")?;
    if modelpacks::installed(&dir, p) {
        return Ok(0);
    }
    tauri::async_runtime::spawn_blocking(move || modelpacks::manifest_sizes(&dir, &id)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn pack_download(app: AppHandle, id: String) -> Result<(), String> {
    let dir = downloaded_models_dir(&app);
    {
        let d = app.state::<Downloads>();
        let mut busy = d.busy.lock().unwrap();
        if busy.is_some() {
            return Err("another download is running".into());
        }
        *busy = Some(id.clone());
        d.cancel.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    let app2 = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let d = app2.state::<Downloads>();
        modelpacks::download(&dir, &id, &d.cancel, &|p| { let _ = app2.emit("model-progress", p); })
    })
    .await
    .map_err(|e| e.to_string())?;
    *app.state::<Downloads>().busy.lock().unwrap() = None;
    // the text model may have just arrived (or a broken one been replaced): load it afresh next time
    *app.state::<AppState>().text.lock().unwrap() = None;
    result
}

#[tauri::command]
fn pack_cancel(app: AppHandle) {
    app.state::<Downloads>().cancel.store(true, std::sync::atomic::Ordering::Relaxed);
}

#[tauri::command]
fn pack_remove(app: AppHandle, id: String) -> Result<(), String> {
    modelpacks::remove(&downloaded_models_dir(&app), &id)?;
    *app.state::<AppState>().text.lock().unwrap() = None;
    Ok(())
}

/// Shows a file (or folder) in Finder / Explorer.
#[tauri::command]
fn reveal(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").args(["-R", &path]).spawn();
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("explorer").arg(format!("/select,{path}")).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let r = std::process::Command::new("xdg-open").arg(Path::new(&path).parent().unwrap_or(Path::new("/"))).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

/// Fast preview thumbnails for a handful of photos (same cache location the analysis uses, so nothing is wasted).
#[tauri::command]
async fn preview_thumbs(app: AppHandle, paths: Vec<String>) -> Result<Vec<Option<String>>, String> {
    let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("thumbs");
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        paths
            .par_iter()
            .map(|p| {
                let src = Path::new(p);
                let out = analyze::cache_name(src, ".jpg", &cache_dir);
                if out.exists() || analyze::quick_thumb(src, &out).is_ok() {
                    Some(out.to_string_lossy().into_owned())
                } else {
                    None
                }
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

/// Records (or clears) one explicit user decision and retrains the taste model. Returns its new status.
fn retrain_style(app: &AppHandle, styles: &mut prefs::Styles, style: Option<&str>) {
    let p = &mut styles.get_mut(style).prefs;
    with_cache(app, |c| p.retrain(|k| c.embedding(k), || c.pool()));
}

/// The label is recorded at once; the retraining it calls for runs a moment later, once the keys have stopped, so a run of P / X
/// presses retrains once rather than every time, on a copy outside the lock, and the file is written outside it too.
#[tauri::command]
async fn record_label(app: AppHandle, path: String, label: Option<bool>, style: Option<String>) -> Result<prefs::PrefsInfo, String> {
    let st = app.state::<AppState>();
    let info = {
        let mut styles = st.prefs.lock().unwrap();
        let p = &mut styles.get_mut(style.as_deref()).prefs;
        match label {
            Some(v) => { p.labels.insert(path, v); }
            None => { p.labels.remove(&path); }
        }
        p.info()
    };
    let gen = st.retrain_gen.fetch_add(1, Ordering::Relaxed) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        if app.state::<AppState>().retrain_gen.load(Ordering::Relaxed) != gen {
            return; // a newer label will do it
        }
        retrain_outside_lock(&app, style.as_deref());
    });
    Ok(info)
}

/// Retrains a style on a copy of its preferences, so the lock is held only to take the copy and to put the weights back, then writes
/// `styles.json` outside the lock as well.
fn retrain_outside_lock(app: &AppHandle, style: Option<&str>) {
    let st = app.state::<AppState>();
    let mut copy = st.prefs.lock().unwrap().get(style).prefs.clone();
    with_cache(app, |c| copy.retrain(|k| c.embedding(k), || c.pool()));
    let pending = {
        let mut styles = st.prefs.lock().unwrap();
        styles.get_mut(style).prefs.adopt_weights(&copy);
        styles.to_save()
    };
    if let Some(pending) = pending {
        pending.write();
    }
}

#[tauri::command]
fn prefs_info(app: AppHandle, style: Option<String>) -> prefs::PrefsInfo {
    app.state::<AppState>().prefs.lock().unwrap().get(style.as_deref()).prefs.info()
}

#[tauri::command]
fn prefs_reset(app: AppHandle, style: Option<String>) -> prefs::PrefsInfo {
    let st = app.state::<AppState>();
    let mut styles = st.prefs.lock().unwrap();
    styles.get_mut(style.as_deref()).prefs.reset();
    styles.save();
    styles.get(style.as_deref()).prefs.info()
}

// ---------------------------------------------------------------- styles

#[tauri::command]
fn styles_list(app: AppHandle) -> Vec<prefs::StyleInfo> {
    app.state::<AppState>().prefs.lock().unwrap().infos()
}

#[tauri::command]
fn style_create(app: AppHandle, name: String) -> Vec<prefs::StyleInfo> {
    let st = app.state::<AppState>();
    let mut styles = st.prefs.lock().unwrap();
    styles.create(&name);
    styles.save();
    styles.infos()
}

#[tauri::command]
fn style_rename(app: AppHandle, id: String, name: String) -> Vec<prefs::StyleInfo> {
    let st = app.state::<AppState>();
    let mut styles = st.prefs.lock().unwrap();
    styles.rename(&id, &name);
    styles.save();
    styles.infos()
}

#[tauri::command]
fn style_delete(app: AppHandle, id: String) -> Vec<prefs::StyleInfo> {
    let st = app.state::<AppState>();
    let mut styles = st.prefs.lock().unwrap();
    styles.delete(&id);
    styles.save();
    styles.infos()
}

#[tauri::command]
fn style_set_default(app: AppHandle, id: String) -> Vec<prefs::StyleInfo> {
    let st = app.state::<AppState>();
    let mut styles = st.prefs.lock().unwrap();
    styles.set_default(&id);
    styles.save();
    styles.infos()
}

/// Learns a style from a folder of past picks: the photos there are what this person chooses. Only their embeddings are kept, never the
/// pictures. Progress goes out as `reference-progress`.
#[tauri::command]
async fn learn_from_folder(app: AppHandle, dir: String, style: Option<String>, compute: Option<String>, quality: Option<String>) -> Result<prefs::PrefsInfo, String> {
    let want = scorer::Quality::parse(quality.as_deref());
    let accel = compute.as_deref() != Some("cpu");
    tauri::async_runtime::spawn_blocking(move || {
        let files = list_images(&dir);
        if files.is_empty() {
            return Err("no photos in that folder".to_string());
        }
        // the embeddings must be the kind the analysed photos have (they are compared with them), so the same model is used
        let scorer = scorer::ClipScorer::load(&models_dir(&app), accel, want)?;
        let total = files.len();
        let done = AtomicUsize::new(0);
        let _ = app.emit("reference-progress", Progress { done: 0, total });
        let refs: Vec<prefs::Reference> = files
            .par_iter()
            .filter_map(|p| {
                let r = analyze::load_for_embedding(p).and_then(|img| scorer.embed(&img)).ok().map(|emb| prefs::Reference {
                    stem: p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default(),
                    emb,
                });
                let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                if d % 4 == 0 || d == total { let _ = app.emit("reference-progress", Progress { done: d, total }); }
                r
            })
            .collect();
        if refs.len() < 6 {
            return Err("at least 6 readable photos are needed to learn from a folder".to_string());
        }
        let st = app.state::<AppState>();
        let mut styles = st.prefs.lock().unwrap();
        {
            let p = &mut styles.get_mut(style.as_deref()).prefs;
            p.refs = refs;
            p.refs_from = dir;
        }
        retrain_style(&app, &mut styles, style.as_deref());
        styles.save();
        Ok(styles.get(style.as_deref()).prefs.info())
    })
    .await
    .map_err(|e| e.to_string())?
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

fn thumbs_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_cache_dir().map_err(|e| e.to_string())?.join("thumbs"))
}

/// Walks tens of thousands of thumbnails on a big install, so it runs off the main thread.
#[tauri::command]
async fn cache_size(app: AppHandle) -> Result<u64, String> {
    let dir = thumbs_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        WalkDir::new(dir)
            .into_iter()
            .filter_map(Result::ok)
            .filter_map(|e| e.metadata().ok())
            .filter(|m| m.is_file())
            .map(|m| m.len())
            .sum()
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_cache(app: AppHandle) -> Result<(), String> {
    let dir = thumbs_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        }
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
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
            let dir = data_dir(app.handle());
            imageio::init(image_bin(app.handle()));
            app.manage(Downloads::default());
            app.manage(AppState { text: Mutex::new(None), cache: Mutex::new(None), prefs: Mutex::new(prefs::Styles::load(&dir)), mediapipe_eyes: Mutex::new(None), face_embedder: Mutex::new(None), last_photos: Mutex::new((String::new(), Vec::new())), scan_job: AtomicU64::new(0), retrain_gen: AtomicU64::new(0), captioner: Mutex::new(None), translator: Mutex::new(None), scene_sentences: Mutex::new(None), scene_job: AtomicU64::new(0), nas: nas::Pool::start() });
            let menu = build_menu(app.handle(), "en")?;
            app.set_menu(menu)?;
            nas_connect_all(app.handle().clone());
            // scratch files (larger previews, zoom crops, motion clips) are kept within bounds; done in the background
            if let Ok(thumbs) = thumbs_dir(app.handle()) {
                std::thread::spawn(move || scratch::prune(&thumbs));
            }
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

#[cfg(test)]
mod organize_tests {
    use super::*;

    #[test]
    fn unique_names_never_overwrite() {
        let dir = std::env::temp_dir().join(format!("fg_unique_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.jpg"), b"1").unwrap();
        std::fs::write(dir.join("a (2).jpg"), b"2").unwrap();
        assert_eq!(unique_path(&dir, std::ffi::OsStr::new("a.jpg")), dir.join("a (3).jpg"));
        assert_eq!(unique_path(&dir, std::ffi::OsStr::new("b.jpg")), dir.join("b.jpg"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trash_accepts_only_photos() {
        let dir = std::env::temp_dir().join(format!("fg_trash_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let photo = dir.join("fg-test-photo.jpg");
        let other = dir.join("notes.txt");
        std::fs::write(&photo, b"not really a jpeg").unwrap();
        std::fs::write(&other, b"keep me").unwrap();
        let r = trash_photos_in(None, vec![photo.to_string_lossy().into(), other.to_string_lossy().into()]);
        println!("moved={} failed={:?}", r.moved, r.failed);
        assert_eq!(r.moved, 1);
        assert_eq!(r.failed.len(), 1);
        assert!(!photo.exists(), "the photo should be in the trash now");
        assert!(other.exists(), "a non-photo must never be touched");
        let _ = std::fs::remove_dir_all(&dir);
    }
}


#[cfg(test)]
mod filter_tests {
    use super::*;

    #[test]
    fn hidden_and_system_names_are_junk() {
        for n in [".DS_Store", "._IMG_0001.jpg", ".Trashes", "__MACOSX", "@eaDir", "Thumbs.db"] {
            assert!(is_junk(n), "{n}");
        }
        for n in ["IMG_0001.jpg", "Trip 2026", "DSCF2586.JPG"] {
            assert!(!is_junk(n), "{n}");
        }
    }

    #[test]
    fn only_readable_photos_are_listed() {
        let dir = std::env::temp_dir().join(format!("fg-filter-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".hidden")).unwrap();
        let img = image::RgbImage::from_fn(400, 300, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x * y) % 256) as u8]));
        img.save(dir.join("good.jpg")).unwrap();
        img.save(dir.join(".hidden").join("inside.jpg")).unwrap();
        img.save(dir.join("._good.jpg")).unwrap();
        std::fs::write(dir.join("fake.jpg"), vec![7u8; 20_000]).unwrap(); // not an image
        std::fs::write(dir.join("tiny.jpg"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"hello").unwrap();
        std::fs::write(dir.join("clip.mov"), vec![0u8; 20_000]).unwrap();
        let found = list_all(&[dir.to_string_lossy().into_owned()]);
        let names: Vec<_> = found.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["good.jpg"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
