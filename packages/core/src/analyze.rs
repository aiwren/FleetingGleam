use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{DynamicImage, GrayImage};
use serde::{Deserialize, Serialize};

use crate::faces::FaceEmbedder;
use crate::mediapipe::MediapipeEyes;
use crate::scoring;
use crate::scorer::AiScorer;
use crate::platform::SystemVision;

pub const ANALYZE_SIZE: u32 = 1024;
const THUMB_SIZE: u32 = 640;

/// Which signal `eyes_open` comes from. The geometric ratio is Apple Vision's own landmarks (macOS only, always
/// available there); MediaPipe is a small downloaded pair of models that read the same crop with a classifier
/// instead of a ratio (see `mediapipe.rs`). Only ever one active at a time for a whole scan.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EyeMethod {
    Vision,
    Mediapipe,
}
impl EyeMethod {
    pub fn parse(s: Option<&str>) -> Self {
        match s { Some("mediapipe") => EyeMethod::Mediapipe, _ => EyeMethod::Vision }
    }
}
/// The `eye_method` tag of a photo measured in MediaPipe mode whose value nevertheless came from Vision's geometric ratio,
/// because the learned model was not sure about any face in it. It is on Vision's scale, and it already is the best MediaPipe
/// mode can do for that photo, so it is not measured again.
pub const MEDIAPIPE_FALLBACK: &str = "mediapipe-fallback";

/// Below this a photo's eyes count as closed: MediaPipe's own openness for a MediaPipe value, Vision's ratio for everything
/// else (including MediaPipe mode's fallback). The two scales are not comparable, so the line must follow the value's origin,
/// not the method selected in Settings.
pub fn eyes_closed_line(method: Option<&str>) -> f32 {
    if method == Some("mediapipe") { scoring::MEDIAPIPE_EYES_CLOSED } else { scoring::EYES_CLOSED }
}

/// True when a cached eye measurement already answers for the method now selected, so the photo need not be measured again.
pub fn eyes_current(method: Option<&str>, selected: EyeMethod) -> bool {
    match selected {
        EyeMethod::Vision => method == Some("vision"),
        EyeMethod::Mediapipe => matches!(method, Some("mediapipe") | Some(MEDIAPIPE_FALLBACK)),
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Photo {
    pub path: String,
    pub name: String,
    pub thumb: String,
    pub width: u32,
    pub height: u32,
    /// Raw energy of the sharpest tiles (see `subject_sharpness`).
    pub sharpness: f32,
    pub brightness: f32,
    pub clipped_high: f32,
    pub clipped_low: f32,
    pub sharp_score: f32,
    pub expo_score: f32,
    pub tech_score: f32,
    /// CLIP + aesthetic head, 0..1.
    pub clip_score: Option<f32>,
    /// Apple's raw overallScore (-1..1), macOS only.
    pub sys_score: Option<f32>,
    /// Blend of the aesthetic signals that were available, 0..1.
    pub ai_score: Option<f32>,
    /// Probability the user would keep this, from the on-device taste model (None when it is off or not yet trained).
    pub pref_score: Option<f32>,
    /// How much the taste model moved the final score (signed, 0 when it was not applied), so the reasons can say so.
    #[serde(default)]
    pub pref_delta: f32,
    pub face_count: u32,
    pub face_min: Option<f32>,
    /// Openness of the most closed pair of eyes among the faces big enough to measure (see scoring::EYES_CLOSED); `None` = not measured.
    #[serde(default)]
    pub eyes_open: Option<f32>,
    /// Which method last computed `eyes_open` ("vision" or "mediapipe"): lets a switch in Settings be noticed on a
    /// photo that is otherwise still a cache hit, without forcing the whole (expensive) analysis to redo.
    #[serde(default)]
    pub eye_method: Option<String>,
    /// What kind of picture it is (see scene.rs); decides which checks weigh most.
    #[serde(default)]
    pub kind: String,
    pub utility: bool,
    pub flags: Vec<String>,
    pub taken_at: Option<i64>,
    pub score: f32,
    pub group: usize,
    pub scene: usize,
    pub group_best: bool,
    /// Faces big enough to recognise, `[x, y, w, h]` top-left origin normalised to the photo (see faces.rs).
    #[serde(default)]
    pub face_boxes: Vec<[f32; 4]>,
    /// Which recurring person each of `face_boxes` is (0 = the person in the most photos), -1 for nobody in particular.
    /// Worked out across the whole project by `faces::assign_people`, so it is never cached.
    #[serde(default, skip_deserializing)]
    pub people: Vec<i32>,
    /// True once the faces were looked at with the recognition model, so a photo analysed before it was downloaded
    /// can be told apart from one that simply has no faces.
    #[serde(default)]
    pub faces_done: bool,
    /// The photo carries an HDR gain map (or is HDR in some other way): shown in HDR in the full-size viewer (thumbnails are always standard range).
    #[serde(default)]
    pub hdr: bool,
    /// A motion photo: a short video follows the picture in the same file.
    #[serde(default)]
    pub motion: bool,
    /// True once `hdr` and `motion` were looked at (photos analysed before they existed are told apart from plain ones).
    #[serde(default)]
    pub media_done: bool,
    /// 显示时顺时针转的角度（0 / 90 / 180 / 270），用户在查看器里转的。只换缩略图、大图和显示尺寸，原文件不动，分析结果也不变。
    #[serde(default)]
    pub rotation: u16,
    #[serde(skip)]
    pub face_embs: Vec<Vec<f32>>,
    #[serde(skip)]
    pub hash: u64,
    #[serde(skip)]
    pub embedding: Vec<f32>,
}

/// Eye openness for a photo whose cached value is missing or was computed by the other method: asks Vision about the
/// saved analysis copy, then (for `Mediapipe`) hands Vision's face crops to the learned model instead of trusting
/// Vision's own ratio.
pub fn measure_eyes(path: &Path, cache_dir: &Path, vision: &dyn SystemVision, method: EyeMethod, mp: Option<&MediapipeEyes>) -> Option<(f32, &'static str)> {
    let copy = cache_name(path, ".a.jpg", cache_dir);
    let copy = if copy.exists() { copy } else { return None };
    let v = vision.analyze(&copy)?;
    let ratio = v.eyes.iter().cloned().reduce(f32::min).unwrap_or(1.0).min(1.0);
    match method {
        EyeMethod::Vision => Some((ratio, "vision")),
        EyeMethod::Mediapipe if v.eyes_mediapipe => Some((ratio, "mediapipe")),
        EyeMethod::Mediapipe => {
            let img = image::open(&copy).ok()?;
            // the value's tag says which scale it is on: the learned model's, or (when it was not sure) Vision's ratio
            Some(match mp.and_then(|m| m.openness(&img, &v.eye_boxes)) {
                Some(e) => (e, "mediapipe"),
                None => (ratio, MEDIAPIPE_FALLBACK),
            })
        }
    }
}

/// Faces of a photo analysed before the recognition model was there: looked at again on its saved analysis copy.
pub fn measure_faces(path: &Path, cache_dir: &Path, vision: &dyn SystemVision, faces: &FaceEmbedder) -> Option<(Vec<[f32; 4]>, Vec<Vec<f32>>)> {
    let copy = cache_name(path, ".a.jpg", cache_dir);
    if !copy.exists() {
        return None;
    }
    let marks = vision.analyze(&copy)?.face_marks;
    let img = image::open(&copy).ok()?;
    let found = faces.embed(&img, &marks);
    Some((found.iter().map(|f| f.bbox).collect(), found.into_iter().map(|f| f.emb).collect()))
}

/// A photo this machine can open: JPEG / PNG / WebP everywhere; on macOS also HEIC, AVIF, JPEG XL, TIFF and camera RAW (see imageio.rs).
pub fn is_image(path: &Path) -> bool {
    crate::imageio::is_supported(path)
}

pub fn analyze(
    path: &Path,
    cache_dir: &Path,
    scorer: Option<&dyn AiScorer>,
    vision: Option<&dyn SystemVision>,
    eye_method: EyeMethod,
    mp: Option<&MediapipeEyes>,
    faces: Option<&FaceEmbedder>,
    on_stage: &dyn Fn(&str, u8, Option<(&str, u32, u32)>),
) -> Result<Photo, String> {
    on_stage("decode", 8, None);
    let (work, width, height) = {
        // a very large photo is hundreds of megabytes while it is decoded: only a few at a time (a burst of them would otherwise take all the memory)
        let _huge = huge_photo_permit(path);
        let (full, width, height) = load_with_size(path, ANALYZE_SIZE)?;
        (fast_resize(&full, ANALYZE_SIZE), width, height)
    };

    // The thumbnail exists from here on, so the UI can show the photo while it is still being analysed.
    let thumb = save_thumb(&work, path, cache_dir)?;
    on_stage("quality", 30, Some((&thumb.to_string_lossy(), width, height)));
    let gray = work.to_luma8();
    let sharpness = subject_sharpness(&gray);
    let (brightness, clipped_high, clipped_low) = exposure_stats(&gray);
    let sharp_score = scoring::sharp_score(sharpness);
    let expo_score = scoring::expo_score(brightness, clipped_high, clipped_low);
    let hash = dhash(&work);

    on_stage("ai", 50, None);
    let ai = scorer.and_then(|s| s.analyze(&work).map_err(|e| eprintln!("ai: {e}")).ok());
    let clip_score = ai.as_ref().map(|a| a.score);
    let embedding = ai.map(|a| a.embedding).unwrap_or_default();

    on_stage("system", 70, None);
    // Vision gets the small, already-upright copy: no need to decode the full-size original twice.
    let sys = vision.and_then(|v| save_analysis_copy(&work, path, cache_dir).and_then(|p| v.analyze(&p)));
    let utility = sys.as_ref().map_or(false, |s| s.utility);
    let sys_score = sys.as_ref().and_then(|s| s.aesthetic);
    let face_min = sys.as_ref().and_then(|s| s.faces.iter().cloned().reduce(f32::min));
    let face_count = sys.as_ref().map_or(0, |s| s.faces.len() as u32);
    // MediaPipe reads the same crops Vision found, in the working copy already in memory; Vision's own ratio is the
    // fallback whenever a face is there but the learned model is not (not downloaded, or not confident about that face).
    // The tag records which scale the value is on, because the two are judged against different lines.
    let (eyes_open, eye_method_used) = match (eye_method, &sys) {
        // 人脸流水线（Android）已经按 MediaPipe 量表量过了：直接用，不再对每张脸做第二遍网格
        (EyeMethod::Mediapipe, Some(s)) if s.eyes_mediapipe => (Some(s.eyes.iter().cloned().reduce(f32::min).unwrap_or(1.0)), Some("mediapipe")),
        (EyeMethod::Mediapipe, Some(s)) => match mp.and_then(|m| m.openness(&work, &s.eye_boxes)) {
            Some(e) => (Some(e), Some("mediapipe")),
            None => (Some(s.eyes.iter().cloned().reduce(f32::min).unwrap_or(1.0)), Some(MEDIAPIPE_FALLBACK)),
        },
        // a face with no eye that could be located is remembered as "very open" (the same as `measure_eyes` does), so it is not measured again
        (_, Some(s)) => (Some(s.eyes.iter().cloned().reduce(f32::min).unwrap_or(1.0)), Some("vision")),
        (_, None) => (None, None),
    };
    let eye_method_used = eyes_open.and(eye_method_used).map(str::to_string);
    // Who is in the picture: only looked at when the recognition model is there and Vision could find the faces.
    let (face_boxes, face_embs, faces_done) = match (faces, &sys) {
        (Some(f), Some(s)) => {
            let found = f.embed(&work, &s.face_marks);
            (found.iter().map(|x| x.bbox).collect(), found.into_iter().map(|x| x.emb).collect(), true)
        }
        _ => (Vec::new(), Vec::new(), false),
    };

    let ai_score = scoring::combine_aesthetic(clip_score, sys_score);
    let flags = scoring::flags(sharp_score, brightness, clipped_high, utility);
    let tech_score = 0.65 * sharp_score + 0.35 * expo_score;
    let score = scoring::final_score("", ai_score, sharp_score, expo_score, None, &flags);

    on_stage("thumb", 92, None);
    let media = crate::media::probe(path, cache_dir);
    Ok(Photo {
        path: path.to_string_lossy().into_owned(),
        name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        thumb: thumb.to_string_lossy().into_owned(),
        width,
        height,
        sharpness,
        brightness,
        clipped_high,
        clipped_low,
        sharp_score,
        expo_score,
        tech_score,
        clip_score,
        sys_score,
        ai_score,
        pref_score: None,
        pref_delta: 0.0,
        face_count,
        face_min,
        eyes_open,
        eye_method: eye_method_used,
        kind: String::new(),
        utility,
        flags,
        taken_at: exif_time(path),
        score,
        group: 0,
        scene: 0,
        group_best: false,
        face_boxes,
        people: Vec::new(),
        faces_done,
        hdr: media.hdr,
        motion: media.motion,
        media_done: true,
        rotation: 0,
        face_embs,
        hash,
        embedding,
    })
}

/// A quick preview thumbnail (used before analysis, so the confirmation screen never has to decode originals in the UI).
pub fn quick_thumb(src: &Path, out: &Path) -> Result<(), String> {
    let (full, _, _) = load_with_size(src, ANALYZE_SIZE)?;
    let work = fast_resize(&full, ANALYZE_SIZE);
    work.thumbnail(THUMB_SIZE, THUMB_SIZE).to_rgb8().save(out).map_err(|e| e.to_string())
}

/// A photo decoded upright and reduced to the working size: what the CLIP model is fed.
pub fn load_for_embedding(src: &Path) -> Result<DynamicImage, String> {
    Ok(fast_resize(&load_with_size(src, ANALYZE_SIZE)?.0, ANALYZE_SIZE))
}

/// A large (long edge 2560) upright JPEG for the full-size viewer: far cheaper for the webview to decode than a 20 MP original.
pub fn large_preview(src: &Path, out: &Path) -> Result<(), String> {
    // The system decoder first: it scales while decoding (several times faster than decoding in full) and keeps the photo's colour profile,
    // so wide-gamut photos are not drained of colour. RAW and the like only go this way.
    if crate::imageio::available() && crate::imageio::render(src, out, if crate::imageio::is_native(src) { 2560 } else { 30000 }, true).is_ok() {
        return Ok(());
    }
    if !crate::imageio::is_native(src) {
        return Err(format!("{}: cannot be read on this machine", src.display()));
    }
    let full = load_oriented(src)?;
    let img = fast_resize(&full, 2560);
    let file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(file), 88).encode_image(&img.to_rgb8()).map_err(|e| e.to_string())
}

/// How many photos of 100 MP or more may be decoded at once: each takes about 600 MB of pixels while it is.
const HUGE_AT_ONCE: usize = 3;
/// A file this large is such a photo (a 200 MP JPEG is 100 MB and more).
const HUGE_FILE: u64 = 60 * 1024 * 1024;

static HUGE_BUSY: (std::sync::Mutex<usize>, std::sync::Condvar) = (std::sync::Mutex::new(0), std::sync::Condvar::new());

/// Held while a huge photo is decoded; `None` for an ordinary one.
struct HugePermit(bool);

impl Drop for HugePermit {
    fn drop(&mut self) {
        if self.0 {
            *HUGE_BUSY.0.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
            HUGE_BUSY.1.notify_one();
        }
    }
}

fn huge_photo_permit(path: &Path) -> HugePermit {
    if std::fs::metadata(path).map_or(0, |m| m.len()) < HUGE_FILE {
        return HugePermit(false);
    }
    let mut busy = HUGE_BUSY.0.lock().unwrap_or_else(|e| e.into_inner());
    while *busy >= HUGE_AT_ONCE {
        busy = HUGE_BUSY.1.wait(busy).unwrap_or_else(|e| e.into_inner());
    }
    *busy += 1;
    HugePermit(true)
}

/// The photo upright, no larger than about `max` on its long side where the decoder can do that cheaply, and its true displayed size.
/// JPEG / PNG / WebP are decoded in full by the `image` crate; everything else (HEIC, RAW...) by the system decoder, at `max`.
fn load_with_size(path: &Path, max: u32) -> Result<(DynamicImage, u32, u32), String> {
    // 系统解码器愿意接原生格式时（Android）就交给它：按需缩放解码，不整张解出来。
    // 别的平台（Mac 的 ImageIO 边车）只在照片特别大（超过 4000 万像素）时才请它缩着解：整张解出来要几百 MB、好几秒。
    let huge = || crate::files::oriented_dims(path).map_or(false, |(w, h)| (w as u64) * (h as u64) > 40_000_000);
    if crate::imageio::is_native(path) && !crate::platform::images().map_or(false, |s| s.prefers_native() || huge()) {
        let full = load_oriented(path)?;
        let (w, h) = (full.width(), full.height());
        return Ok((full, w, h));
    }
    let info = crate::imageio::info(path, false).ok_or_else(|| format!("{}: cannot be read on this machine", path.display()))?;
    let tmp = crate::platform::scratch_dir().join(format!("fg-{:016x}.jpg", { let mut h = DefaultHasher::new(); path.hash(&mut h); std::thread::current().id().hash(&mut h); h.finish() }));
    crate::imageio::render(path, &tmp, max, true)?;
    let img = image::open(&tmp).map_err(|e| format!("{}: {e}", path.display()));
    let _ = std::fs::remove_file(&tmp);
    Ok((img?, info.w, info.h))
}

/// Decodes and applies the EXIF orientation, so portrait shots are upright and sizes are the displayed ones.
pub fn load_oriented(path: &Path) -> Result<DynamicImage, String> {
    use image::ImageDecoder;
    let err = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let reader = image::ImageReader::open(path).map_err(|e| err(&e))?.with_guessed_format().map_err(|e| err(&e))?;
    let mut decoder = reader.into_decoder().map_err(|e| err(&e))?;
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| err(&e))?;
    img.apply_orientation(orientation);
    Ok(img)
}

/// SIMD box/bilinear downscale; several times faster than `DynamicImage::resize` on large photos.
pub fn fast_resize(img: &DynamicImage, max: u32) -> DynamicImage {
    use fast_image_resize as fir;
    let (w, h) = (img.width(), img.height());
    if w <= max && h <= max {
        return img.clone();
    }
    let scale = max as f32 / w.max(h) as f32;
    let (nw, nh) = (((w as f32 * scale).round() as u32).max(1), ((h as f32 * scale).round() as u32).max(1));
    let rgb = img.to_rgb8();
    let done = (|| {
        let src = fir::images::Image::from_vec_u8(w, h, rgb.into_raw(), fir::PixelType::U8x3).ok()?;
        let mut dst = fir::images::Image::new(nw, nh, fir::PixelType::U8x3);
        let opts = fir::ResizeOptions::new().resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::Bilinear));
        fir::Resizer::new().resize(&src, &mut dst, &opts).ok()?;
        image::RgbImage::from_raw(nw, nh, dst.into_vec()).map(DynamicImage::ImageRgb8)
    })();
    done.unwrap_or_else(|| img.resize(max, max, FilterType::Triangle))
}

pub fn cache_name(src: &Path, suffix: &str, cache_dir: &Path) -> PathBuf {
    let mut h = DefaultHasher::new();
    src.hash(&mut h);
    cache_dir.join(format!("{:016x}{suffix}", h.finish()))
}

pub fn save_analysis_copy(img: &DynamicImage, src: &Path, cache_dir: &Path) -> Option<PathBuf> {
    let out = cache_name(src, ".a.jpg", cache_dir);
    let file = std::fs::File::create(&out).ok()?;
    image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(file), 90)
        .encode_image(&img.to_rgb8())
        .ok()?;
    Some(out)
}

pub fn save_thumb(img: &DynamicImage, src: &Path, cache_dir: &Path) -> Result<PathBuf, String> {
    let out = cache_name(src, ".jpg", cache_dir);
    img.thumbnail(THUMB_SIZE, THUMB_SIZE).to_rgb8().save(&out).map_err(|e| e.to_string())?;
    Ok(out)
}

// ---------------------------------------------------------------- 旋转

/// 把任意角度收成 0 / 90 / 180 / 270（顺时针）。
pub fn norm_rotation(deg: i64) -> u16 {
    (((deg % 360) + 360) % 360 / 90 * 90) as u16
}

/// 一张图顺时针转 `deg`。
pub fn rotate_image(img: DynamicImage, deg: u16) -> DynamicImage {
    match deg {
        90 => img.rotate90(),
        180 => img.rotate180(),
        270 => img.rotate270(),
        _ => img,
    }
}

/// 把一个 JPEG 文件转 `deg` 后写到另一处（先写临时文件再改名，读的一方不会看到半张）。
pub fn rotate_jpeg(from: &Path, to: &Path, deg: u16, quality: u8) -> Result<(), String> {
    let img = image::open(from).map_err(|e| format!("{}: {e}", from.display()))?;
    let tmp = to.with_extension("tmp");
    let file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(file), quality).encode_image(&rotate_image(img, deg).to_rgb8()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, to).map_err(|e| e.to_string())
}

/// 转过的缩略图（`.r90.jpg` 之类）：从原缩略图转出来，原缩略图不在了就先做一张。0 度就是原缩略图本身。
pub fn rotated_thumb(src: &Path, deg: u16, cache_dir: &Path) -> Result<PathBuf, String> {
    let base = cache_name(src, ".jpg", cache_dir);
    if !base.exists() {
        quick_thumb(src, &base)?;
    }
    if deg == 0 {
        return Ok(base);
    }
    let out = cache_name(src, &format!(".r{deg}.jpg"), cache_dir);
    if !out.exists() {
        rotate_jpeg(&base, &out, deg, 90)?;
    }
    Ok(out)
}

/// 给一条照片记录套上旋转：角度、缩略图、显示尺寸（90 / 270 时宽高对调）。先还原记录里原来的角度再套新的，所以可以反复调用。
pub fn apply_rotation(p: &mut Photo, deg: u16, cache_dir: &Path) -> Result<(), String> {
    let (ow, oh) = if p.rotation % 180 == 90 { (p.height, p.width) } else { (p.width, p.height) };
    let thumb = rotated_thumb(Path::new(&p.path), deg, cache_dir)?;
    p.thumb = thumb.to_string_lossy().into_owned();
    p.rotation = deg;
    let (w, h) = if deg % 180 == 90 { (oh, ow) } else { (ow, oh) };
    p.width = w;
    p.height = h;
    Ok(())
}

/// Focus measured where the photo is sharpest, not everywhere: the mean Laplacian energy of the
/// sharpest ~12% of an 8x8 tile grid, after a light blur to suppress sensor noise. A shallow-depth
/// portrait keeps a sharp subject and scores well; camera shake softens every tile and scores badly.
pub fn subject_sharpness(g: &GrayImage) -> f32 {
    let g = image::imageops::blur(g, 0.8);
    let (w, h) = g.dimensions();
    if w < 24 || h < 24 {
        return 0.0;
    }
    let px = |x: u32, y: u32| g.get_pixel(x, y)[0] as f32;
    const N: u32 = 8;
    let (tw, th) = (w / N, h / N);
    let mut energies = Vec::with_capacity((N * N) as usize);
    for ty in 0..N {
        for tx in 0..N {
            let (x0, y0) = (tx * tw + 1, ty * th + 1);
            let (x1, y1) = (((tx + 1) * tw).min(w - 1), ((ty + 1) * th).min(h - 1));
            let (mut sum, mut sq, mut n) = (0.0f64, 0.0f64, 0u32);
            for y in y0..y1 {
                for x in x0..x1 {
                    let l = px(x - 1, y) + px(x + 1, y) + px(x, y - 1) + px(x, y + 1) - 4.0 * px(x, y);
                    sum += l as f64;
                    sq += (l * l) as f64;
                    n += 1;
                }
            }
            if n > 0 {
                let mean = sum / n as f64;
                energies.push((sq / n as f64 - mean * mean) as f32);
            }
        }
    }
    energies.sort_by(|a, b| b.total_cmp(a));
    let k = (energies.len() / 8).max(1);
    energies[..k].iter().sum::<f32>() / k as f32
}

/// Returns (mean brightness 0..1, fraction near white, fraction near black).
pub fn exposure_stats(g: &GrayImage) -> (f32, f32, f32) {
    let n = (g.width() * g.height()) as f32;
    let (mut sum, mut hi, mut lo) = (0u64, 0u32, 0u32);
    for p in g.pixels() {
        let v = p[0];
        sum += v as u64;
        hi += (v >= 250) as u32;
        lo += (v <= 5) as u32;
    }
    (sum as f32 / n / 255.0, hi as f32 / n, lo as f32 / n)
}

pub fn dhash(img: &DynamicImage) -> u64 {
    let g = img.resize_exact(9, 8, FilterType::Triangle).to_luma8();
    let mut bits = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            bits <<= 1;
            bits |= (g.get_pixel(x, y)[0] > g.get_pixel(x + 1, y)[0]) as u64;
        }
    }
    bits
}

/// EXIF DateTimeOriginal as naive seconds (no timezone; only differences are used).
pub fn exif_time(path: &Path) -> Option<i64> {
    let file = std::fs::File::open(path).ok()?;
    let ex = exif::Reader::new().read_from_container(&mut std::io::BufReader::new(file)).ok()?;
    let f = ex
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .or_else(|| ex.get_field(exif::Tag::DateTime, exif::In::PRIMARY))?;
    let s = f.display_value().to_string(); // "2026-09-04 18:22:31"
    let n: Vec<i64> = s.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).filter_map(|p| p.parse().ok()).collect();
    if n.len() < 6 {
        return None;
    }
    let (y, m, d) = (n[0], n[1], n[2]);
    let (y2, m2) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * m2 + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe;
    Some(days * 86400 + n[3] * 3600 + n[4] * 60 + n[5])
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Greedy grouping in shooting order. A photo joins a group only if it matches *every* member (complete linkage),
/// so chains of "A looks like B, B looks like C" can never glue unrelated shots together, and a group is capped in size.
fn cluster(photos: &[Photo], same: impl Fn(&Photo, &Photo) -> bool, max_size: usize) -> Vec<usize> {
    let n = photos.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (photos[i].taken_at.unwrap_or(i64::MAX), i));
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut of = vec![0usize; n];
    for &i in &order {
        // Only recent groups are candidates: near-duplicates are shot back to back, and this keeps it linear.
        let hit = groups.iter().enumerate().rev().take(48).find(|(_, g)| g.len() < max_size && g.iter().all(|&m| same(&photos[i], &photos[m]))).map(|(k, _)| k);
        match hit {
            Some(k) => { groups[k].push(i); of[i] = k; }
            None => { of[i] = groups.len(); groups.push(vec![i]); }
        }
    }
    // Number groups by first appearance in the original order, so ids are stable and readable.
    let mut ids = HashMap::new();
    (0..n).map(|i| { let next = ids.len(); *ids.entry(of[i]).or_insert(next) }).collect()
}

/// Near-duplicate distance / similarity thresholds.
const DUP_HASH_DIST: u32 = 6;
/// With a strong CLIP match the layout may differ a little (a step, a turn of the head), but not more than this many of 64 bits.
const DUP_HASH_LOOSE: u32 = 18;
const DUP_COS: f32 = 0.94;
/// A near-identical hash still needs a broadly similar look, so flat, bright frames of different scenes do not match.
const DUP_HASH_MIN_COS: f32 = 0.88;
/// One "same moment" group never grows beyond this many frames.
const DUP_MAX_SIZE: usize = 12;
const SCENE_MAX_SIZE: usize = 80;
/// Shots this many seconds apart are never the "same moment", however alike they look.
const DUP_MAX_GAP_SECS: i64 = 3 * 60;
/// A photo joins a scene when, on average, it looks like the scene's photos this much (CLIP cosine). Averaging (rather than
/// demanding a match with every member) keeps one place together across poses and angles: the same lawn photographed from five
/// spots is one scene, even though its first and last frames are quite unlike each other.
const SCENE_COS: f32 = 0.82;
/// A scene is one stretch of the shoot: a gap this long since its latest photo starts a new one.
const SCENE_GAP_SECS: i64 = 30 * 60;
/// Only the most recent scenes are candidates, which keeps this linear in the number of photos.
const SCENE_LOOKBACK: usize = 64;

/// Scenes: photos of one place and moment, in time order. See `SCENE_COS`.
fn cluster_scenes(photos: &[Photo]) -> Vec<usize> {
    let n = photos.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (photos[i].taken_at.unwrap_or(i64::MAX), i));
    struct Scene { size: usize, sum: Vec<f32>, last: Option<i64> }
    let mut scenes: Vec<Scene> = Vec::new();
    let mut of = vec![0usize; n];
    for &i in &order {
        let p = &photos[i];
        let hit = if p.embedding.is_empty() {
            None
        } else {
            let mut best: Option<(usize, f32)> = None;
            for k in (0..scenes.len()).rev().take(SCENE_LOOKBACK) {
                let s = &scenes[k];
                if s.size >= SCENE_MAX_SIZE || s.sum.len() != p.embedding.len() { continue; }
                if let (Some(a), Some(b)) = (p.taken_at, s.last) {
                    if a - b > SCENE_GAP_SECS { continue; }
                }
                let sim = cosine(&p.embedding, &s.sum) / s.size as f32;
                if sim >= SCENE_COS && best.map_or(true, |(_, b)| sim > b) { best = Some((k, sim)); }
            }
            best.map(|(k, _)| k)
        };
        match hit {
            Some(k) => {
                let s = &mut scenes[k];
                s.size += 1;
                s.sum.iter_mut().zip(&p.embedding).for_each(|(a, b)| *a += b);
                if p.taken_at.is_some() { s.last = p.taken_at; }
                of[i] = k;
            }
            None => {
                of[i] = scenes.len();
                scenes.push(Scene { size: 1, sum: p.embedding.clone(), last: p.taken_at });
            }
        }
    }
    // Number scenes by first appearance in the original order, as `cluster` does.
    let mut ids = HashMap::new();
    (0..n).map(|i| { let next = ids.len(); *ids.entry(of[i]).or_insert(next) }).collect()
}

/// -1 portrait, 0 square-ish, 1 landscape (as displayed, EXIF orientation applied).
fn orientation(p: &Photo) -> i8 {
    let r = p.width as f32 / p.height.max(1) as f32;
    if r > 1.05 { 1 } else if r < 0.95 { -1 } else { 0 }
}

/// Groups near-duplicates, clusters looser scenes, then computes final scores (which need the
/// group context for face comparison) and marks the best photo of every group.
pub fn group_and_score(photos: &mut [Photo], prefs: Option<&crate::prefs::Prefs>) {
    let groups = cluster(photos, |a, b| {
        if let (Some(x), Some(y)) = (a.taken_at, b.taken_at) {
            if (x - y).abs() > DUP_MAX_GAP_SECS {
                return false;
            }
        }
        // A portrait and a landscape frame are different pictures, however alike the scene: they are never "the same shot".
        if orientation(a) * orientation(b) < 0 {
            return false;
        }
        // People walking in or out of frame between two exposures.
        if (a.face_count as i64 - b.face_count as i64).abs() >= 2 {
            return false;
        }
        let dist = (a.hash ^ b.hash).count_ones();
        let cos = if a.embedding.is_empty() || b.embedding.is_empty() { None } else { Some(cosine(&a.embedding, &b.embedding)) };
        match cos {
            // Same look (CLIP) and a similar layout (dHash): the layout check is what separates "same pose" from "same backdrop".
            Some(c) => (c >= DUP_COS && dist <= DUP_HASH_LOOSE) || (dist <= DUP_HASH_DIST && c >= DUP_HASH_MIN_COS),
            None => dist <= DUP_HASH_DIST,
        }
    }, DUP_MAX_SIZE);
    let scenes = cluster_scenes(photos);

    // Face quality is small in absolute terms but meaningful *between frames of the same moment*.
    let mut best_face: HashMap<usize, f32> = HashMap::new();
    for (p, &g) in photos.iter().zip(&groups) {
        if let Some(f) = p.face_min {
            let e = best_face.entry(g).or_insert(0.0);
            *e = e.max(f);
        }
    }
    let mut group_size: HashMap<usize, usize> = HashMap::new();
    groups.iter().for_each(|g| *group_size.entry(*g).or_default() += 1);

    // Closed eyes only count against a shot when the same moment has another frame with open eyes to pick instead.
    // A lone frame, or a group where every frame is closed (a sleeping child, a deliberate pose), is not marked down.
    // Each photo's value is judged on its own scale (see `eyes_closed_line`): a group can mix MediaPipe values with Vision ratios.
    let mut open_alt: std::collections::HashMap<usize, bool> = std::collections::HashMap::new();
    for (i, p) in photos.iter().enumerate() {
        if let Some(e) = p.eyes_open {
            if e >= eyes_closed_line(p.eye_method.as_deref()) {
                open_alt.insert(groups[i] as usize, true);
            }
        }
    }
    for (i, p) in photos.iter_mut().enumerate() {
        p.group = groups[i];
        p.scene = scenes[i];
        let compare = group_size[&groups[i]] > 1;
        let face_rel = match (p.face_min, best_face.get(&groups[i])) {
            (Some(f), Some(&m)) if compare && m > 0.0 => Some(f / m),
            _ => None,
        };
        // What kind of picture this is decides which checks weigh most (worked out from the embedding, so it needs no re-analysis).
        let kind = crate::scene::classify(&p.embedding).map(|(k, _)| crate::scene::with_faces(k, p.face_count)).unwrap_or(if p.face_count > 0 { "people" } else { "" });
        p.kind = kind.to_string();
        p.flags.retain(|f| f != "eyes");
        let closed_at = eyes_closed_line(p.eye_method.as_deref());
        if matches!(kind, "portrait" | "people") && p.eyes_open.map_or(false, |e| e < closed_at)
            && group_size[&groups[i]] > 1 && open_alt.get(&(groups[i] as usize)).copied().unwrap_or(false) {
            p.flags.push("eyes".to_string());
        }
        p.score = scoring::final_score(kind, p.ai_score, p.sharp_score, p.expo_score, face_rel, &p.flags);
        // The learned taste nudges the score; hard problems (blur, screenshots) still cap it via the flags above.
        p.pref_score = prefs.and_then(|m| m.predict(&p.embedding));
        p.pref_delta = 0.0;
        if let (Some(pref), Some(m)) = (p.pref_score, prefs) {
            let lam = m.strength();
            let before = p.score;
            let blended = ((1.0 - lam) * before + lam * pref).clamp(0.0, 1.0);
            // Taste can lift a photo, but never rescue one with a hard problem (blur, screenshot).
            let hard = p.flags.iter().any(|f| f == "blur" || f == "utility");
            p.score = if hard { blended.min(before) } else { blended };
            p.pref_delta = p.score - before;
        }
    }

    let mut best: HashMap<usize, usize> = HashMap::new();
    for (i, p) in photos.iter().enumerate() {
        let e = best.entry(p.group).or_insert(i);
        if p.score > photos[*e].score {
            *e = i;
        }
    }
    for i in best.into_values() {
        photos[i].group_best = true;
    }
    crate::faces::assign_people(photos);
}

#[cfg(test)]
mod eye_scale_tests {
    use super::*;

    /// A value is judged on the scale it was measured on, whatever method is selected now; and a MediaPipe-mode fallback
    /// value counts as current for MediaPipe, so it is not measured again on every open.
    #[test]
    fn eye_values_keep_their_own_scale() {
        assert_eq!(eyes_closed_line(Some("mediapipe")), scoring::MEDIAPIPE_EYES_CLOSED);
        assert_eq!(eyes_closed_line(Some(MEDIAPIPE_FALLBACK)), scoring::EYES_CLOSED);
        assert_eq!(eyes_closed_line(Some("vision")), scoring::EYES_CLOSED);
        assert_eq!(eyes_closed_line(None), scoring::EYES_CLOSED);
        // 0.3 is wide open on Vision's ratio and must not read as closed because MediaPipe mode is selected
        assert!(0.3 >= eyes_closed_line(Some(MEDIAPIPE_FALLBACK)));
        assert!(0.3 < eyes_closed_line(Some("mediapipe")));
        assert!(eyes_current(Some("mediapipe"), EyeMethod::Mediapipe));
        assert!(eyes_current(Some(MEDIAPIPE_FALLBACK), EyeMethod::Mediapipe));
        assert!(!eyes_current(Some("vision"), EyeMethod::Mediapipe));
        assert!(eyes_current(Some("vision"), EyeMethod::Vision));
        assert!(!eyes_current(Some(MEDIAPIPE_FALLBACK), EyeMethod::Vision));
        assert!(!eyes_current(None, EyeMethod::Vision));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the whole grouping and scoring on the photos of a real analysis cache and prints what the scene kinds and eyes did.
    #[test]
    #[ignore = "needs a real analysis cache (FG_CACHE)"]
    fn scoring_on_real_cache() {
        let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(std::env::var("FG_CACHE").expect("FG_CACHE")).unwrap()).unwrap();
        let mut photos: Vec<Photo> = raw["entries"].as_object().unwrap().values().map(|e| {
            let mut p: Photo = serde_json::from_value(e["photo"].clone()).unwrap();
            p.embedding = serde_json::from_value(e["emb"].clone()).unwrap();
            p
        }).collect();
        photos.sort_by(|a, b| a.path.cmp(&b.path));
        group_and_score(&mut photos, None);
        let mut by: std::collections::BTreeMap<String, usize> = Default::default();
        photos.iter().for_each(|p| *by.entry(p.kind.clone()).or_default() += 1);
        println!("kinds: {by:?}");
        println!("closed eyes flagged: {}", photos.iter().filter(|p| p.flags.iter().any(|f| f == "eyes")).count());
        let groups = photos.iter().map(|p| p.group).collect::<std::collections::HashSet<_>>().len();
        let scenes = photos.iter().map(|p| p.scene).collect::<std::collections::HashSet<_>>().len();
        println!("{} photos, {groups} groups, {scenes} scenes, {} group bests", photos.len(), photos.iter().filter(|p| p.group_best).count());
        for kind in ["portrait", "people", "landscape", "object"] {
            let mut v: Vec<&Photo> = photos.iter().filter(|p| p.kind == kind).collect();
            v.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
            println!("{kind}: best {:?}", v.iter().take(3).map(|p| format!("{}:{:.2}", p.name, p.score)).collect::<Vec<_>>());
        }
    }

    /// Prints sharpness for a real photo and progressively blurred copies, to calibrate scoring.rs.
    #[test]
    #[ignore = "needs PROBE_IMG"]
    fn calibrate_sharpness() {
        let Ok(p) = std::env::var("PROBE_IMG") else { return };
        let work = image::open(p).unwrap().resize(ANALYZE_SIZE, ANALYZE_SIZE, FilterType::Triangle);
        for sigma in [0.0f32, 0.8, 1.5, 2.5, 4.0, 8.0] {
            let img = if sigma == 0.0 { work.clone() } else { DynamicImage::ImageRgb8(image::imageops::blur(&work.to_rgb8(), sigma)) };
            let raw = subject_sharpness(&img.to_luma8());
            println!("sigma={sigma:>4}  raw={raw:>8.1}  score={:.2}", scoring::sharp_score(raw));
        }
    }

    /// The CPU side of analysing one photo, step by step (no models): FG_PHOTO=<a jpeg>; run with --ignored --nocapture.
    #[test]
    #[ignore = "timing only: FG_PHOTO=<a photo>; run with --ignored --nocapture"]
    fn where_one_photo_goes() {
        use std::time::Instant;
        let Ok(p) = std::env::var("FG_PHOTO") else { return };
        let path = Path::new(&p);
        let tmp = std::env::temp_dir().join("fg_onephoto");
        std::fs::create_dir_all(&tmp).unwrap();
        let rows = std::cell::RefCell::new(Vec::<(&str, f64)>::new());
        let lap = |name: &'static str, t: Instant| rows.borrow_mut().push((name, t.elapsed().as_secs_f64() * 1000.0));
        for pass in 0..2 {
            rows.borrow_mut().clear();
            let t = Instant::now();
            let full = load_oriented(path).unwrap();
            lap("decode (image crate)", t);
            let t = Instant::now();
            let work = fast_resize(&full, ANALYZE_SIZE);
            lap("resize to 1024", t);
            drop(full);
            let t = Instant::now();
            let _thumb = save_thumb(&work, path, &tmp).unwrap();
            lap("thumbnail 640 (resize + jpeg + write)", t);
            let t = Instant::now();
            let gray = work.to_luma8();
            lap("grayscale", t);
            let t = Instant::now();
            let _ = subject_sharpness(&gray);
            lap("sharpness (blur + laplacian tiles)", t);
            let t = Instant::now();
            let _ = exposure_stats(&gray);
            lap("exposure stats", t);
            let t = Instant::now();
            let _ = dhash(&work);
            lap("dhash", t);
            let t = Instant::now();
            let _ = crate::scorer::preprocess(&work);
            lap("CLIP preprocessing (resize 224, normalise)", t);
            let t = Instant::now();
            let _ = save_analysis_copy(&work, path, &tmp);
            lap("analysis copy for Vision (jpeg + write)", t);
            let t = Instant::now();
            let _ = exif_time(path);
            lap("exif time", t);
            if pass == 1 {
                let rows = rows.borrow();
                let total: f64 = rows.iter().map(|r| r.1).sum();
                println!("\n{} ({} MB):", path.file_name().unwrap().to_string_lossy(), std::fs::metadata(path).unwrap().len() / 1_000_000);
                for (n, ms) in rows.iter() { println!("  {n:<42} {ms:>7.1} ms  {:>4.0}%", 100.0 * ms / total); }
                println!("  {:<42} {total:>7.1} ms", "total");
            }
        }
    }

    /// Same photo, degraded in known ways, scored by both model tiers. Shows whether each tier reacts sensibly.
    #[test]
    #[ignore = "needs PROBE_IMG, models/ (both tiers)"]
    fn compare_quality() {
        use crate::scorer::Quality;
        use std::time::Instant;
        let Ok(p) = std::env::var("PROBE_IMG") else { return };
        let base = image::open(p).unwrap().resize(1024, 1024, FilterType::Triangle).to_rgb8();
        let crop = image::imageops::crop_imm(&base, base.width() / 3, base.height() / 3, base.width() / 4, base.height() / 4).to_image();
        let variants: Vec<(&str, DynamicImage)> = vec![
            ("original", DynamicImage::ImageRgb8(base.clone())),
            ("blur light", DynamicImage::ImageRgb8(image::imageops::blur(&base, 1.5))),
            ("blur heavy", DynamicImage::ImageRgb8(image::imageops::blur(&base, 6.0))),
            ("dark", DynamicImage::ImageRgb8(image::imageops::brighten(&base, -90))),
            ("over-exposed", DynamicImage::ImageRgb8(image::imageops::brighten(&base, 110))),
            ("tiny crop", DynamicImage::ImageRgb8(crop)),
            ("flat grey", DynamicImage::ImageRgb8(image::RgbImage::from_pixel(512, 512, image::Rgb([128, 128, 128])))),
        ];
        for q in [Quality::Fast, Quality::High] {
            let load = Instant::now();
            let s = crate::scorer::ClipScorer::load(&crate::dev_models_dir(), true, q).unwrap();
            println!("== {} via {}  (loaded in {:.0} ms)", s.model, s.backend, load.elapsed().as_secs_f64() * 1000.0);
            let _ = s.analyze(&variants[0].1); // warm-up
            for (name, img) in &variants {
                let t = Instant::now();
                match s.analyze(img) {
                    Ok(o) => println!("   {name:<13} score {:.2}   {:.0} ms", o.score, t.elapsed().as_secs_f64() * 1000.0),
                    Err(e) => println!("   {name:<13} rejected: {e}"),
                }
            }
        }
    }

    /// How long grouping and scoring take on a very large project (it runs every time a project is opened): synthetic photos with random CLIP
    /// vectors, shot in runs of look-alikes, and a clock that goes forward.
    #[test]
    #[ignore = "timing only; run with --ignored --nocapture"]
    fn grouping_a_huge_project_is_fast_enough() {
        let mut seed = 99u64;
        let mut rnd = move || { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); ((seed >> 40) as f32) / ((1u64 << 24) as f32) - 0.5 };
        for n in [5_000usize, 20_000] {
            let mut photos: Vec<Photo> = Vec::with_capacity(n);
            let mut base: Vec<f32> = Vec::new();
            for i in 0..n {
                if i % 6 == 0 { base = (0..768).map(|_| rnd()).collect(); }
                let mut e: Vec<f32> = base.iter().map(|b| b + rnd() * 0.15).collect();
                let norm = e.iter().map(|x| x * x).sum::<f32>().sqrt();
                e.iter_mut().for_each(|x| *x /= norm);
                photos.push(Photo { path: format!("p{i}"), width: 4000, height: 3000, embedding: e, hash: (i as u64 / 6).wrapping_mul(0x9e3779b97f4a7c15), taken_at: Some(1_700_000_000 + i as i64 * 20), score: 0.5 + (i % 50) as f32 / 100.0, ..Default::default() });
            }
            let t = std::time::Instant::now();
            group_and_score(&mut photos, None);
            println!("{n} photos: grouping and scoring {:?}", t.elapsed());
        }
    }
}
