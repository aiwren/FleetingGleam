//! ONNX Runtime 会话的统一入口：所有模型都从这里建会话，平台加速器（macOS 的 CoreML、Android 的 NNAPI）只在这一处接。
//!
//! 大多数模型（int8 的文本 / 场景 / 翻译模型、小的人脸模型）就在 CPU 上跑，不碰加速器：它们要么是量化图，加速器不接；要么工作量很小。
//! 需要加速的只有视觉编码器（CLIP、SigLIP），由 `scorer.rs` 决定——而且先在固定输入上和 CPU 对一次答案，不一致就不用（见 `verified`）。

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use ort::session::builder::SessionBuilder;
use ort::session::Session;

/// 这个平台的加速器叫什么（给"分析信息"显示用）；没有就是 "cpu"。
pub fn accelerator() -> &'static str {
    if cfg!(target_os = "macos") {
        "coreml"
    } else if cfg!(target_os = "android") {
        "nnapi"
    } else {
        "cpu"
    }
}

pub fn has_accelerator() -> bool {
    accelerator() != "cpu"
}

/// 同一个 ONNX 模型的几份会话，谁来谁用空着的那份。ONNX Runtime 的 `run` 要独占会话，一份会话一次只能算一张；
/// 分析是很多条线程同时在喂，一份会话（以前的 `Mutex<Session>`）就把它们串成了一排。CPU 上一次推理也占不满所有核
/// （一串矩阵乘，中间有等待），所以几份同时跑比一份排队做得多。
pub struct Pool {
    sessions: Vec<Mutex<Session>>,
    next: AtomicUsize,
}

impl Pool {
    pub fn new(sessions: Vec<Session>) -> Self {
        Self { sessions: sessions.into_iter().map(Mutex::new).collect(), next: AtomicUsize::new(0) }
    }

    /// `copies` 份并排加载（加载一份要一会儿），每份算子内 `threads` 条线程。
    pub fn load(path: &Path, copies: usize, threads: Option<usize>) -> Result<Self, String> {
        let sessions: Vec<Result<Session, String>> = std::thread::scope(|sc| {
            let hs: Vec<_> = (0..copies.max(1)).map(|_| sc.spawn(|| load(path, threads))).collect();
            hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("loading panicked".into()))).collect()
        });
        Ok(Self::new(sessions.into_iter().collect::<Result<Vec<_>, _>>()?))
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// 找一份空着的会话跑 `f`；都忙就等自己轮到的那份。
    pub fn run<R>(&self, f: impl FnOnce(&mut Session) -> Result<R, String>) -> Result<R, String> {
        let n = self.sessions.len();
        let start = self.next.fetch_add(1, Ordering::Relaxed);
        let mut f = Some(f);
        for i in 0..n {
            if let Ok(mut g) = self.sessions[(start + i) % n].try_lock() {
                return (f.take().expect("used once"))(&mut g);
            }
        }
        let mut g = self.sessions[start % n].lock().map_err(|e| e.to_string())?;
        (f.take().expect("used once"))(&mut g)
    }

    /// 同 `run`，但闭包用 Option 回答（人脸那几个模型的习惯）。
    pub fn with<R>(&self, f: impl FnOnce(&mut Session) -> Option<R>) -> Option<R> {
        self.run(|s| Ok(f(s))).ok().flatten()
    }
}

/// 小模型（人脸检测、网格、表情、人物向量，每个几 MB）各留几份：核数的一半，至少两份、最多六份。一份一两条线程就够，
/// 并行来自几张照片同时算——以前每个模型只有一份、所有分析线程排队等它，手机上一半时间都在等人脸。
pub fn small_copies() -> usize {
    std::env::var("FG_SMALL_COPIES").ok().and_then(|v| v.parse().ok()).unwrap_or_else(|| (std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) / 2).clamp(2, 6))
}

/// 一个会话构建器；`threads` 是算子内并行的线程数（None = ONNX Runtime 自己定）。
pub fn builder(threads: Option<usize>) -> Result<SessionBuilder, String> {
    let mut b = Session::builder().map_err(|e| e.to_string())?;
    if let Some(n) = threads {
        b = b.with_intra_threads(n).map_err(|e| e.to_string())?;
    }
    Ok(b)
}

/// 路径上没有、但随应用带着的模型：它的字节。
fn inside_app(path: &Path) -> Option<&'static [u8]> {
    if path.exists() {
        return None;
    }
    crate::modelfiles::bundled(path.file_name()?.to_str()?)
}

/// 在 CPU 上跑的会话。下载目录里没有、但随应用带着的模型直接从内存建。
pub fn load(path: &Path, threads: Option<usize>) -> Result<Session, String> {
    let mut b = builder(threads)?;
    match inside_app(path) {
        Some(bytes) => b.commit_from_memory(bytes).map_err(|e| format!("{}: {e}", path.display())),
        None => b.commit_from_file(path).map_err(|e| format!("{}: {e}", path.display())),
    }
}

/// 用这个平台的加速器的会话：macOS 的 CoreML，Android 的 NNAPI；别的平台就是 CPU。
/// ONNX Runtime 会把加速器不接的节点留在 CPU 上，所以总能建出来；答案对不对要调用方自己验（`verified`）。
#[cfg_attr(not(any(target_os = "macos", target_os = "android")), allow(unused_mut))]
pub fn load_accelerated(path: &Path, threads: Option<usize>) -> Result<Session, String> {
    let mut b = builder(threads)?;
    #[cfg(target_os = "macos")]
    {
        b = b.with_execution_providers([ort::ep::CoreML::default().build()]).map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "android")]
    {
        // NCHW 省掉模型前后的转置；fp16 松弛不开（分数要和别的平台一致）
        b = b.with_execution_providers([ort::ep::NNAPI::default().with_nchw(true).build()]).map_err(|e| e.to_string())?;
    }
    match inside_app(path) {
        Some(bytes) => b.commit_from_memory(bytes).map_err(|e| format!("{}: {e}", path.display())),
        None => b.commit_from_file(path).map_err(|e| format!("{}: {e}", path.display())),
    }
}

/// 加速器上的会话是否和 CPU 给出同样的答案，这台机器上只对一次：`check` 建两个会话算一个固定输入、比较并返回是否一致；
/// 结论记在草稿目录的标记文件里（`.ok` / `.bad`），以后启动直接读。`key` 区分模型（名字 + 大小）。
pub fn verified(key: &str, check: impl FnOnce() -> Result<bool, String>) -> bool {
    if !has_accelerator() || std::env::var_os("FG_NO_ACCEL").is_some() {
        return false;
    }
    let marker = crate::platform::scratch_dir().join(format!("fg-accel-{}-{key}", accelerator()));
    let (ok_marker, bad_marker) = (marker.with_extension("ok"), marker.with_extension("bad"));
    if ok_marker.exists() {
        return true;
    }
    if bad_marker.exists() {
        return false;
    }
    match check() {
        Ok(true) => {
            let _ = std::fs::write(&ok_marker, b"ok");
            true
        }
        Ok(false) => {
            eprintln!("{} disagrees with the CPU on {key}: not used", accelerator());
            let _ = std::fs::write(&bad_marker, b"bad");
            false
        }
        Err(e) => {
            eprintln!("{} not used for {key}: {e}", accelerator());
            false
        }
    }
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let d: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    d / (a.iter().map(|x| x * x).sum::<f32>().sqrt() * b.iter().map(|x| x * x).sum::<f32>().sqrt()).max(1e-12)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_verdict_is_remembered() {
        let dir = std::env::temp_dir().join(format!("fg-onnx-verified-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        crate::platform::set_scratch_dir(dir.clone());
        if !has_accelerator() {
            assert!(!verified("x", || Ok(true)));
            return;
        }
        let key = format!("t{}", std::process::id());
        assert!(!verified(&key, || Ok(false)));
        assert!(!verified(&key, || panic!("the .bad marker answers")));
        let key2 = format!("u{}", std::process::id());
        assert!(verified(&key2, || Ok(true)));
        assert!(verified(&key2, || panic!("the .ok marker answers")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
