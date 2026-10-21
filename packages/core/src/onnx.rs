//! ONNX Runtime 会话的统一入口：所有模型都从这里建会话，平台加速器（macOS 的 CoreML、Android 的 NNAPI）只在这一处接。
//!
//! 大多数模型（int8 的文本 / 场景 / 翻译模型、小的人脸模型）就在 CPU 上跑，不碰加速器：它们要么是量化图，加速器不接；要么工作量很小。
//! 需要加速的只有视觉编码器（CLIP、SigLIP），由 `scorer.rs` 决定——而且先在固定输入上和 CPU 对一次答案，不一致就不用（见 `verified`）。

use std::path::Path;

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

/// 一个会话构建器；`threads` 是算子内并行的线程数（None = ONNX Runtime 自己定）。
pub fn builder(threads: Option<usize>) -> Result<SessionBuilder, String> {
    let mut b = Session::builder().map_err(|e| e.to_string())?;
    if let Some(n) = threads {
        b = b.with_intra_threads(n).map_err(|e| e.to_string())?;
    }
    Ok(b)
}

/// 在 CPU 上跑的会话。
pub fn load(path: &Path, threads: Option<usize>) -> Result<Session, String> {
    builder(threads)?.commit_from_file(path).map_err(|e| format!("{}: {e}", path.display()))
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
    b.commit_from_file(path).map_err(|e| format!("{}: {e}", path.display()))
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
