use crate::seal;
use std::path::Path;
use std::sync::Mutex;

use image::imageops::FilterType;
use image::DynamicImage;
use ort::ep::CoreML;
use ort::session::Session;
use ort::value::Tensor;
use serde::Deserialize;

const CLIP_SIZE: u32 = 224;
const CLIP_MEAN: [f32; 3] = [0.481_454_66, 0.457_827_5, 0.408_210_73];
const CLIP_STD: [f32; 3] = [0.268_629_54, 0.261_302_6, 0.275_777_1];

pub struct AiOutput {
    /// Aesthetic score normalised to 0.0..=1.0.
    pub score: f32,
    /// L2-normalised CLIP image embedding, used for semantic grouping.
    pub embedding: Vec<f32>,
}

/// Pluggable on-device image model. Implementations must never send pixels off the machine.
pub trait AiScorer: Send + Sync {
    fn analyze(&self, img: &DynamicImage) -> Result<AiOutput, String>;
}

#[derive(Deserialize)]
struct Head {
    weight: Vec<f32>,
    bias: f32,
    /// Raw score range mapped onto 0..1 (LAION scores mostly fall in a band of a few points).
    #[serde(default = "default_lo")]
    lo: f32,
    #[serde(default = "default_hi")]
    hi: f32,
}
fn default_lo() -> f32 { 3.0 }
fn default_hi() -> f32 { 7.0 }

/// Which CLIP + aesthetic head to use.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Quality {
    /// ViT-B/32: small and quick.
    Fast,
    /// ViT-L/14 with the improved LAION aesthetic predictor: much larger, noticeably better judgement.
    High,
    /// A newer, larger aesthetic scorer (SigLIP so400m + Aesthetic Predictor V2.5) for the score; the quick model still makes the
    /// vectors used for grouping and search, so those stay consistent across all three tiers.
    Ultra,
}

impl Quality {
    pub fn parse(s: Option<&str>) -> Self {
        match s { Some("fast") => Quality::Fast, Some("ultra") => Quality::Ultra, _ => Quality::High }
    }
}

const INT8_FILE: &str = "clip_vit_b32_vision_int8.onnx";
const B32_FP16_FILE: &str = "clip_vit_b32_vision_fp16.onnx";
const B32_HEAD: &str = "aesthetic_head_vit_b_32.json";
const L14_FP16_FILE: &str = "clip_vit_l14_vision_fp16.onnx";
const L14_HEAD: &str = "aesthetic_head_vit_l_14.json";
pub const SIGLIP_FILE: &str = "siglip_so400m_vision_fp16.onnx";
pub const SIGLIP_HEAD: &str = "aesthetic_v25_head.json";

/// What will actually run for a request. High needs its files and an accelerator (the L/14 model is fp16 only);
/// otherwise it quietly falls back to Fast, and everything downstream (cache, reporting) uses the effective choice.
pub fn resolve(model_dir: &Path, accel: bool, want: Quality) -> Quality {
    let has = |n: &str| seal::has_model(model_dir, n);
    let quick = has(B32_FP16_FILE) || has(INT8_FILE);
    if want == Quality::Ultra && accel && quick && has(SIGLIP_FILE) && has(SIGLIP_HEAD) {
        Quality::Ultra
    } else if want != Quality::Fast && accel && has(L14_FP16_FILE) && has(L14_HEAD) {
        Quality::High
    } else {
        Quality::Fast
    }
}

/// Part of every cache entry: change any of these and old analyses are redone.
pub fn analysis_tag(q: Quality) -> String {
    format!("v3|{}|thumb640", match q { Quality::Ultra => "s25", Quality::High => "l14", Quality::Fast => "b32" })
}

/// The newer aesthetic scorer: a SigLIP so400m image encoder and a small head trained for aesthetics (scores 1-10, 5.5+ is great).
/// The head's linear layers have no non-linearity between them, so it is stored collapsed into one weight vector and a bias.
pub struct SiglipAesthetic {
    session: Mutex<Session>,
    head: Head,
}

impl SiglipAesthetic {
    fn load(dir: &Path, gpu: bool) -> Result<Self, String> {
        let mut b = Session::builder().map_err(|e| e.to_string())?;
        if gpu {
            b = b.with_execution_providers([CoreML::default().build()]).map_err(|e| e.to_string())?;
        }
        let session = if seal::is_sealed(dir, SIGLIP_FILE) {
            b.commit_from_memory(&seal::read_model(dir, SIGLIP_FILE)?).map_err(|e| e.to_string())?
        } else {
            b.commit_from_file(dir.join(SIGLIP_FILE)).map_err(|e| e.to_string())?
        };
        let head: Head = serde_json::from_slice(&seal::read_model(dir, SIGLIP_HEAD)?).map_err(|e| e.to_string())?;
        Ok(Self { session: Mutex::new(session), head })
    }

    /// 384 x 384 (the picture is squeezed to a square, as SigLIP was trained), pixels scaled to -1..1.
    fn score(&self, img: &DynamicImage) -> Result<f32, String> {
        let rgb = img.resize_exact(384, 384, FilterType::CatmullRom).to_rgb8();
        let plane = 384 * 384;
        let mut data = vec![0f32; 3 * plane];
        for (i, p) in rgb.pixels().enumerate() {
            for c in 0..3 {
                data[c * plane + i] = p[c] as f32 / 127.5 - 1.0;
            }
        }
        let input = Tensor::from_array(([1usize, 3, 384, 384], data)).map_err(|e| e.to_string())?;
        let emb: Vec<f32> = {
            let mut s = self.session.lock().map_err(|e| e.to_string())?;
            let out = s.run(ort::inputs!["pixel_values" => input]).map_err(|e| e.to_string())?;
            let (_, d) = out["image_embeds"].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
            d.to_vec()
        };
        if emb.len() != self.head.weight.len() || emb.iter().any(|v| !v.is_finite()) {
            return Err("the aesthetic model produced an unusable embedding".into());
        }
        let norm = emb.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-9);
        let raw: f32 = emb.iter().zip(&self.head.weight).map(|(a, b)| a / norm * b).sum::<f32>() + self.head.bias;
        Ok(((raw - self.head.lo) / (self.head.hi - self.head.lo)).clamp(0.0, 1.0))
    }
}

/// CLIP image encoder (ONNX, CoreML-accelerated where it helps) + LAION aesthetic head.
/// Backend choice is measured, not assumed: int8 B/32 is fastest on the CPU, while fp16 through CoreML wins over a
/// whole batch because it frees the CPU for decoding and Apple Vision.
pub struct ClipScorer {
    session: Mutex<Session>,
    head: Head,
    aes: Option<SiglipAesthetic>,
    pub backend: &'static str,
    pub model: &'static str,
}

impl ClipScorer {
    pub fn load(model_dir: &Path, accel: bool, want: Quality) -> Result<Self, String> {
        let q = resolve(model_dir, accel, want);
        let (file, head_file, backend, model, gpu) = match q {
            Quality::High => (L14_FP16_FILE, L14_HEAD, "coreml-fp16", "ViT-L/14", true),
            Quality::Fast | Quality::Ultra => {
                if accel && seal::has_model(model_dir, B32_FP16_FILE) { (B32_FP16_FILE, B32_HEAD, "coreml-fp16", "ViT-B/32", true) } else { (INT8_FILE, B32_HEAD, "cpu-int8", "ViT-B/32", false) }
            }
        };
        let build = || -> Result<Session, String> {
            let mut b = Session::builder().map_err(|e| e.to_string())?;
            if gpu {
                b = b.with_execution_providers([CoreML::default().build()]).map_err(|e| e.to_string())?;
            }
            if seal::is_sealed(model_dir, file) {
                // A sealed model is decrypted in memory only; there is never a plaintext file on disk.
                let bytes = seal::read_model(model_dir, file)?;
                b.commit_from_memory(&bytes).map_err(|e| e.to_string())
            } else {
                b.commit_from_file(model_dir.join(file)).map_err(|e| e.to_string())
            }
        };
        let session = build().map_err(|e| format!("load CLIP model: {e}"))?;
        let head: Head = serde_json::from_slice(&seal::read_model(model_dir, head_file).map_err(|e| format!("read aesthetic head: {e}"))?).map_err(|e| e.to_string())?;
        // the newer scorer is added on top of the quick model, which keeps making the grouping / search vectors
        let (aes, model) = if q == Quality::Ultra { (Some(SiglipAesthetic::load(model_dir, true)?), "SigLIP so400m") } else { (None, model) };
        Ok(Self { session: Mutex::new(session), head, aes, backend, model })
    }
}

/// Resize the short side to 224 (bicubic), centre-crop, normalise to CHW floats — CLIP's own preprocessing.
fn preprocess(img: &DynamicImage) -> Vec<f32> {
    let (w, h) = (img.width() as f32, img.height() as f32);
    let scale = CLIP_SIZE as f32 / w.min(h);
    let (nw, nh) = ((w * scale).round().max(224.0) as u32, (h * scale).round().max(224.0) as u32);
    let rgb = img.resize_exact(nw, nh, FilterType::CatmullRom).to_rgb8();
    let (x0, y0) = ((nw - CLIP_SIZE) / 2, (nh - CLIP_SIZE) / 2);
    let plane = (CLIP_SIZE * CLIP_SIZE) as usize;
    let mut out = vec![0f32; 3 * plane];
    for y in 0..CLIP_SIZE {
        for x in 0..CLIP_SIZE {
            let p = rgb.get_pixel(x0 + x, y0 + y);
            let i = (y * CLIP_SIZE + x) as usize;
            for c in 0..3 {
                out[c * plane + i] = (p[c] as f32 / 255.0 - CLIP_MEAN[c]) / CLIP_STD[c];
            }
        }
    }
    out
}

impl ClipScorer {
    /// The unit-length CLIP embedding of a picture: what grouping, search and the learned taste work with.
    pub fn embed(&self, img: &DynamicImage) -> Result<Vec<f32>, String> {
        let input = Tensor::from_array(([1usize, 3, 224, 224], preprocess(img)))
            .map_err(|e| e.to_string())?;
        let mut emb: Vec<f32> = {
            let mut session = self.session.lock().map_err(|e| e.to_string())?;
            let outputs = session
                .run(ort::inputs!["pixel_values" => input])
                .map_err(|e| e.to_string())?;
            let (_, data) = outputs["image_embeds"]
                .try_extract_tensor::<f32>()
                .map_err(|e| e.to_string())?;
            data.to_vec()
        };
        if emb.iter().any(|v| !v.is_finite()) {
            return Err("model produced non-finite embedding".into());
        }
        let norm = emb.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-9);
        emb.iter_mut().for_each(|v| *v /= norm);
        Ok(emb)
    }
}

impl AiScorer for ClipScorer {
    fn analyze(&self, img: &DynamicImage) -> Result<AiOutput, String> {
        let emb = self.embed(img)?;

        let raw: f32 = emb.iter().zip(&self.head.weight).map(|(a, b)| a * b).sum::<f32>() + self.head.bias;
        let score = match &self.aes {
            Some(a) => a.score(img)?,
            None => ((raw - self.head.lo) / (self.head.hi - self.head.lo)).clamp(0.0, 1.0),
        };
        Ok(AiOutput { score, embedding: emb })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "needs models/ and downloads ONNX Runtime on first build"]
    fn clip_smoke() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../models");
        let s = ClipScorer::load(&dir, true, Quality::Fast).expect("load");
        let flat = DynamicImage::new_rgb8(640, 480);
        let noise = DynamicImage::ImageRgb8(image::RgbImage::from_fn(640, 480, |x, y| {
            let v = ((x * 7919 + y * 104729) % 256) as u8;
            image::Rgb([v, v ^ 0x55, 255 - v])
        }));
        let mut imgs = vec![("flat", flat), ("noise", noise)];
        if let Ok(p) = std::env::var("PROBE_IMG") {
            imgs.push(("probe", image::open(p).unwrap()));
        }
        for (n, img) in imgs {
            match s.analyze(&img) {
                Ok(o) => {
                    let norm: f32 = o.embedding.iter().map(|v| v * v).sum::<f32>().sqrt();
                    println!("{n}: score={:.3} dim={} norm={norm:.3}", o.score, o.embedding.len());
                    assert_eq!(o.embedding.len(), 512);
                }
                Err(e) => println!("{n}: rejected ({e})"),
            }
        }
    }
}

#[cfg(test)]
mod sealed_tests {
    use super::*;

    /// The sealed models must give exactly the same scores as the plain ones. Needs the real models directory.
    #[test]
    #[ignore = "needs models/"]
    fn sealed_scorer_matches_plain() {
        let plain = Path::new(env!("CARGO_MANIFEST_DIR")).join("../models");
        let sealed = std::env::temp_dir().join(format!("fg-sealed-{}", std::process::id()));
        std::fs::create_dir_all(&sealed).unwrap();
        for name in [INT8_FILE, B32_HEAD] {
            seal::seal_file(&plain.join(name), &sealed.join(format!("{name}.{}", seal::EXT)), [3u8; 12]).unwrap();
        }
        assert!(!sealed.join(INT8_FILE).exists(), "no plaintext next to the sealed files");
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_fn(300, 200, |x, y| image::Rgb([(x % 251) as u8, (y % 241) as u8, ((x + y) % 239) as u8])));
        let a = ClipScorer::load(&plain, false, Quality::Fast).unwrap().analyze(&img).unwrap();
        let b = ClipScorer::load(&sealed, false, Quality::Fast).unwrap().analyze(&img).unwrap();
        assert_eq!(a.embedding, b.embedding);
        assert_eq!(a.score, b.score);
        let _ = std::fs::remove_dir_all(&sealed);
    }
}

#[cfg(test)]
mod ultra_tests {
    use super::*;

    /// The Rust scorer must give what the reference (Python) run gave. FG_CACHE = the analysis cache, FG_V25 = the reference scores json
    /// ([name, thumb, score] rows), FG_N = how many photos to compare.
    #[test]
    #[ignore = "needs models/ (incl. the SigLIP scorer) and reference scores"]
    fn ultra_matches_reference() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../models");
        let refs: Vec<(String, String, f64)> = serde_json::from_slice(&std::fs::read(std::env::var("FG_V25").unwrap()).unwrap()).unwrap();
        let n: usize = std::env::var("FG_N").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
        let s = ClipScorer::load(&dir, true, Quality::Ultra).expect("load ultra");
        assert_eq!(s.model, "SigLIP so400m");
        let t = std::time::Instant::now();
        for (name, thumb, want) in refs.iter().take(n) {
            let img = image::open(thumb).unwrap();
            let got = s.analyze(&img).unwrap();
            let raw = got.score as f64 * 4.5 + 3.0; // back from 0..1 to the 1-10 scale (lo 3.0, hi 7.5)
            println!("{name}: rust {raw:.3}  reference {want:.3}  embedding dim {}", got.embedding.len());
            assert!((raw - want).abs() < 0.05, "{name}: {raw} vs {want}");
            assert_eq!(got.embedding.len(), 512, "grouping/search vectors still come from the quick model");
        }
        println!("{:.0} ms/photo", t.elapsed().as_millis() as f64 / n as f64);
    }
}

#[cfg(test)]
mod search_tests {
    use super::*;
    use crate::textsearch::{self, TextEncoder};

    /// Real-data check of the search: needs models/ and the app's analysis cache (your own photos).
    /// FG_CACHE = the cache file, FG_QUERIES = queries separated by |, FG_OUT = folder for one contact sheet (png) per query.
    #[test]
    #[ignore = "needs models/ and a real analysis cache"]
    fn search_quality_on_real_photos() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../models");
        let cache = std::env::var("FG_CACHE").unwrap_or_default();
        let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(&cache).expect("FG_CACHE")).unwrap();
        let scorer = ClipScorer::load(&dir, true, Quality::Fast).unwrap();
        struct Row { name: String, thumb: String, sem: Vec<f32>, color: Vec<u8>, faces: u32 }
        let mut rows: Vec<Row> = Vec::new();
        for (k, e) in raw["entries"].as_object().unwrap() {
            let thumb = e["photo"]["thumb"].as_str().unwrap().to_string();
            if let Ok(img) = image::open(&thumb) {
                rows.push(Row {
                    name: Path::new(k).file_name().unwrap().to_string_lossy().into_owned(),
                    sem: scorer.analyze(&img).unwrap().embedding,
                    color: crate::colorsig::signature(&img),
                    faces: e["photo"]["face_count"].as_u64().unwrap_or(0) as u32,
                    thumb,
                });
            }
        }
        println!("{} photos embedded", rows.len());
        let enc = TextEncoder::load(&dir).unwrap();
        for query in std::env::var("FG_QUERIES").unwrap_or_default().split('|') {
            let q = textsearch::parse(query);
            let qv = q.text.as_ref().map(|t| enc.encode_query(t).unwrap());
            let items: Vec<textsearch::Item> = rows.iter().map(|r| textsearch::Item { sem: Some(&r.sem), color: Some(&r.color), faces: r.faces }).collect();
            let hits = textsearch::rank(&q, qv.as_deref(), &items);
            println!("[{query}] -> text {:?} colours {:?} background {} people {:?}: {} hits; top {}", q.text, q.colours, q.background, q.people, hits.len(),
                hits.iter().take(6).map(|(i, s)| format!("{}:{s:.2}", rows[*i].name)).collect::<Vec<_>>().join(" "));
            if let Ok(out) = std::env::var("FG_OUT") {
                let shown: Vec<_> = hits.iter().take(24).collect();
                let (cw, ch, cols) = (200u32, 140u32, 6u32);
                let n_rows = ((shown.len() as u32).max(1) + cols - 1) / cols;
                let mut sheet = image::RgbImage::from_pixel(cols * cw, n_rows * ch, image::Rgb([20, 20, 22]));
                for (k, (i, _)) in shown.iter().enumerate() {
                    if let Ok(t) = image::open(&rows[*i].thumb) {
                        let t = t.resize(cw - 6, ch - 6, image::imageops::FilterType::Triangle).to_rgb8();
                        image::imageops::overlay(&mut sheet, &t, ((k as u32 % cols) * cw + 3) as i64, ((k as u32 / cols) * ch + 3) as i64);
                    }
                }
                let _ = std::fs::create_dir_all(&out);
                sheet.save(Path::new(&out).join(format!("{}.png", query.replace(' ', "_")))).unwrap();
            }
        }
    }
}
