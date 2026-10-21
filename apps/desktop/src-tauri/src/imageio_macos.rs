//! 系统图像解码器（macOS）：ImageIO 边车 `fg-image`，给核心库的 `platform::SystemImages` 用。`image` crate 读不了的格式
//! （HDR 增益图、HEIC / AVIF / JPEG XL / TIFF / 相机 RAW）都由它解。别的平台没有这个边车，这些格式就不提供。

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use fg_core::platform::{ImageInfo, SystemImages};

/// 解一张很大的 RAW 可能要几秒；到时还没完成的边车是卡住了（或文件有问题），杀掉，这样一张照片永远拖不住整次分析。
const TIMEOUT: Duration = Duration::from_secs(60);

/// 只有系统解码器读得了的格式（macOS）：现代容器和相机 RAW（含 Apple ProRAW 等 DNG）。
const SYSTEM: [&str; 36] = [
    "heic", "heif", "avif", "jxl", "tif", "tiff", "bmp", "gif",
    "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef", "srw", "x3f", "3fr", "erf", "kdc", "mef", "mos", "mrw", "raw", "rwl", "iiq", "fff", "dcr", "k25", "rwz",
];

/// 跑边车并收集输出，到期就杀。两条管道都由线程读空，话多的子进程不会因为管道满了而卡住我们。
fn run(cmd: &mut Command) -> Option<Output> {
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
    let drain = |mut r: Option<std::process::ChildStdout>, mut e: Option<std::process::ChildStderr>| {
        std::thread::spawn(move || {
            use std::io::Read;
            let mut out = Vec::new();
            if let Some(r) = r.as_mut() { let _ = r.read_to_end(&mut out); }
            let mut err = Vec::new();
            if let Some(e) = e.as_mut() { let _ = e.read_to_end(&mut err); }
            (out, err)
        })
    };
    let reader = drain(child.stdout.take(), child.stderr.take());
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < TIMEOUT => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let (stdout, stderr) = reader.join().ok()?;
    Some(Output { status, stdout, stderr })
}

#[derive(serde::Deserialize)]
struct InfoRow {
    w: u32,
    h: u32,
    hdr: bool,
}

/// 边车所在的路径；`register` 时才知道（打包进应用的资源目录，或开发时仓库里的 `bin/`）。
pub struct Helper {
    bin: PathBuf,
}

impl Helper {
    /// 边车存在就把它注册给核心库；不存在（没编过 `scripts/build-vision.sh`）就什么都不注册，系统格式不提供。
    pub fn register(bin: PathBuf) {
        if cfg!(target_os = "macos") && bin.exists() {
            fg_core::platform::set_images(Box::new(Helper { bin }));
        }
    }
}

impl SystemImages for Helper {
    fn supports(&self, ext: &str) -> bool {
        SYSTEM.contains(&ext)
    }

    fn info(&self, path: &Path, with_hdr: bool) -> Option<ImageInfo> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg("info").arg(path);
        if !with_hdr {
            cmd.arg("fast");
        }
        let out = run(&mut cmd).filter(|o| o.status.success())?;
        let r: InfoRow = serde_json::from_slice(&out.stdout).ok()?;
        Some(ImageInfo { w: r.w, h: r.h, hdr: r.hdr })
    }

    fn render(&self, src: &Path, out: &Path, max: u32, sdr: bool) -> Result<(), String> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg("render").arg(src).arg(out).arg(max.to_string());
        if sdr {
            cmd.arg("sdr");
        }
        let o = run(&mut cmd).ok_or_else(|| format!("{}: the image helper did not finish in time", src.display()))?;
        if o.status.success() && out.exists() {
            Ok(())
        } else {
            Err(format!("{}: {}", src.display(), String::from_utf8_lossy(&o.stderr).trim()))
        }
    }

    fn crop(&self, src: &Path, out: &Path, f: [f64; 4], out_w: u32) -> Result<(), String> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg("crop").arg(src).arg(out).arg(f[0].to_string()).arg(f[1].to_string()).arg(f[2].to_string()).arg(f[3].to_string()).arg(out_w.to_string());
        let o = run(&mut cmd).ok_or_else(|| format!("{}: the image helper did not finish in time", src.display()))?;
        if o.status.success() && out.exists() { Ok(()) } else { Err(format!("{}: {}", src.display(), String::from_utf8_lossy(&o.stderr).trim())) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_and_modern_containers_are_the_helpers() {
        let h = Helper { bin: PathBuf::from("/nonexistent") };
        assert!(h.supports("dng") && h.supports("heic") && h.supports("jxl"));
        assert!(!h.supports("jpg") && !h.supports("txt"));
    }
}
