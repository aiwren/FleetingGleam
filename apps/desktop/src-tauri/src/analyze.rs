use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{DynamicImage, GrayImage};
use serde::{Deserialize, Serialize};

use crate::scoring;
use crate::scorer::AiScorer;
use crate::vision::Vision;

const ANALYZE_SIZE: u32 = 1024;
const THUMB_SIZE: u32 = 640;
pub const IMAGE_EXTS: [&str; 5] = ["jpg", "jpeg", "png", "webp", "jpe"];

#[derive(Serialize, Deserialize, Clone)]
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
    pub face_count: u32,
    pub face_min: Option<f32>,
    /// Openness of the most closed pair of eyes among the faces big enough to measure (see scoring::EYES_CLOSED); `None` = not measured.
    #[serde(default)]
    pub eyes_open: Option<f32>,
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
    #[serde(skip)]
    pub hash: u64,
    #[serde(skip)]
    pub embedding: Vec<f32>,
}

/// Eye openness for a photo that was analysed before this signal existed: asks the Vision helper about the saved analysis copy.
pub fn measure_eyes(path: &Path, cache_dir: &Path, vision: &Vision) -> Option<f32> {
    let copy = cache_name(path, ".a.jpg", cache_dir);
    let copy = if copy.exists() { copy } else { return None };
    vision.analyze(&copy).map(|v| v.eyes.iter().cloned().reduce(f32::min).unwrap_or(1.0).min(1.0))
}

pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn analyze(
    path: &Path,
    cache_dir: &Path,
    scorer: Option<&dyn AiScorer>,
    vision: Option<&Vision>,
    on_stage: &dyn Fn(&str, u8, Option<(&str, u32, u32)>),
) -> Result<Photo, String> {
    on_stage("decode", 8, None);
    let full = load_oriented(path)?;
    let (width, height) = (full.width(), full.height());
    let work = fast_resize(&full, ANALYZE_SIZE);
    drop(full);

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
    let sys_score = sys.as_ref().map(|s| s.aesthetic);
    let (faces, eyes) = sys.map(|s| (s.faces, s.eyes)).unwrap_or_default();
    let face_min = faces.iter().cloned().reduce(f32::min);
    let eyes_open = eyes.iter().cloned().reduce(f32::min);

    let ai_score = scoring::combine_aesthetic(clip_score, sys_score);
    let flags = scoring::flags(sharp_score, brightness, clipped_high, utility);
    let tech_score = 0.65 * sharp_score + 0.35 * expo_score;
    let score = scoring::final_score("", ai_score, sharp_score, expo_score, None, &flags);

    on_stage("thumb", 92, None);
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
        face_count: faces.len() as u32,
        face_min,
        eyes_open,
        kind: String::new(),
        utility,
        flags,
        taken_at: exif_time(path),
        score,
        group: 0,
        scene: 0,
        group_best: false,
        hash,
        embedding,
    })
}

/// A quick preview thumbnail (used before analysis, so the confirmation screen never has to decode originals in the UI).
pub fn quick_thumb(src: &Path, out: &Path) -> Result<(), String> {
    let full = load_oriented(src)?;
    let work = fast_resize(&full, ANALYZE_SIZE);
    work.thumbnail(THUMB_SIZE, THUMB_SIZE).to_rgb8().save(out).map_err(|e| e.to_string())
}

/// A photo decoded upright and reduced to the working size: what the CLIP model is fed.
pub fn load_for_embedding(src: &Path) -> Result<DynamicImage, String> {
    Ok(fast_resize(&load_oriented(src)?, ANALYZE_SIZE))
}

/// A large (long edge 2560) upright JPEG for the full-size viewer: far cheaper for the webview to decode than a 20 MP original.
pub fn large_preview(src: &Path, out: &Path) -> Result<(), String> {
    let full = load_oriented(src)?;
    let img = fast_resize(&full, 2560);
    let file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(file), 88).encode_image(&img.to_rgb8()).map_err(|e| e.to_string())
}

/// Decodes and applies the EXIF orientation, so portrait shots are upright and sizes are the displayed ones.
fn load_oriented(path: &Path) -> Result<DynamicImage, String> {
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
fn fast_resize(img: &DynamicImage, max: u32) -> DynamicImage {
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

fn save_analysis_copy(img: &DynamicImage, src: &Path, cache_dir: &Path) -> Option<PathBuf> {
    let out = cache_name(src, ".a.jpg", cache_dir);
    let file = std::fs::File::create(&out).ok()?;
    image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(file), 90)
        .encode_image(&img.to_rgb8())
        .ok()?;
    Some(out)
}

fn save_thumb(img: &DynamicImage, src: &Path, cache_dir: &Path) -> Result<PathBuf, String> {
    let out = cache_name(src, ".jpg", cache_dir);
    img.thumbnail(THUMB_SIZE, THUMB_SIZE).to_rgb8().save(&out).map_err(|e| e.to_string())?;
    Ok(out)
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
fn exposure_stats(g: &GrayImage) -> (f32, f32, f32) {
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

fn dhash(img: &DynamicImage) -> u64 {
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
fn exif_time(path: &Path) -> Option<i64> {
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
const SCENE_MAX_SIZE: usize = 60;
/// Shots this many seconds apart are never the "same moment", however alike they look.
const DUP_MAX_GAP_SECS: i64 = 3 * 60;
/// Looser similarity that defines a "scene", used to spread picks across the whole trip.
const SCENE_COS: f32 = 0.86;

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
    let scenes = cluster(photos, |a, b| {
        !a.embedding.is_empty() && !b.embedding.is_empty() && cosine(&a.embedding, &b.embedding) >= SCENE_COS
    }, SCENE_MAX_SIZE);

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
    let mut open_alt: std::collections::HashMap<usize, f32> = std::collections::HashMap::new();
    for (i, p) in photos.iter().enumerate() {
        if let Some(e) = p.eyes_open {
            let m = open_alt.entry(groups[i] as usize).or_insert(0.0);
            *m = m.max(e);
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
        if matches!(kind, "portrait" | "people") && p.eyes_open.map_or(false, |e| e < scoring::EYES_CLOSED)
            && group_size[&groups[i]] > 1 && open_alt.get(&(groups[i] as usize)).map_or(false, |&m| m >= scoring::EYES_CLOSED) {
            p.flags.push("eyes".to_string());
        }
        p.score = scoring::final_score(kind, p.ai_score, p.sharp_score, p.expo_score, face_rel, &p.flags);
        // The learned taste nudges the score; hard problems (blur, screenshots) still cap it via the flags above.
        p.pref_score = prefs.and_then(|m| m.predict(&p.embedding));
        if let (Some(pref), Some(m)) = (p.pref_score, prefs) {
            let lam = m.strength();
            let before = p.score;
            let blended = ((1.0 - lam) * before + lam * pref).clamp(0.0, 1.0);
            // Taste can lift a photo, but never rescue one with a hard problem (blur, screenshot).
            let hard = p.flags.iter().any(|f| f == "blur" || f == "utility");
            p.score = if hard { blended.min(before) } else { blended };
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
        println!("{} photos, {groups} groups, {} group bests", photos.len(), photos.iter().filter(|p| p.group_best).count());
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

    /// End to end: original + degraded copies through the real pipeline (CLIP, Apple helper, grouping).
    #[test]
    #[ignore = "needs PROBE_IMG, TMP_DIR, models/ and bin/fg-vision"]
    fn pipeline_smoke() {
        let (Ok(p), Ok(tmp)) = (std::env::var("PROBE_IMG"), std::env::var("TMP_DIR")) else { return };
        let tmp = PathBuf::from(tmp);
        let base = image::open(p).unwrap().resize(1600, 1600, FilterType::Triangle).to_rgb8();
        let variants: Vec<(&str, image::RgbImage)> = vec![
            ("a_original", base.clone()),
            ("b_blur_light", image::imageops::blur(&base, 1.5)),
            ("c_blur_heavy", image::imageops::blur(&base, 6.0)),
            ("d_dark", image::imageops::brighten(&base, -90)),
            ("e_over", image::imageops::brighten(&base, 110)),
        ];
        let mut paths = Vec::new();
        for (n, im) in &variants {
            let path = tmp.join(format!("{n}.jpg"));
            im.save(&path).unwrap();
            paths.push(path);
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let scorer = crate::scorer::ClipScorer::load(&root.join("../models"), true, crate::scorer::Quality::Fast).ok();
        let vision = Vision::start(&root.join("bin/fg-vision"), 2, false);
        println!("clip={} vision={}", scorer.is_some(), vision.is_some());
        let mut photos: Vec<Photo> = paths
            .iter()
            .map(|p| analyze(p, &tmp, scorer.as_ref().map(|s| s as &dyn AiScorer), vision.as_ref(), &|_, _, _| {}).unwrap())
            .collect();
        group_and_score(&mut photos, None);
        for p in &photos {
            println!(
                "{:<14} score={:.2} sharp={:.2} expo={:.2} clip={:?} sys={:?} faces={} flags={:?} group={} scene={} best={}",
                p.name, p.score, p.sharp_score, p.expo_score,
                p.clip_score.map(|v| (v * 100.0).round() / 100.0), p.sys_score.map(|v| (v * 100.0).round() / 100.0),
                p.face_count, p.flags, p.group, p.scene, p.group_best
            );
        }
    }

    /// Per-stage wall time on one full-size photo, and CLIP on CPU vs the CoreML provider.
    #[test]
    #[ignore = "needs PROBE_IMG, models/ and bin/fg-vision"]
    fn bench_stages() {
        use std::time::Instant;
        let Ok(p) = std::env::var("PROBE_IMG") else { return };
        let ms = |t: Instant| t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let full = load_oriented(Path::new(&p)).unwrap();
        println!("decode {}x{}      {:>7.1} ms", full.width(), full.height(), ms(t));
        let t = Instant::now();
        let work = fast_resize(&full, ANALYZE_SIZE);
        println!("resize to 1024       {:>7.1} ms  -> {}x{}", ms(t), work.width(), work.height());
        let gray = work.to_luma8();
        let t = Instant::now();
        let _ = subject_sharpness(&gray);
        println!("sharpness (tiles)    {:>7.1} ms", ms(t));
        let t = Instant::now();
        let _ = dhash(&work);
        println!("dhash                {:>7.1} ms", ms(t));
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for (label, accel) in [("CLIP  CPU only      ", false), ("CLIP  CoreML provider", true)] {
            let load = Instant::now();
            let s = crate::scorer::ClipScorer::load(&root.join("../models"), accel, crate::scorer::Quality::Fast).unwrap();
            let load_ms = ms(load);
            let _ = s.analyze(&work); // warm-up (CoreML compiles on first run)
            let t = Instant::now();
            for _ in 0..8 { s.analyze(&work).unwrap(); }
            println!("{label} load {:>7.0} ms, per image {:>7.1} ms", load_ms, ms(t) / 8.0);
        }
        if let Some(v) = Vision::start(&root.join("bin/fg-vision"), 1, false) {
            let small = save_analysis_copy(&work, Path::new(&p), &std::env::temp_dir()).unwrap();
            let _ = v.analyze(&small);
            let t = Instant::now();
            for _ in 0..4 { v.analyze(&small); }
            println!("Apple Vision (1024px copy) {:>7.1} ms per image", ms(t) / 4.0);
            let t = Instant::now();
            for _ in 0..2 { v.analyze(Path::new(&p)); }
            println!("Apple Vision (full original) {:>5.1} ms per image", ms(t) / 2.0);
        }
    }

    /// Whole-pipeline throughput on N copies of one full-size photo: CPU int8 vs CoreML fp16 (with and without Apple Vision).
    #[test]
    #[ignore = "needs PROBE_IMG, TMP_DIR, models/ (incl. fp16) and bin/fg-vision"]
    fn pipeline_throughput() {
        use rayon::prelude::*;
        use std::time::Instant;
        let (Ok(p), Ok(tmp)) = (std::env::var("PROBE_IMG"), std::env::var("TMP_DIR")) else { return };
        let tmp = PathBuf::from(tmp);
        let n = 30;
        let paths: Vec<PathBuf> = (0..n)
            .map(|i| {
                let d = tmp.join(format!("tp_{i}.jpg"));
                if !d.exists() {
                    std::fs::copy(&p, &d).unwrap();
                }
                d
            })
            .collect();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for with_vision in [false, true] {
            for accel in [false, true] {
                let scorer = crate::scorer::ClipScorer::load(&root.join("../models"), accel, crate::scorer::Quality::Fast).unwrap();
                let vision = if with_vision { Vision::start(&root.join("bin/fg-vision"), 2, false) } else { None };
                let t = Instant::now();
                let done: usize = paths
                    .par_iter()
                    .map(|q| analyze(q, &tmp, Some(&scorer as &dyn AiScorer), vision.as_ref(), &|_, _, _| {}).is_ok() as usize)
                    .sum();
                let secs = t.elapsed().as_secs_f64();
                println!("vision={:<5} clip={:<12} {done} photos in {secs:>5.1}s  ({:.0} ms/photo)", with_vision, scorer.backend, secs * 1000.0 / done as f64);
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
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for q in [Quality::Fast, Quality::High] {
            let load = Instant::now();
            let s = crate::scorer::ClipScorer::load(&root.join("../models"), true, q).unwrap();
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
}

