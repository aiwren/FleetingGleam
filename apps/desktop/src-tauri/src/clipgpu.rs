//! The CLIP image encoder on the GPU (macOS): the network is built from the weights of the ONNX file with Metal Performance Shaders Graph (see
//! native/clip.swift). ONNX Runtime's CoreML provider takes none of the float16 models the app ships, so they would run on the CPU; this
//! runs the same network on the GPU, about ten times quicker for ViT-L/14, with no compiling step and no cache.
//!
//! Only the standard Hugging Face export of a CLIP vision model is understood (ViT-B/32, ViT-L/14); anything else is refused, and the caller keeps
//! using ONNX Runtime. The ONNX file is a protocol buffer; the little of it that is needed (the weights, and which weight each matrix product
//! uses) is read with a small reader instead of a protobuf library.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};

use fg_core::platform::{GpuClipFactory, GpuEncoder};

/// 注册给核心库的工厂：`scorer.rs` 通过它把 ONNX 的权重建到 GPU 上。
pub struct Factory;

impl GpuClipFactory for Factory {
    fn from_onnx(&self, model: &[u8]) -> Result<Box<dyn GpuEncoder>, String> {
        GpuClip::from_onnx(model).map(|g| Box::new(g) as Box<dyn GpuEncoder>)
    }
}

impl GpuEncoder for GpuClip {
    fn embed(&self, pixels: &[f32]) -> Result<Vec<f32>, String> {
        GpuClip::embed(self, pixels)
    }
    fn embed_dim(&self) -> usize {
        self.embed_dim
    }
}

/// The most pictures one GPU run takes: ViT-L/14 is 35 ms a picture alone and about 26 ms in a run of four (the GPU is nearly full by then),
/// ViT-B/32 is 9 ms against 3.6 ms.
const MAX_BATCH: usize = 4;

/// What one picture waiting for the GPU is: its pixels, and where its embedding goes.
struct Job {
    pixels: Vec<f32>,
    done: Arc<Slot>,
}

#[derive(Default)]
struct Slot {
    result: Mutex<Option<Result<Vec<f32>, String>>>,
    ready: Condvar,
}

/// A CLIP vision encoder running on the GPU. Pictures that arrive while the GPU is busy wait in a queue, and the next run takes up to four of
/// them at once (see `embed`), which is how many threads feeding it keep it busy without any of them waiting for a full batch.
///
/// 2026-10-23 为了把它推快试过、都没留下的做法（数字见 CHANGELOG"分析速度"一节）：批上限 8 / 16——单测里 GPU 每张从 3.8 降到 3.1–3.5 ms，
/// 可真实分析时批常凑不满、补零图白算，整条流水线反而略慢；一两条专职的高优先级线程来提交——无竞争时快一倍，十条分析线程占满核时
/// 唤醒它反而更慢；两批同时在飞（同一个 MPSGraph 可执行图并发 run）——不安全，十六条线程时有照片拿到了别人的向量；输入直接送 float16——
/// 向量逐位相同、GPU 时间少三成，但真实分析时 GPU 只忙三成，看不出效果；MPSGraph 一级优化——没有稳定差别。真正省下时间的是预处理
/// （`scorer::preprocess_for` 换了缩放器）。
pub struct GpuClip {
    #[cfg(target_os = "macos")]
    handle: *mut std::ffi::c_void,
    pub embed_dim: usize,
    queue: Mutex<Vec<Job>>,
    /// Held by whoever is running the GPU.
    running: Mutex<()>,
}

// The handle is only used through the Swift side, which takes a lock for every call.
unsafe impl Send for GpuClip {}
unsafe impl Sync for GpuClip {}

#[cfg(target_os = "macos")]
extern "C" {
    fn fg_clip_create(cfg: *const i32, weights: *const u8, lens: *const i64, count: i32) -> *mut std::ffi::c_void;
    fn fg_clip_embed(h: *mut std::ffi::c_void, pixels: *const f32, count: i32, out: *mut f32) -> i32;
    fn fg_clip_free(h: *mut std::ffi::c_void);
}

impl GpuClip {
    /// Builds the encoder from the bytes of a float16 (or float32) ONNX file.
    #[cfg(target_os = "macos")]
    pub fn from_onnx(model: &[u8]) -> Result<Self, String> {
        let mut w = Weights::read(model)?;
        w.cfg[7] = MAX_BATCH as i32;
        let lens: Vec<i64> = w.lens.iter().map(|&n| n as i64).collect();
        let h = unsafe { fg_clip_create(w.cfg.as_ptr(), w.bytes.as_ptr(), lens.as_ptr(), lens.len() as i32) };
        if h.is_null() {
            return Err("the GPU could not build the network".into());
        }
        Ok(Self { handle: h, embed_dim: w.cfg[5] as usize, queue: Mutex::new(Vec::new()), running: Mutex::new(()) })
    }

    #[cfg(not(target_os = "macos"))]
    pub fn from_onnx(_: &[u8]) -> Result<Self, String> {
        Err("the GPU encoder is only available on macOS".into())
    }

    /// One run on the GPU for `n` pictures laid out one after the other (1 to `MAX_BATCH`).
    #[cfg(target_os = "macos")]
    fn run(&self, pixels: &[f32], n: usize) -> Result<Vec<f32>, String> {
        let mut out = vec![0f32; n * self.embed_dim];
        if unsafe { fg_clip_embed(self.handle, pixels.as_ptr(), n as i32, out.as_mut_ptr()) } == 1 { Ok(out) } else { Err("the GPU run failed".into()) }
    }

    #[cfg(not(target_os = "macos"))]
    fn run(&self, _: &[f32], _: usize) -> Result<Vec<f32>, String> {
        Err("the GPU encoder is only available on macOS".into())
    }

    /// The embedding (not normalised) of a picture preprocessed the way CLIP wants: 3 x 224 x 224 floats.
    ///
    /// Callers come from many threads at once. Each puts its picture in the queue; whoever finds the GPU free runs the next batch (everything that is
    /// waiting, up to four) and hands each picture its embedding, then looks again for its own. Nobody waits for a batch to fill: one picture alone
    /// goes straight through.
    pub fn embed(&self, pixels: &[f32]) -> Result<Vec<f32>, String> {
        if pixels.len() != 3 * 224 * 224 {
            return Err("the picture is not 3 x 224 x 224".into());
        }
        let slot = Arc::new(Slot::default());
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).push(Job { pixels: pixels.to_vec(), done: slot.clone() });
        loop {
            if let Some(r) = slot.result.lock().unwrap_or_else(|e| e.into_inner()).take() {
                return r;
            }
            if let Ok(_gpu) = self.running.try_lock() {
                let jobs: Vec<Job> = {
                    let mut q = self.queue.lock().unwrap_or_else(|e| e.into_inner());
                    let n = q.len().min(MAX_BATCH);
                    q.drain(..n).collect()
                };
                if !jobs.is_empty() {
                    let all: Vec<f32> = jobs.iter().flat_map(|j| j.pixels.iter().copied()).collect();
                    let out = self.run(&all, jobs.len());
                    for (i, j) in jobs.iter().enumerate() {
                        let r = match &out {
                            Ok(v) => Ok(v[i * self.embed_dim..(i + 1) * self.embed_dim].to_vec()),
                            Err(e) => Err(e.clone()),
                        };
                        *j.done.result.lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
                        j.done.ready.notify_all();
                    }
                }
                continue;
            }
            // the GPU is busy: wait a moment for my result, then look again (the one who was running may be done)
            let g = slot.result.lock().unwrap_or_else(|e| e.into_inner());
            if g.is_none() {
                let _ = slot.ready.wait_timeout(g, std::time::Duration::from_millis(1));
            }
        }
    }
}

impl Drop for GpuClip {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        unsafe {
            fg_clip_free(self.handle);
        }
    }
}

// ---- reading the weights out of the ONNX file ----

const FLOAT: u64 = 1;
const FLOAT16: u64 = 10;

#[derive(Clone, Copy)]
enum Val<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    Other,
}

fn varint(b: &[u8], pos: &mut usize) -> Result<u64, String> {
    let (mut v, mut shift) = (0u64, 0u32);
    loop {
        let byte = *b.get(*pos).ok_or("truncated varint")?;
        *pos += 1;
        if shift >= 64 {
            return Err("varint too long".into());
        }
        v |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok(v);
        }
        shift += 7;
    }
}

fn fields(b: &[u8]) -> Result<Vec<(u32, Val<'_>)>, String> {
    let (mut pos, mut out) = (0usize, Vec::new());
    while pos < b.len() {
        let tag = varint(b, &mut pos)?;
        let (num, wire) = ((tag >> 3) as u32, tag & 7);
        let v = match wire {
            0 => Val::Varint(varint(b, &mut pos)?),
            1 => {
                pos += 8;
                Val::Other
            }
            2 => {
                let n = varint(b, &mut pos)? as usize;
                let s = b.get(pos..pos.checked_add(n).ok_or("bad length")?).ok_or("truncated field")?;
                pos += n;
                Val::Bytes(s)
            }
            5 => {
                pos += 4;
                Val::Other
            }
            w => return Err(format!("unsupported wire type {w}")),
        };
        out.push((num, v));
    }
    Ok(out)
}

fn bytes<'a>(v: &Val<'a>) -> Option<&'a [u8]> {
    if let Val::Bytes(b) = v { Some(b) } else { None }
}

fn text<'a>(v: &Val<'a>) -> Option<&'a str> {
    bytes(v).and_then(|b| std::str::from_utf8(b).ok())
}

struct Tensor<'a> {
    dims: Vec<usize>,
    dtype: u64,
    raw: &'a [u8],
}

fn tensor(b: &[u8]) -> Result<(&str, Tensor<'_>), String> {
    let (mut name, mut dims, mut dtype, mut raw) = ("", Vec::new(), 0u64, &b[0..0]);
    for (n, v) in fields(b)? {
        match (n, v) {
            (1, Val::Varint(d)) => dims.push(d as usize),
            // packed
            (1, Val::Bytes(p)) => {
                let mut pos = 0;
                while pos < p.len() {
                    dims.push(varint(p, &mut pos)? as usize);
                }
            }
            (2, Val::Varint(t)) => dtype = t,
            (8, Val::Bytes(s)) => name = std::str::from_utf8(s).map_err(|e| e.to_string())?,
            (9, Val::Bytes(r)) => raw = r,
            // float16 / float32 kept anywhere but raw_data, or stored outside the file
            (4, _) | (5, _) | (13, _) => return Err(format!("{name}: stored in a way that is not handled")),
            _ => {}
        }
    }
    Ok((name, Tensor { dims, dtype, raw }))
}

/// Single precision to half precision, round to nearest even.
fn f32_to_f16(f: f32) -> u16 {
    let x = f.to_bits();
    let sign = ((x >> 16) & 0x8000) as u16;
    let exp = ((x >> 23) & 0xff) as i32;
    let man = x & 0x007f_ffff;
    if exp == 0xff {
        return sign | 0x7c00 | if man != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        let m = man | 0x0080_0000;
        let shift = (14 - e) as u32;
        let half = (m >> shift) as u16;
        let rem = m & ((1 << shift) - 1);
        let halfway = 1 << (shift - 1);
        return sign | (half + if rem > halfway || (rem == halfway && half & 1 == 1) { 1 } else { 0 });
    }
    let half = ((e as u32) << 10 | (man >> 13)) as u16;
    let rem = man & 0x1fff;
    sign | (half + if rem > 0x1000 || (rem == 0x1000 && half & 1 == 1) { 1 } else { 0 })
}

/// What the GPU side needs: the sizes, and every weight as float16 bytes in the order `ClipGPU.init` takes them.
struct Weights {
    cfg: [i32; 8],
    bytes: Vec<u8>,
    lens: Vec<usize>,
}

impl Weights {
    fn read(model: &[u8]) -> Result<Self, String> {
        let top = fields(model)?;
        let graph = top.iter().find(|(n, _)| *n == 7).and_then(|(_, v)| bytes(v)).ok_or("no graph")?;
        let mut inits: HashMap<&str, Tensor> = HashMap::new();
        let mut matmul: HashMap<&str, &str> = HashMap::new();
        for (n, v) in fields(graph)? {
            match (n, bytes(&v)) {
                (5, Some(t)) => {
                    let (name, tn) = tensor(t)?;
                    inits.insert(name, tn);
                }
                (1, Some(node)) => {
                    let f = fields(node)?;
                    let op = f.iter().find(|(n, _)| *n == 4).and_then(|(_, v)| text(v));
                    if op == Some("MatMul") {
                        let name = f.iter().find(|(n, _)| *n == 3).and_then(|(_, v)| text(v)).unwrap_or("");
                        let ins: Vec<&str> = f.iter().filter(|(n, _)| *n == 1).filter_map(|(_, v)| text(v)).collect();
                        if ins.len() == 2 {
                            matmul.insert(name, ins[1]);
                        }
                    }
                }
                _ => {}
            }
        }
        let get = |name: &str| inits.get(name).ok_or_else(|| format!("the model has no {name}"));
        let conv = get("vision_model.embeddings.patch_embedding.weight")?;
        let pos = get("vision_model.embeddings.position_embedding.weight")?;
        if conv.dims.len() != 4 || pos.dims.len() != 2 || conv.dims[1] != 3 || conv.dims[2] != conv.dims[3] {
            return Err("not the expected CLIP vision model".into());
        }
        let (d, p, n) = (conv.dims[0], conv.dims[2], pos.dims[0]);
        let grid = 224 / p;
        if 224 % p != 0 || n != grid * grid + 1 || pos.dims[1] != d || d % 64 != 0 {
            return Err("not the expected CLIP vision model".into());
        }
        let mut layers = 0;
        while matmul.contains_key(format!("/vision_model/encoder/layers.{layers}/self_attn/q_proj/MatMul").as_str()) {
            layers += 1;
        }
        if layers == 0 {
            return Err("no encoder layers found".into());
        }
        let mm = |path: &str| -> Result<&Tensor, String> {
            let w = matmul.get(path).ok_or_else(|| format!("no matrix product {path}"))?;
            get(w)
        };
        let fc1 = mm("/vision_model/encoder/layers.0/mlp/fc1/MatMul")?;
        let proj = mm("/visual_projection/MatMul")?;
        if fc1.dims.len() != 2 || fc1.dims[0] != d || proj.dims.len() != 2 || proj.dims[0] != d {
            return Err("not the expected CLIP vision model".into());
        }
        let (mlp, embed) = (fc1.dims[1], proj.dims[1]);

        let mut out = Weights { cfg: [d as i32, n as i32, p as i32, layers as i32, (d / 64) as i32, embed as i32, mlp as i32, 1], bytes: Vec::new(), lens: Vec::new() };
        let mut push = |t: &Tensor, dims: &[usize]| -> Result<(), String> {
            let count: usize = dims.iter().product();
            if t.dims.iter().product::<usize>() != count {
                return Err(format!("a weight has {:?}, expected {:?}", t.dims, dims));
            }
            let before = out.bytes.len();
            match t.dtype {
                FLOAT16 if t.raw.len() == count * 2 => out.bytes.extend_from_slice(t.raw),
                FLOAT if t.raw.len() == count * 4 => {
                    for c in t.raw.chunks_exact(4) {
                        out.bytes.extend_from_slice(&f32_to_f16(f32::from_le_bytes([c[0], c[1], c[2], c[3]])).to_le_bytes());
                    }
                }
                _ => return Err("a weight is stored in a way that is not handled".into()),
            }
            out.lens.push(out.bytes.len() - before);
            Ok(())
        };
        push(conv, &[d, 3, p, p])?;
        push(get("vision_model.embeddings.class_embedding")?, &[d])?;
        push(pos, &[n, d])?;
        push(get("vision_model.pre_layrnorm.weight")?, &[d])?;
        push(get("vision_model.pre_layrnorm.bias")?, &[d])?;
        for i in 0..layers {
            let l = format!("vision_model.encoder.layers.{i}");
            let m = format!("/vision_model/encoder/layers.{i}");
            push(get(&format!("{l}.layer_norm1.weight"))?, &[d])?;
            push(get(&format!("{l}.layer_norm1.bias"))?, &[d])?;
            for (name, out_proj) in [("q_proj", false), ("k_proj", false), ("v_proj", false), ("out_proj", true)] {
                let _ = out_proj;
                push(mm(&format!("{m}/self_attn/{name}/MatMul"))?, &[d, d])?;
                push(get(&format!("{l}.self_attn.{name}.bias"))?, &[d])?;
            }
            push(get(&format!("{l}.layer_norm2.weight"))?, &[d])?;
            push(get(&format!("{l}.layer_norm2.bias"))?, &[d])?;
            push(mm(&format!("{m}/mlp/fc1/MatMul"))?, &[d, mlp])?;
            push(get(&format!("{l}.mlp.fc1.bias"))?, &[mlp])?;
            push(mm(&format!("{m}/mlp/fc2/MatMul"))?, &[mlp, d])?;
            push(get(&format!("{l}.mlp.fc2.bias"))?, &[d])?;
        }
        push(get("vision_model.post_layernorm.weight")?, &[d])?;
        push(get("vision_model.post_layernorm.bias")?, &[d])?;
        push(proj, &[d, embed])?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves_round_like_the_hardware() {
        for (f, h) in [(0.0f32, 0x0000u16), (1.0, 0x3c00), (-2.0, 0xc000), (65504.0, 0x7bff), (1e9, 0x7c00), (5.960_464_5e-8, 0x0001), (0.333_333_34, 0x3555), (1.0009766, 0x3c01)] {
            assert_eq!(f32_to_f16(f), h, "{f}");
        }
        // every half survives the trip through single precision
        for h in 0..=0x7bffu16 {
            let back = f32_to_f16(f32::from(half_to_f32(h)));
            assert_eq!(back, h, "{h:#06x}");
        }
    }

    fn half_to_f32(h: u16) -> f32 {
        let (e, m) = (((h >> 10) & 0x1f) as i32, (h & 0x3ff) as f32);
        if e == 0 { m * 2f32.powi(-24) } else { (1.0 + m / 1024.0) * 2f32.powi(e - 15) }
    }

    #[test]
    fn the_varint_reader_reads_packed_dims() {
        // TensorProto { dims: [2, 3] packed, data_type: 10, name: "w", raw_data: 12 bytes }
        let t = [&[0x0a, 0x02, 0x02, 0x03][..], &[0x10, 0x0a], &[0x42, 0x01, b'w'], &[0x4a, 0x0c], &[0u8; 12]].concat();
        let (name, tn) = tensor(&t).unwrap();
        assert_eq!((name, tn.dims.as_slice(), tn.dtype, tn.raw.len()), ("w", &[2usize, 3][..], FLOAT16, 12));
    }

    /// The GPU network gives the embedding the ONNX model gives, and how long it takes: FG_PHOTOS = a folder of photos (jpg) to preprocess.
    #[test]
    #[ignore = "needs models/ and a Metal GPU; FG_PHOTOS=<folder of jpgs>; run with --ignored --nocapture"]
    fn the_gpu_network_matches_onnx_runtime() {
        use ort::session::Session;
        use ort::value::Tensor as OrtTensor;
        use std::time::Instant;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../models");
        let photos: Vec<std::path::PathBuf> = std::env::var("FG_PHOTOS").ok().map(|d| {
            let mut v: Vec<_> = std::fs::read_dir(d).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().map_or(false, |e| e == "jpg")).collect();
            v.sort();
            v.truncate(6);
            v
        }).unwrap_or_default();
        let mut inputs: Vec<Vec<f32>> = vec![(0..3 * 224 * 224).map(|i| ((i * 7919) % 255) as f32 / 127.0 - 1.0).collect()];
        for p in &photos {
            inputs.push(fg_core::scorer::preprocess(&image::open(p).unwrap()));
        }
        let cos = |a: &[f32], b: &[f32]| { let d: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum(); d / (a.iter().map(|x| x * x).sum::<f32>().sqrt() * b.iter().map(|x| x * x).sum::<f32>().sqrt()) };
        for file in ["clip_vit_b32_vision_fp16.onnx", "clip_vit_l14_vision_fp16.onnx"] {
            let bytes = std::fs::read(dir.join(file)).unwrap();
            let t = Instant::now();
            let gpu = GpuClip::from_onnx(&bytes).unwrap();
            println!("\n{file}: GPU network built in {:?} (embedding of {})", t.elapsed(), gpu.embed_dim);
            let mut cpu = Session::builder().unwrap().commit_from_bytes_for_test(&bytes);
            let _ = &mut cpu;
            let mut worst = 1.0f32;
            let first = gpu.embed(&inputs[0]).unwrap();
            let _ = first;
            for (i, x) in inputs.iter().enumerate() {
                let g = gpu.embed(x).unwrap();
                let c = cpu.run_one(x);
                let s = cos(&g, &c);
                worst = worst.min(s);
                println!("  input {i}: cosine GPU vs CPU {s:.6}");
            }
            assert!(worst > 0.999, "{worst}");
            // 预热：凑一批四张跑一遍，别把第一次的开销算进计时
            std::thread::scope(|sc| { for _ in 0..8 { sc.spawn(|| { gpu.embed(&inputs[0]).unwrap(); }); } });
            let t = Instant::now();
            for _ in 0..10 { gpu.embed(&inputs[0]).unwrap(); }
            println!("  GPU: {:.1} ms a picture, one thread", t.elapsed().as_secs_f64() * 100.0);
            let t = Instant::now();
            std::thread::scope(|sc| { for _ in 0..8 { sc.spawn(|| for _ in 0..8 { gpu.embed(&inputs[0]).unwrap(); }); } });
            println!("  GPU: {:.1} ms a picture, 8 threads", t.elapsed().as_secs_f64() * 1000.0 / 64.0);
            // many threads at once: runs of up to four pictures, and every thread gets the embedding of its own picture
            let each: Vec<Vec<f32>> = (0..inputs.len()).map(|i| gpu.embed(&inputs[i]).unwrap()).collect();
            let t = Instant::now();
            std::thread::scope(|sc| {
                for w in 0..16usize {
                    let (gpu, inputs, each) = (&gpu, &inputs, &each);
                    sc.spawn(move || for k in 0..6usize {
                        let i = (w + k) % inputs.len();
                        let e = gpu.embed(&inputs[i]).unwrap();
                        assert!(cos(&e, &each[i]) > 0.9999, "thread {w} picture {i}: it got another picture's embedding");
                    });
                }
            });
            println!("  GPU, 16 threads: {:.1} ms a picture (and every picture got its own embedding)", t.elapsed().as_secs_f64() * 1000.0 / 96.0);
            let t = Instant::now();
            for _ in 0..3 { cpu.run_one(&inputs[0]); }
            println!("  CPU (ONNX Runtime): {:.1} ms a picture", t.elapsed().as_secs_f64() * 1000.0 / 3.0);
            let _ = OrtTensor::<f32>::from_array(([1usize], vec![0f32])).is_ok();
        }
    }

    /// 一张照片的向量不能取决于它和谁一批：同一批真实照片，单张逐一跑和十几条线程一起挤成四张一批跑，每张余弦 > 0.9999。
    /// `FG_PHOTOS=<文件夹> cargo test -p fleetinggleam --lib embeddings_do_not_depend_on_the_batch -- --ignored --nocapture`。
    #[test]
    #[ignore = "needs models/ and FG_PHOTOS"]
    fn embeddings_do_not_depend_on_the_batch() {
        let Ok(dir) = std::env::var("FG_PHOTOS") else { return };
        let models = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../models");
        let mut photos: Vec<std::path::PathBuf> = std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().map_or(false, |e| e == "jpg")).collect();
        photos.sort();
        photos.truncate(32);
        let inputs: Vec<Vec<f32>> = photos.iter().map(|p| fg_core::scorer::preprocess(&fg_core::analyze::load_for_embedding(p).unwrap())).collect();
        let cos = |a: &[f32], b: &[f32]| { let d: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum(); d / (a.iter().map(|x| x * x).sum::<f32>().sqrt() * b.iter().map(|x| x * x).sum::<f32>().sqrt()) };
        for file in ["clip_vit_b32_vision_fp16.onnx", "clip_vit_l14_vision_fp16.onnx"] {
            let gpu = GpuClip::from_onnx(&std::fs::read(models.join(file)).unwrap()).unwrap();
            let alone: Vec<Vec<f32>> = inputs.iter().map(|x| gpu.embed(x).unwrap()).collect();
            let together: Mutex<Vec<Option<Vec<f32>>>> = Mutex::new(vec![None; inputs.len()]);
            std::thread::scope(|sc| {
                for (i, x) in inputs.iter().enumerate() {
                    let (gpu, together) = (&gpu, &together);
                    sc.spawn(move || { let e = gpu.embed(x).unwrap(); together.lock().unwrap()[i] = Some(e); });
                }
            });
            let together = together.into_inner().unwrap();
            let worst = alone.iter().zip(&together).map(|(a, b)| cos(a, b.as_ref().unwrap())).fold(1.0f32, f32::min);
            println!("{file}: {} photos: worst cosine alone vs in a batch {worst:.6}", inputs.len());
            assert!(worst > 0.9999, "{file}: {worst}");
        }
    }

    /// 预处理换了缩放器，向量要和老的一样：同一批真实照片，老预处理（image crate 的 CatmullRom）和新预处理（fast_image_resize 的 CatmullRom）
    /// 喂同一个网络（GPU 上的 B/32 与 L/14、CPU 上的 B/32），余弦 > 0.9999；int8 模型（Android）的输入逐位不变。顺便量两者每张的 CPU 耗时。
    /// `FG_PHOTOS=<文件夹> cargo test -p fleetinggleam --lib the_faster_preprocessing_gives_the_same_embedding -- --ignored --nocapture`。
    #[test]
    #[ignore = "needs models/ and FG_PHOTOS"]
    fn the_faster_preprocessing_gives_the_same_embedding() {
        use std::time::Instant;
        let Ok(dir) = std::env::var("FG_PHOTOS") else { return };
        let models = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../models");
        let mut photos: Vec<std::path::PathBuf> = std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().map_or(false, |e| e == "jpg")).collect();
        photos.sort();
        photos.truncate(40);
        let imgs: Vec<image::DynamicImage> = photos.iter().map(|p| fg_core::analyze::load_for_embedding(p).unwrap()).collect();
        let pre = |file: &str| -> (Vec<Vec<f32>>, f64) {
            let t = Instant::now();
            let v: Vec<Vec<f32>> = imgs.iter().map(|i| fg_core::scorer::preprocess_for(i, file)).collect();
            (v, t.elapsed().as_secs_f64() * 1000.0 / imgs.len() as f64)
        };
        let (old, old_ms) = pre("clip_vit_b32_vision_int8.onnx");
        let (new, new_ms) = pre("clip_vit_b32_vision_fp16.onnx");
        let pix: f32 = old.iter().zip(&new).map(|(a, b)| a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32).fold(0.0, f32::max);
        println!("{} photos: preprocessing {old_ms:.1} ms (image crate) vs {new_ms:.1} ms (fast_image_resize); worst mean |Δinput| {pix:.4} (in normalised units)", imgs.len());
        let cos = |a: &[f32], b: &[f32]| { let d: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum(); d / (a.iter().map(|x| x * x).sum::<f32>().sqrt() * b.iter().map(|x| x * x).sum::<f32>().sqrt()) };
        for file in ["clip_vit_b32_vision_fp16.onnx", "clip_vit_l14_vision_fp16.onnx"] {
            let gpu = GpuClip::from_onnx(&std::fs::read(models.join(file)).unwrap()).unwrap();
            let worst = old.iter().zip(&new).map(|(a, b)| cos(&gpu.embed(a).unwrap(), &gpu.embed(b).unwrap())).fold(1.0f32, f32::min);
            println!("{file} (GPU): worst cosine old vs new preprocessing {worst:.6}");
            assert!(worst > 0.9999, "{file}: {worst}");
        }
        // float16 模型在 CPU 上（Mac 没有 GPU 编码器时的退路）也一样
        use ort::session::Session;
        let mut cpu = Session::builder().unwrap().commit_from_bytes_for_test(&std::fs::read(models.join("clip_vit_b32_vision_fp16.onnx")).unwrap());
        let worst = old.iter().zip(&new).take(12).map(|(a, b)| cos(&cpu.run_one(a), &cpu.run_one(b))).fold(1.0f32, f32::min);
        println!("clip_vit_b32_vision_fp16.onnx (CPU): worst cosine old vs new preprocessing {worst:.6}");
        assert!(worst > 0.9999, "fp16 CPU: {worst}");
        // Android 走 CPU 上的 int8 模型：它的输入必须和以前逐位相同（快的缩放器在它上面余弦只有 0.984，所以 int8 留在老路上）
        let again: Vec<Vec<f32>> = imgs.iter().map(fg_core::scorer::preprocess).collect();
        assert!(old.iter().zip(&again).all(|(a, b)| a == b), "the int8 model's input must not change");
        println!("clip_vit_b32_vision_int8.onnx (CPU, the Android path): input unchanged, bit for bit");
    }

    /// An ONNX Runtime session on the CPU for the comparison above.
    trait CpuRef {
        fn commit_from_bytes_for_test(self, bytes: &[u8]) -> Reference;
    }
    struct Reference(ort::session::Session);
    impl Reference {
        fn run_one(&mut self, x: &[f32]) -> Vec<f32> {
            let t = ort::value::Tensor::from_array(([1usize, 3, 224, 224], x.to_vec())).unwrap();
            let o = self.0.run(ort::inputs!["pixel_values" => t]).unwrap();
            let v = o["image_embeds"].try_extract_tensor::<f32>().unwrap().1.to_vec();
            v
        }
    }
    impl CpuRef for ort::session::builder::SessionBuilder {
        fn commit_from_bytes_for_test(mut self, bytes: &[u8]) -> Reference {
            Reference(self.commit_from_memory(bytes).unwrap())
        }
    }
}
