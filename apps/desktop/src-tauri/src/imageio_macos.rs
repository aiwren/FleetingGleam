//! 系统图像解码器（macOS）：ImageIO 边车 `fg-image`，给核心库的 `platform::SystemImages` 用。`image` crate 读不了的格式
//! （HDR 增益图、HEIC / AVIF / JPEG XL / TIFF / 相机 RAW）都由它解。别的平台没有这个边车，这些格式就不提供。

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fg_core::platform::{ImageInfo, SystemImages};
use image::DynamicImage;

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

/// 常驻解码边车（`fg-image serve`）的回复头。
#[derive(serde::Deserialize)]
struct DecodeHead {
    id: u64,
    w: Option<u32>,
    h: Option<u32>,
    ow: Option<u32>,
    oh: Option<u32>,
    error: Option<String>,
}

/// 一个常驻的 `fg-image serve` 进程：分析线程各自写一行请求（锁一下 stdin），一条读线程按 id 把像素分发回去。
/// 同一个进程里同时解几张（`--par`），所以整个应用只有一个。死了（崩溃、超时被杀）就在下一个请求时重起。
struct Server {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    pending: Mutex<HashMap<u64, SyncSender<Option<(DynamicImage, u32, u32)>>>>,
    next_id: AtomicU64,
    dead: AtomicBool,
}

impl Server {
    fn spawn(bin: &Path, parallel: usize) -> Option<Arc<Server>> {
        // 调性能时（FG_IMAGE_TRACE=1）让边车的 stderr 透出来
        let stderr = if std::env::var("FG_IMAGE_TRACE").map_or(false, |v| v == "1") { Stdio::inherit() } else { Stdio::null() };
        let mut child = Command::new(bin).arg("serve").arg("--par").arg(parallel.max(1).to_string()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(stderr).spawn().ok()?;
        let stdin = child.stdin.take()?;
        let mut out = BufReader::with_capacity(1 << 20, child.stdout.take()?);
        let server = Arc::new(Server { child: Mutex::new(child), stdin: Mutex::new(stdin), pending: Mutex::new(HashMap::new()), next_id: AtomicU64::new(1), dead: AtomicBool::new(false) });
        let weak = Arc::downgrade(&server);
        std::thread::spawn(move || {
            let mut line = String::new();
            loop {
                line.clear();
                let Ok(n) = out.read_line(&mut line) else { break };
                if n == 0 {
                    break;
                }
                let Ok(head) = serde_json::from_str::<DecodeHead>(&line) else { break };
                // 头后面紧跟像素；读不满就是管道断了
                let body = match (head.w, head.h, head.ow, head.oh, &head.error) {
                    (Some(w), Some(h), Some(ow), Some(oh), None) if w > 0 && h > 0 => {
                        let mut px = vec![0u8; (w as usize) * (h as usize) * 3];
                        if out.read_exact(&mut px).is_err() {
                            break;
                        }
                        image::RgbImage::from_raw(w, h, px).map(|img| (DynamicImage::ImageRgb8(img), ow, oh))
                    }
                    _ => None,
                };
                let Some(s) = weak.upgrade() else { return };
                let tx = s.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&head.id);
                if let Some(tx) = tx {
                    let _ = tx.try_send(body);
                }
            }
            if let Some(s) = weak.upgrade() {
                s.dead.store(true, Ordering::Relaxed);
                s.pending.lock().unwrap_or_else(|e| e.into_inner()).clear();
            }
        });
        Some(server)
    }

    fn kill(&self) {
        self.dead.store(true, Ordering::Relaxed);
        if let Ok(mut c) = self.child.lock() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    fn decode(&self, src: &Path, max: u32, managed: bool) -> Option<(DynamicImage, u32, u32)> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).insert(id, tx);
        let written = {
            let mut w = self.stdin.lock().unwrap_or_else(|e| e.into_inner());
            writeln!(w, "{}", serde_json::json!({ "id": id, "path": src.to_string_lossy(), "max": max, "managed": managed })).and_then(|_| w.flush()).is_ok()
        };
        if !written {
            self.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
            self.dead.store(true, Ordering::Relaxed);
            return None;
        }
        match rx.recv_timeout(TIMEOUT) {
            Ok(r) => r,
            Err(RecvTimeoutError::Disconnected) => None,
            Err(RecvTimeoutError::Timeout) => {
                // 卡住了（巨大的 RAW、坏文件）：杀掉重起，别的在等的请求也会得到"没有"，各自走老路子
                self.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
                self.kill();
                None
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.kill();
    }
}

/// 边车所在的路径；`register` 时才知道（打包进应用的资源目录，或开发时仓库里的 `bin/`）。
pub struct Helper {
    bin: PathBuf,
    /// 常驻的解码进程，第一次要用时才起。
    server: Mutex<Option<Arc<Server>>>,
}

impl Helper {
    /// 边车存在就把它注册给核心库；不存在（没编过 `scripts/build-vision.sh`）就什么都不注册，系统格式不提供。
    pub fn register(bin: PathBuf) {
        if cfg!(target_os = "macos") && bin.exists() {
            fg_core::platform::set_images(Box::new(Helper { bin, server: Mutex::new(None) }));
        }
    }

    /// 常驻解码进程；没有或死了就起一个。`FG_IMAGE_DECODE=0` 关掉这条路（调性能、排查用）。
    fn server(&self) -> Option<Arc<Server>> {
        if std::env::var("FG_IMAGE_DECODE").map_or(false, |v| v == "0") {
            return None;
        }
        let mut g = self.server.lock().unwrap_or_else(|e| e.into_inner());
        if g.as_ref().map_or(true, |s| s.dead.load(Ordering::Relaxed)) {
            // 同时解的张数跟分析线程数走（核数），每条线程的请求都不用排队
            let par = std::env::var("FG_IMAGE_PAR").ok().and_then(|v| v.parse().ok()).unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));
            *g = Server::spawn(&self.bin, par);
        }
        g.clone()
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

    /// 颜色不转 sRGB（`image` crate 也不管色彩配置），CLIP 分才和老分析接得上。ImageIO 在 DCT 域直接缩到 1024 的图比老路（整张解码后
    /// 双线性缩）略锐，清晰度分系统性高约 0.07（中位 0.066、p95 0.11）；`FG_IMAGE_OVERSAMPLE=2` 让边车解到两倍大、这里再用
    /// `fast_resize` 缩，漂移降到 0.02 以内，但管道字节翻四倍、边车 CPU 多一成，整条流水线的提速就没了（量过：60 张重图 57–65 对 68–72 ms/张，
    /// 老路 63–76），所以默认 1。`FG_IMAGE_MANAGED=1` 转 sRGB，对比用。
    fn decode(&self, src: &Path, max: u32) -> Option<(DynamicImage, u32, u32)> {
        let over: u32 = std::env::var("FG_IMAGE_OVERSAMPLE").ok().and_then(|v| v.parse().ok()).unwrap_or(1).clamp(1, 4);
        let managed = std::env::var("FG_IMAGE_MANAGED").map_or(false, |v| v == "1");
        let (img, ow, oh) = self.server()?.decode(src, max.saturating_mul(over), managed)?;
        Some((if over > 1 { fg_core::analyze::fast_resize(&img, max) } else { img }, ow, oh))
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

    /// 要有编好的边车（`bin/fg-image`），手动跑：`cargo test -p fleetinggleam --lib imageio_macos -- --ignored`。
    #[test]
    #[ignore]
    fn the_resident_decoder_scales_upright_and_survives_a_bad_file() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let h = Helper { bin: root.join("bin/fg-image"), server: Mutex::new(None) };
        let dir = std::env::temp_dir().join(format!("fg_imageio_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // 3000x2000 的渐变，缩到长边 1024 应得 1024x683（和核心库 fast_resize 的取整一样）
        let img = image::RgbImage::from_fn(3000, 2000, |x, y| image::Rgb([(x / 12) as u8, (y / 8) as u8, 90]));
        let good = dir.join("wide.jpg");
        img.save(&good).unwrap();
        let (small, ow, oh) = h.decode(&good, 1024).expect("decoded by the resident helper");
        assert_eq!((ow, oh), (3000, 2000));
        assert_eq!((small.width(), small.height()), (1024, 683));
        // 左上角偏暗、右下角偏亮：方向没有翻转
        let (tl, br) = (small.to_rgb8()[(10, 10)], small.to_rgb8()[(1000, 670)]);
        assert!(tl[0] < br[0] && tl[1] < br[1]);
        // 比原图小的照片原样给出
        let (same, _, _) = h.decode(&good, 4000).unwrap();
        assert_eq!((same.width(), same.height()), (3000, 2000));
        // 坏文件只是这张没有，边车还活着
        std::fs::write(dir.join("junk.jpg"), b"not a jpeg").unwrap();
        assert!(h.decode(&dir.join("junk.jpg"), 1024).is_none());
        assert!(h.decode(&dir.join("missing.jpg"), 1024).is_none());
        assert!(h.decode(&good, 512).is_some(), "still answering after refusing two files");
        // 几条线程同时问，各得其所
        std::thread::scope(|s| {
            for _ in 0..6 {
                s.spawn(|| assert_eq!(h.decode(&good, 256).unwrap().0.width(), 256));
            }
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn raw_and_modern_containers_are_the_helpers() {
        let h = Helper { bin: PathBuf::from("/nonexistent"), server: Mutex::new(None) };
        assert!(h.supports("dng") && h.supports("heic") && h.supports("jxl"));
        assert!(!h.supports("jpg") && !h.supports("txt"));
    }
}
