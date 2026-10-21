use crate::modelfiles;
use crate::platform::GpuEncoder;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use image::imageops::FilterType;
use image::DynamicImage;
#[cfg(target_os = "macos")]
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
    let has = |n: &str| modelfiles::has_model(model_dir, n);
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
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables, unused_mut))]
    fn load(dir: &Path, gpu: bool) -> Result<Self, String> {
        let mut b = Session::builder().map_err(|e| e.to_string())?;
        // 加速只在 macOS 有（CoreML 提供者）；其它平台先用 CPU，Android 的 NNAPI 在阶段 B 第 2 步接上
        #[cfg(target_os = "macos")]
        if gpu {
            b = b.with_execution_providers([CoreML::default().build()]).map_err(|e| e.to_string())?;
        }
        let session = b.commit_from_file(dir.join(SIGLIP_FILE)).map_err(|e| e.to_string())?;
        let head: Head = serde_json::from_slice(&modelfiles::read_model(dir, SIGLIP_HEAD)?).map_err(|e| e.to_string())?;
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

/// Several copies of one ONNX session, used by whichever caller finds one free. One run on the CPU provider does not keep all the cores busy
/// (the work is a chain of matrix products with waits in between), so several runs at once get more done than one after the other: the scan
/// feeds it from many threads.
struct Pool {
    sessions: Vec<Mutex<Session>>,
    next: AtomicUsize,
}

impl Pool {
    fn run<R>(&self, f: impl FnOnce(&mut Session) -> Result<R, String>) -> Result<R, String> {
        let n = self.sessions.len();
        let start = self.next.fetch_add(1, Ordering::Relaxed);
        let mut f = Some(f);
        for i in 0..n {
            if let Ok(mut g) = self.sessions[(start + i) % n].try_lock() {
                return (f.take().expect("used once"))(&mut g);
            }
        }
        // all busy: wait for one
        let mut g = self.sessions[start % n].lock().map_err(|e| e.to_string())?;
        (f.take().expect("used once"))(&mut g)
    }
}

pub use crate::platform::physical_memory;

/// How many copies of the model are worth their memory. Measured: every copy of the small model holds about 600 MB (the float16 weights are
/// widened for the CPU) and a second one makes the encoder about 1.4x quicker, a third hardly more; the large model takes more than a
/// gigabyte a copy for about 10% (it already keeps all the cores busy), so it gets one.
fn sessions_for(model: &str) -> usize {
    if model == "ViT-L/14" { 1 } else if physical_memory() >> 30 >= 16 { 2 } else { 1 }
}

/// How the CPU runs a CLIP model (ONNX Runtime): what to load and how many copies of it.
struct CpuSpec {
    dir: PathBuf,
    file: &'static str,
    copies: usize,
    threads: usize,
}

impl CpuSpec {
    /// The sessions, built side by side (loading one takes a moment).
    fn build(&self, copies: usize) -> Result<Pool, String> {
        let one = || -> Result<Session, String> {
            let mut b = Session::builder().map_err(|e| e.to_string())?.with_intra_threads(self.threads).map_err(|e| e.to_string())?;
            b.commit_from_file(self.dir.join(self.file)).map_err(|e| e.to_string())
        };
        let sessions: Vec<Result<Session, String>> = std::thread::scope(|sc| {
            let hs: Vec<_> = (0..copies.max(1)).map(|_| sc.spawn(&one)).collect();
            hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("loading panicked".into()))).collect()
        });
        let sessions = sessions.into_iter().collect::<Result<Vec<_>, _>>()?;
        Ok(Pool { sessions: sessions.into_iter().map(Mutex::new).collect(), next: AtomicUsize::new(0) })
    }
}

fn run_clip(pool: &Pool, input: Vec<f32>) -> Result<Vec<f32>, String> {
    let input = Tensor::from_array(([1usize, 3, 224, 224], input)).map_err(|e| e.to_string())?;
    pool.run(|session| {
        let outputs = session.run(ort::inputs!["pixel_values" => input]).map_err(|e| e.to_string())?;
        let (_, data) = outputs["image_embeds"].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
        Ok(data.to_vec())
    })
}

/// A fixed, picture-like input for comparing two runs of the network.
fn probe_input() -> Vec<f32> {
    (0..3 * 224 * 224).map(|i| (((i * 7919) % 255) as f32) / 127.0 - 1.0).collect()
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let d: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    d / (a.iter().map(|x| x * x).sum::<f32>().sqrt() * b.iter().map(|x| x * x).sum::<f32>().sqrt()).max(1e-12)
}

/// The encoder on the GPU, when this machine can run it and it agrees with ONNX Runtime. The first time on a machine the two are compared on a fixed
/// input (a marker file remembers the verdict, so later starts skip the comparison and never load the CPU copy at all).
fn gpu_encoder(model_dir: &Path, file: &str, spec: &CpuSpec) -> Option<Box<dyn GpuEncoder>> {
    // 宿主没注册 GPU 编码器（Android 现在就是这样）就没有这条路
    let factory = crate::platform::gpu_clip()?;
    if std::env::var_os("FG_NO_GPU").is_some() {
        return None;
    }
    let bytes = modelfiles::read_model(model_dir, file).ok()?;
    let marker = std::env::temp_dir().join(format!("fg-clip-gpu-{file}-{}", bytes.len()));
    let (ok_marker, bad_marker) = (marker.with_extension("ok"), marker.with_extension("bad"));
    if bad_marker.exists() {
        return None;
    }
    let gpu = match factory.from_onnx(&bytes) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("GPU encoder not used: {e}");
            return None;
        }
    };
    drop(bytes);
    if ok_marker.exists() {
        return Some(gpu);
    }
    let input = probe_input();
    let verdict = (|| -> Result<bool, String> {
        let a = gpu.embed(&input)?;
        let b = run_clip(&spec.build(1)?, input.clone())?;
        Ok(a.iter().all(|v| v.is_finite()) && cosine(&a, &b) > 0.999)
    })();
    match verdict {
        Ok(true) => {
            let _ = std::fs::write(&ok_marker, b"ok");
            Some(gpu)
        }
        Ok(false) => {
            eprintln!("GPU encoder disagrees with ONNX Runtime: not used");
            let _ = std::fs::write(&bad_marker, b"bad");
            None
        }
        Err(e) => {
            eprintln!("GPU encoder not used: {e}");
            None
        }
    }
}

/// CLIP image encoder + LAION aesthetic head.
/// Where it runs is measured, not assumed. On a Mac the network is built on the GPU from the weights of the ONNX file (the desktop crate's clipgpu.rs, registered through `platform::GpuClipFactory`): ViT-L/14 takes
/// about 35 ms a picture there, against about 450 ms on the CPU. ONNX Runtime's CoreML provider is no way to get there: it refuses every node of
/// a float16 graph (the model then runs on the CPU, the GPU idle), and compiling the graph for it takes minutes and gigabytes. The CPU path is
/// the fallback (no GPU, an unfamiliar model, a mismatch), and then what made scanning faster was running several copies at once (see `Pool`).
pub struct ClipScorer {
    gpu: Option<Box<dyn GpuEncoder>>,
    /// The CPU sessions: built at once when there is no GPU path, otherwise only if the GPU ever fails.
    cpu: std::sync::OnceLock<Result<Pool, String>>,
    spec: CpuSpec,
    gpu_ok: std::sync::atomic::AtomicBool,
    head: Head,
    aes: Option<SiglipAesthetic>,
    pub backend: &'static str,
    pub model: &'static str,
}

impl ClipScorer {
    pub fn load(model_dir: &Path, accel: bool, want: Quality) -> Result<Self, String> {
        Self::load_with(model_dir, accel, want, None)
    }

    /// `copies`: how many CPU sessions to keep (None: as many as `sessions_for` says).
    pub fn load_with(model_dir: &Path, accel: bool, want: Quality, copies: Option<usize>) -> Result<Self, String> {
        let q = resolve(model_dir, accel, want);
        let (file, head_file, backend, model) = match q {
            Quality::High => (L14_FP16_FILE, L14_HEAD, "cpu-fp16", "ViT-L/14"),
            Quality::Fast | Quality::Ultra => {
                if accel && modelfiles::has_model(model_dir, B32_FP16_FILE) { (B32_FP16_FILE, B32_HEAD, "cpu-fp16", "ViT-B/32") } else { (INT8_FILE, B32_HEAD, "cpu-int8", "ViT-B/32") }
            }
        };
        let n = copies.unwrap_or_else(|| sessions_for(model)).max(1);
        // the cores are shared out between the copies
        let threads = (std::thread::available_parallelism().map(|c| c.get()).unwrap_or(4) / n).max(2);
        let spec = CpuSpec { dir: model_dir.to_path_buf(), file, copies: n, threads };
        // only the float16 models are built for the GPU (the int8 one is a quantised graph of another kind)
        let gpu = if backend == "cpu-fp16" { gpu_encoder(model_dir, file, &spec) } else { None };
        let cpu = std::sync::OnceLock::new();
        if gpu.is_none() {
            let pool = spec.build(n).map_err(|e| format!("load CLIP model: {e}"))?;
            let _ = cpu.set(Ok(pool));
        }
        let head: Head = serde_json::from_slice(&modelfiles::read_model(model_dir, head_file).map_err(|e| format!("read aesthetic head: {e}"))?).map_err(|e| e.to_string())?;
        // the newer scorer is added on top of the quick model, which keeps making the grouping / search vectors
        let (aes, model) = if q == Quality::Ultra { (Some(SiglipAesthetic::load(model_dir, true)?), "SigLIP so400m") } else { (None, model) };
        let backend = if gpu.is_some() { "gpu-fp16" } else { backend };
        Ok(Self { gpu, cpu, spec, gpu_ok: std::sync::atomic::AtomicBool::new(true), head, aes, backend, model })
    }
}

/// Resize the short side to 224 (bicubic), centre-crop, normalise to CHW floats — CLIP's own preprocessing.
pub fn preprocess(img: &DynamicImage) -> Vec<f32> {
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
        let input = preprocess(img);
        let mut emb: Vec<f32> = 'run: {
            if let Some(g) = &self.gpu {
                if self.gpu_ok.load(Ordering::Relaxed) {
                    match g.embed(&input) {
                        Ok(v) => break 'run v,
                        Err(e) => {
                            // from now on the CPU does it
                            eprintln!("GPU encoder: {e}; using the CPU");
                            self.gpu_ok.store(false, Ordering::Relaxed);
                        }
                    }
                }
            }
            let pool = self.cpu.get_or_init(|| self.spec.build(self.spec.copies)).as_ref().map_err(|e| e.clone())?;
            run_clip(pool, input)?
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

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    /// How the CLIP image encoder runs under different CoreML settings: latency of one inference, the work of four threads sharing one session
    /// (what a scan does now) against four sessions, and whether the embedding is the same.
    #[test]
    #[ignore = "timing only: models/; run with --ignored --nocapture"]
    fn clip_provider_options() {
        use ort::ep::coreml::{ComputeUnits, ModelFormat};
        use std::time::Instant;
        let dir = crate::dev_models_dir();
        type Make = Box<dyn Fn() -> CoreML>;
        let configs: Vec<(&str, Option<Make>)> = vec![
            ("current: CoreML default", Some(Box::new(|| CoreML::default()))),
            ("no CoreML (fp16 on the CPU)", None),
            ("MLProgram", Some(Box::new(|| CoreML::default().with_model_format(ModelFormat::MLProgram)))),
            ("MLProgram + static shapes", Some(Box::new(|| CoreML::default().with_model_format(ModelFormat::MLProgram).with_static_input_shapes(true)))),
            ("MLProgram + Neural Engine", Some(Box::new(|| CoreML::default().with_model_format(ModelFormat::MLProgram).with_compute_units(ComputeUnits::CPUAndNeuralEngine)))),
            ("MLProgram + GPU", Some(Box::new(|| CoreML::default().with_model_format(ModelFormat::MLProgram).with_compute_units(ComputeUnits::CPUAndGPU)))),
            ("NeuralNetwork + Neural Engine", Some(Box::new(|| CoreML::default().with_compute_units(ComputeUnits::CPUAndNeuralEngine)))),
            ("NeuralNetwork + GPU", Some(Box::new(|| CoreML::default().with_compute_units(ComputeUnits::CPUAndGPU)))),
        ];
        let mut seed = 7u64;
        let input: Vec<f32> = (0..3 * 224 * 224).map(|_| { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); ((seed >> 40) as f32) / ((1u64 << 24) as f32) * 2.0 - 1.0 }).collect();
        let run = |s: &Mutex<Session>| -> Vec<f32> {
            let t = Tensor::from_array(([1usize, 3, 224, 224], input.clone())).unwrap();
            let mut g = s.lock().unwrap();
            let o = g.run(ort::inputs!["pixel_values" => t]).unwrap();
            o["image_embeds"].try_extract_tensor::<f32>().unwrap().1.to_vec()
        };
        let cos = |a: &[f32], b: &[f32]| { let d: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum(); d / (a.iter().map(|x| x * x).sum::<f32>().sqrt() * b.iter().map(|x| x * x).sum::<f32>().sqrt()) };
        let mut reference: Option<Vec<f32>> = None;
        for (name, make) in &configs {
            let t0 = Instant::now();
            let build = || -> Result<Mutex<Session>, String> {
                let mut b = Session::builder().map_err(|e| e.to_string())?;
                if let Some(m) = make { b = b.with_execution_providers([m().build()]).map_err(|e| e.to_string())?; }
                Ok(Mutex::new(b.commit_from_file(dir.join(B32_FP16_FILE)).map_err(|e| e.to_string())?))
            };
            let s = match build() { Ok(s) => s, Err(e) => { println!("{name:<32} cannot load: {e}"); continue; } };
            let first = run(&s);
            let load = t0.elapsed();
            let t = Instant::now();
            for _ in 0..30 { run(&s); }
            let one = t.elapsed().as_secs_f64() * 1000.0 / 30.0;
            // four threads, one shared session (a scan today)
            let t = Instant::now();
            std::thread::scope(|sc| { for _ in 0..4 { sc.spawn(|| for _ in 0..16 { run(&s); }); } });
            let shared = t.elapsed().as_secs_f64() * 1000.0 / 64.0;
            // four threads, a session each
            let many: Vec<Mutex<Session>> = (0..4).filter_map(|_| build().ok()).collect();
            for m in &many { run(m); }
            let t = Instant::now();
            std::thread::scope(|sc| { for m in &many { sc.spawn(|| for _ in 0..16 { run(m); } ); } });
            let own = t.elapsed().as_secs_f64() * 1000.0 / 64.0;
            let same = reference.as_ref().map(|r| cos(r, &first));
            if reference.is_none() { reference = Some(first); }
            println!("{name:<32} load+first {:>6.0} ms | one {:>5.1} ms | 4 threads, one session {:>5.1} ms/img | 4 sessions {:>5.1} ms/img | cos vs current {}",
                load.as_secs_f64() * 1000.0, one, shared, own, same.map_or("-".into(), |c| format!("{c:.5}")));
        }
        // the CPU int8 model, for reference
        let mut b = Session::builder().unwrap();
        let s = Mutex::new(b.commit_from_file(dir.join(INT8_FILE)).unwrap());
        run(&s);
        let t = Instant::now();
        for _ in 0..10 { run(&s); }
        println!("{:<32} one {:>5.1} ms (CPU, int8)", "reference", t.elapsed().as_secs_f64() * 100.0);
    }

    /// How many CLIP sessions are worth having: the same 64 inferences on 1..4 sessions from 8 threads, and the memory it costs.
    #[test]
    #[ignore = "timing only: models/; run with --ignored --nocapture"]
    fn clip_session_pool_size() {
        use std::time::Instant;
        let dir = crate::dev_models_dir();
        let rss = || -> u64 { String::from_utf8_lossy(&std::process::Command::new("ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).output().unwrap().stdout).trim().parse::<u64>().unwrap_or(0) / 1024 };
        let input: Vec<f32> = (0..3 * 224 * 224).map(|i| ((i * 7919) % 255) as f32 / 127.0 - 1.0).collect();
        let build = || -> Mutex<Session> {
            let mut b = Session::builder().unwrap();
            if std::env::var("FG_NO_COREML").is_err() { b = b.with_execution_providers([CoreML::default().build()]).unwrap(); }
            Mutex::new(b.commit_from_file(dir.join(B32_FP16_FILE)).unwrap())
        };
        let run = |s: &Mutex<Session>| {
            let t = Tensor::from_array(([1usize, 3, 224, 224], input.clone())).unwrap();
            let mut g = s.lock().unwrap();
            let o = g.run(ort::inputs!["pixel_values" => t]).unwrap();
            let _ = o["image_embeds"].try_extract_tensor::<f32>().unwrap().1.len();
        };
        let base = rss();
        for n in 1..=4usize {
            let t0 = Instant::now();
            let pool: Vec<Mutex<Session>> = std::thread::scope(|sc| { let hs: Vec<_> = (0..n).map(|_| sc.spawn(build)).collect(); hs.into_iter().map(|h| h.join().unwrap()).collect() });
            let loaded = t0.elapsed();
            for m in &pool { run(m); }
            let t = Instant::now();
            std::thread::scope(|sc| {
                for w in 0..8usize {
                    let pool = &pool;
                    sc.spawn(move || for i in 0..8usize { run(&pool[(w + i) % pool.len()]); });
                }
            });
            println!("{n} session(s): load {:>5.0} ms, {:>5.1} ms/img over 64, memory +{} MB", loaded.as_secs_f64() * 1000.0, t.elapsed().as_secs_f64() * 1000.0 / 64.0, rss().saturating_sub(base));
        }
    }

    /// Whether the CLIP encoder takes a batch, and what a batch costs per picture.
    #[test]
    #[ignore = "timing only: models/; run with --ignored --nocapture"]
    fn clip_batches() {
        use std::time::Instant;
        let dir = crate::dev_models_dir();
        for (name, file, gpu) in [("B/32 fp16 CoreML", B32_FP16_FILE, true), ("B/32 int8 CPU", INT8_FILE, false)] {
            let mut b = Session::builder().unwrap();
            if gpu { b = b.with_execution_providers([CoreML::default().build()]).unwrap(); }
            let mut s = b.commit_from_file(dir.join(file)).unwrap();
            println!("{name}: input {:?}", s.inputs().iter().map(|i| format!("{} {:?}", i.name(), i.dtype())).collect::<Vec<_>>());
            for n in [1usize, 2, 4, 8, 16] {
                let data: Vec<f32> = (0..n * 3 * 224 * 224).map(|i| ((i * 7919) % 255) as f32 / 127.0 - 1.0).collect();
                let mk = || Tensor::from_array(([n, 3, 224, 224], data.clone())).unwrap();
                if let Err(e) = s.run(ort::inputs!["pixel_values" => mk()]) { println!("  batch {n}: {e}"); continue; }
                let reps = 6;
                let t = Instant::now();
                for _ in 0..reps { let _ = s.run(ort::inputs!["pixel_values" => mk()]).unwrap(); }
                let per_batch = t.elapsed().as_secs_f64() * 1000.0 / reps as f64;
                let out = s.run(ort::inputs!["pixel_values" => mk()]).unwrap();
                let shape = format!("{:?}", out["image_embeds"].try_extract_tensor::<f32>().unwrap().0);
                println!("  batch {n:>2}: {per_batch:>7.1} ms per batch = {:>5.1} ms per picture (output {shape})", per_batch / n as f64);
            }
        }
    }

    /// The ViT-L/14 encoder (the High quality tier, what a scan waits on) under different CoreML settings: one inference at a time.
    #[test]
    #[ignore = "timing only: models/; run with --ignored --nocapture"]
    fn clip_l14_options() {
        use ort::ep::coreml::{ComputeUnits, ModelFormat};
        use std::time::Instant;
        let dir = crate::dev_models_dir();
        type Make = Box<dyn Fn() -> Option<CoreML>>;
        let mut configs: Vec<(&str, bool, Make)> = vec![
            ("current: CoreML default", false, Box::new(|| Some(CoreML::default()))),
            ("MLProgram", false, Box::new(|| Some(CoreML::default().with_model_format(ModelFormat::MLProgram)))),
            ("MLProgram + static shapes", false, Box::new(|| Some(CoreML::default().with_model_format(ModelFormat::MLProgram).with_static_input_shapes(true)))),
            ("MLProgram + Neural Engine", false, Box::new(|| Some(CoreML::default().with_model_format(ModelFormat::MLProgram).with_compute_units(ComputeUnits::CPUAndNeuralEngine)))),
            ("MLProgram + GPU", false, Box::new(|| Some(CoreML::default().with_model_format(ModelFormat::MLProgram).with_compute_units(ComputeUnits::CPUAndGPU)))),
            ("MLProgram + GPU + fast predict", false, Box::new(|| Some(CoreML::default().with_model_format(ModelFormat::MLProgram).with_compute_units(ComputeUnits::CPUAndGPU).with_specialization_strategy(ort::ep::coreml::SpecializationStrategy::FastPrediction)))),
            ("NeuralNetwork + GPU", false, Box::new(|| Some(CoreML::default().with_compute_units(ComputeUnits::CPUAndGPU)))),
            ("NeuralNetwork + Neural Engine", false, Box::new(|| Some(CoreML::default().with_compute_units(ComputeUnits::CPUAndNeuralEngine)))),
        ];
        // the same, with the input pinned to 1 x 3 x 224 x 224 (the model says [-1, -1, -1, -1]: CoreML cannot take layers whose shapes are not known)
        let pinned: Vec<(&str, bool, Make)> = vec![
            ("pinned shape, CoreML default", true, Box::new(|| Some(CoreML::default()))),
            ("pinned shape, MLProgram", true, Box::new(|| Some(CoreML::default().with_model_format(ModelFormat::MLProgram)))),
            ("pinned shape, MLProgram + static", true, Box::new(|| Some(CoreML::default().with_model_format(ModelFormat::MLProgram).with_static_input_shapes(true)))),
            ("pinned shape, MLProgram + ANE", true, Box::new(|| Some(CoreML::default().with_model_format(ModelFormat::MLProgram).with_compute_units(ComputeUnits::CPUAndNeuralEngine)))),
            ("pinned shape, MLProgram + GPU", true, Box::new(|| Some(CoreML::default().with_model_format(ModelFormat::MLProgram).with_compute_units(ComputeUnits::CPUAndGPU)))),
            ("pinned shape, NN + static", true, Box::new(|| Some(CoreML::default().with_static_input_shapes(true)))),
            ("no CoreML (CPU only)", false, Box::new(|| None)),
            ("pinned shape, no CoreML (CPU only)", true, Box::new(|| None)),
            ("plan: MLProgram pinned GPU", true, Box::new(|| Some(CoreML::default().with_model_format(ModelFormat::MLProgram).with_compute_units(ComputeUnits::CPUAndGPU).with_profile_compute_plan(true)))),
            ("fp16 accumulation: MLProgram GPU", true, Box::new(|| Some(CoreML::default().with_model_format(ModelFormat::MLProgram).with_compute_units(ComputeUnits::CPUAndGPU).with_low_precision_accumulation_on_gpu(true)))),
            ("fp16 accumulation: NN GPU", true, Box::new(|| Some(CoreML::default().with_compute_units(ComputeUnits::CPUAndGPU).with_low_precision_accumulation_on_gpu(true)))),
        ];
        if std::env::var("FG_ONLY_PINNED").is_ok() { configs.clear(); }
        configs.extend(pinned);
        if std::env::var("FG_ORT_LOG").is_ok() {
            // what ONNX Runtime says while it splits the graph between the CoreML provider and the CPU
            ort::init().with_logger(std::sync::Arc::new(|_level: ort::logging::LogLevel, category: &str, _id: &str, _loc: &str, message: &str| {
                if message.contains("CoreML") || message.contains("partition") || message.contains("supported") { println!("[ort {category}] {message}"); }
            })).commit();
            if let Ok(env) = ort::environment::Environment::current() { env.set_log_level(ort::logging::LogLevel::Verbose); }
        }
        let input: Vec<f32> = (0..3 * 224 * 224).map(|i| (((i * 7919) % 255) as f32) / 127.0 - 1.0).collect();
        let mut reference: Option<Vec<f32>> = None;
        let only = std::env::var("FG_ONLY").ok();
        for (name, pin, make) in &configs {
            if only.as_deref().map_or(false, |o| if std::env::var("FG_EXACT").is_ok() { *name != o } else { !name.contains(o) }) { continue; }
            let t0 = Instant::now();
            let mut b = Session::builder().unwrap();
            if *pin {
                for (dim, n) in [("batch_size", 1), ("num_channels", 3), ("height", 224), ("width", 224)] { b = b.with_dimension_override(dim, n).unwrap(); }
            }
            if let Some(mut m) = make() {
                if let Ok(d) = std::env::var("FG_CACHE_DIR") { m = m.with_model_cache_dir(d); }
                b = match b.with_execution_providers([m.build()]) { Ok(b) => b, Err(e) => { println!("{name:<32} {e}"); continue; } };
            }
            let mut s = match b.commit_from_file(std::env::var("FG_MODEL").map(std::path::PathBuf::from).unwrap_or_else(|_| dir.join(L14_FP16_FILE))) { Ok(s) => s, Err(e) => { println!("{name:<32} cannot load: {e}"); continue; } };
            let mut run = |s: &mut Session| -> Vec<f32> {
                let t = Tensor::from_array(([1usize, 3, 224, 224], input.clone())).unwrap();
                let o = s.run(ort::inputs!["pixel_values" => t]).unwrap();
                o["image_embeds"].try_extract_tensor::<f32>().unwrap().1.to_vec()
            };
            let first = run(&mut s);
            let load = t0.elapsed();
            let t = Instant::now();
            for _ in 0..4 { run(&mut s); }
            let one = t.elapsed().as_secs_f64() * 1000.0 / 4.0;
            let cos = reference.as_ref().map(|r| { let d: f32 = r.iter().zip(&first).map(|(x, y)| x * y).sum(); d / (r.iter().map(|x| x * x).sum::<f32>().sqrt() * first.iter().map(|x| x * x).sum::<f32>().sqrt()) });
            if reference.is_none() { reference = Some(first); }
            println!("{name:<32} load+first {:>6.0} ms | one {:>6.1} ms | cos vs current {}", load.as_secs_f64() * 1000.0, one, cos.map_or("-".into(), |c| format!("{c:.5}")));
        }
    }

    #[test]
    #[ignore = "needs models/ and downloads ONNX Runtime on first build"]
    fn clip_smoke() {
        let dir = crate::dev_models_dir();
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
mod ultra_tests {
    use super::*;

    /// The Rust scorer must give what the reference (Python) run gave. FG_CACHE = the analysis cache, FG_V25 = the reference scores json
    /// ([name, thumb, score] rows), FG_N = how many photos to compare.
    #[test]
    #[ignore = "needs models/ (incl. the SigLIP scorer) and reference scores"]
    fn ultra_matches_reference() {
        let dir = crate::dev_models_dir();
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
        let dir = crate::dev_models_dir();
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
