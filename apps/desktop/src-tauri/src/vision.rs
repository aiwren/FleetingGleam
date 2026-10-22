//! Apple Vision 边车（macOS）的客户端：核心库 `platform::SystemVision` 在桌面端的实现。
//! 一切都能退化：没有边车二进制、不是 macOS、边车崩了，都只是"没有这路信号"，分析照常。
//!
//! 协议见 `vision/fg-vision.swift`：请求带 id，像素直接走管道（分析线程内存里那张 1024px 的 RGB 副本，不再写 JPEG 到盘、
//! 边车再读回），回复可以乱序；一个边车进程内同时处理几个请求，所以通常只起一个进程、多条线程一起往里送。

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fg_core::platform::{SystemVision, VisionOut};
use image::DynamicImage;
use serde::Deserialize;

/// 到时还没答的请求就放弃：边车杀掉重起，这张照片没有信号。Vision 正常远不到一秒就答；这个上限只在神经引擎那条路卡死时起作用。
/// 边车里几个请求同时做、后面的还要排队，所以比单张的耗时宽裕得多。
const TIMEOUT: Duration = Duration::from_secs(20);
/// 一次分析里边车最多重起几次，再多就不要它了。
const MAX_RESTARTS: usize = 8;

#[derive(Deserialize)]
struct Reply {
    ok: bool,
    aesthetic: Option<f32>,
    utility: Option<bool>,
    faces: Option<Vec<f32>>,
    eyes: Option<Vec<f32>>,
    #[serde(rename = "eyeBoxes")]
    eye_boxes: Option<Vec<[f32; 4]>>,
    #[serde(rename = "faceMarks")]
    face_marks: Option<Vec<[f32; 14]>>,
}

#[derive(Deserialize)]
struct Envelope {
    id: u64,
    results: Vec<Reply>,
}

impl Reply {
    fn into_out(self) -> Option<VisionOut> {
        if !self.ok {
            return None;
        }
        Some(VisionOut {
            eyes_mediapipe: false,
            aesthetic: Some(self.aesthetic?),
            utility: self.utility.unwrap_or(false),
            faces: self.faces.unwrap_or_default(),
            eyes: self.eyes.unwrap_or_default(),
            eye_boxes: self.eye_boxes.unwrap_or_default(),
            face_marks: self.face_marks.unwrap_or_default().iter().map(fg_core::faces::FaceMark::from_row).collect(),
        })
    }
}

/// 一次请求：一张文件里的照片，或内存里的几张像素。
enum Request<'a> {
    Path(&'a Path),
    Pixels(Vec<&'a image::RgbImage>),
}

/// 一个边车进程。几条分析线程共用：写请求时锁一下 stdin，回复由一条读线程按 id 分发给等着的线程。
struct Proc {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    /// 还没收到回复的请求：id → 把结果交给等待方的通道。
    pending: Mutex<HashMap<u64, SyncSender<Vec<Option<VisionOut>>>>>,
    next_id: AtomicU64,
    /// 边车不再答话（被杀、崩了、关了管道）就置位；下一个请求会重起一个。
    dead: AtomicBool,
}

impl Proc {
    fn spawn(bin: &Path, parallel: usize, cpu_only: bool) -> Option<Arc<Proc>> {
        let mut cmd = Command::new(bin);
        cmd.arg("--par").arg(parallel.max(1).to_string());
        if cpu_only {
            cmd.arg("--cpu");
        }
        // 调性能时（FG_VISION_TRACE=1）让边车的 stderr 透出来
        let stderr = if std::env::var("FG_VISION_TRACE").map_or(false, |v| v == "1") { Stdio::inherit() } else { Stdio::null() };
        let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(stderr).spawn().ok()?;
        let stdin = child.stdin.take()?;
        let out = BufReader::new(child.stdout.take()?);
        let proc_ = Arc::new(Proc { child: Mutex::new(child), stdin: Mutex::new(stdin), pending: Mutex::new(HashMap::new()), next_id: AtomicU64::new(1), dead: AtomicBool::new(false) });
        let weak = Arc::downgrade(&proc_);
        std::thread::spawn(move || {
            for line in out.lines() {
                let Ok(line) = line else { break };
                let Some(p) = weak.upgrade() else { return };
                if let Ok(env) = serde_json::from_str::<Envelope>(&line) {
                    let tx = p.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&env.id);
                    if let Some(tx) = tx {
                        let _ = tx.try_send(env.results.into_iter().map(Reply::into_out).collect());
                    }
                }
            }
            // 管道断了：还在等的都等不到了（丢掉发送端，接收方立刻醒来拿到"没有"）
            if let Some(p) = weak.upgrade() {
                p.dead.store(true, Ordering::Relaxed);
                p.pending.lock().unwrap_or_else(|e| e.into_inner()).clear();
            }
        });
        Some(proc_)
    }

    fn kill(&self) {
        self.dead.store(true, Ordering::Relaxed);
        if let Ok(mut c) = self.child.lock() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    /// 发一个请求并等它的回复；每张一个结果，边车答不上的那张是 None。整个请求没回来（超时、边车没了）也是 None。
    fn ask(&self, req: &Request) -> Option<Vec<Option<VisionOut>>> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).insert(id, tx);
        let written = {
            let mut w = self.stdin.lock().unwrap_or_else(|e| e.into_inner());
            let r = match req {
                Request::Path(p) => writeln!(w, "{}", serde_json::json!({ "id": id, "path": p.to_string_lossy() })),
                Request::Pixels(imgs) => {
                    let dims: Vec<[u32; 2]> = imgs.iter().map(|i| [i.width(), i.height()]).collect();
                    writeln!(w, "{}", serde_json::json!({ "id": id, "imgs": dims })).and_then(|_| imgs.iter().try_for_each(|i| w.write_all(i.as_raw())))
                }
            };
            r.and_then(|_| w.flush()).is_ok()
        };
        if !written {
            self.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
            self.dead.store(true, Ordering::Relaxed);
            return None;
        }
        match rx.recv_timeout(TIMEOUT) {
            Ok(results) => Some(results),
            Err(RecvTimeoutError::Disconnected) => None,
            Err(RecvTimeoutError::Timeout) => {
                // 卡住了：之后就算答了也对不上号，整个边车重起
                self.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
                self.kill();
                None
            }
        }
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        self.kill();
    }
}

pub struct Vision {
    bin: PathBuf,
    cpu_only: bool,
    /// 每个边车进程内同时做几个请求。
    parallel: usize,
    /// 边车进程；一个槽位崩太多次后是 None。
    procs: Vec<Mutex<Option<Arc<Proc>>>>,
    /// 请求轮流交给各个进程。
    turn: AtomicUsize,
    restarts: AtomicUsize,
}

impl Vision {
    /// 起 `workers` 路并发的 Vision：`procs` 个进程，每个进程内 `workers / procs` 路。`procs` 为 1 时一个进程里全并发
    /// （量过：一个进程里四条线程的吞吐是一条的三四倍，和几个进程各一条相当，但不用来回切进程）。
    pub fn start_with(bin: &Path, procs: usize, workers: usize, cpu_only: bool) -> Option<Self> {
        if !cfg!(target_os = "macos") || !bin.exists() {
            return None;
        }
        let procs = procs.max(1);
        let parallel = (workers / procs).max(1);
        let procs: Vec<_> = (0..procs).filter_map(|_| Proc::spawn(bin, parallel, cpu_only)).map(|p| Mutex::new(Some(p))).collect();
        (!procs.is_empty()).then_some(Self { bin: bin.to_path_buf(), cpu_only, parallel, procs, turn: AtomicUsize::new(0), restarts: AtomicUsize::new(0) })
    }

    /// 一个边车进程、`workers` 路并发。
    pub fn start(bin: &Path, workers: usize, cpu_only: bool) -> Option<Self> {
        Self::start_with(bin, 1, workers, cpu_only)
    }

    /// 下一个该用的进程；死了就重起一个（次数有限），这个槽位放弃了就试下一个。
    fn pick(&self) -> Option<Arc<Proc>> {
        let n = self.procs.len();
        let first = self.turn.fetch_add(1, Ordering::Relaxed) % n;
        for k in 0..n {
            let mut slot = self.procs[(first + k) % n].lock().unwrap_or_else(|e| e.into_inner());
            if slot.as_ref().map_or(true, |p| p.dead.load(Ordering::Relaxed)) {
                if self.restarts.fetch_add(1, Ordering::Relaxed) >= MAX_RESTARTS {
                    *slot = None;
                    continue;
                }
                *slot = Proc::spawn(&self.bin, self.parallel, self.cpu_only);
            }
            if let Some(p) = slot.as_ref() {
                return Some(p.clone());
            }
        }
        None
    }

    fn ask(&self, req: &Request) -> Option<Vec<Option<VisionOut>>> {
        self.pick()?.ask(req)
    }

    pub fn analyze(&self, path: &Path) -> Option<VisionOut> {
        self.ask(&Request::Path(path))?.into_iter().next().flatten()
    }

    /// 内存里的像素：几张一起送（一次请求），边车内部并行做；每张一个结果。
    pub fn analyze_many(&self, imgs: &[&image::RgbImage]) -> Vec<Option<VisionOut>> {
        // 没有像素的"图"不送（协议里一张图至少一个像素）
        if imgs.is_empty() || imgs.iter().any(|i| i.width() == 0 || i.height() == 0) {
            return (0..imgs.len()).map(|_| None).collect();
        }
        self.ask(&Request::Pixels(imgs.to_vec())).unwrap_or_else(|| (0..imgs.len()).map(|_| None).collect())
    }
}

/// 注册给核心库的工厂：每次分析起 Apple Vision 边车（`bin/fg-vision`）；边车不在（没编过 `scripts/build-vision.sh`）就没有视觉信号。
pub struct Factory {
    pub bin: PathBuf,
}

impl fg_core::platform::VisionFactory for Factory {
    fn start(&self, _model_dir: &Path, workers: usize, cpu_only: bool) -> Option<Box<dyn SystemVision>> {
        let procs = std::env::var("FG_VISION_PROCS").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
        Vision::start_with(&self.bin, procs, workers, cpu_only).map(|v| Box::new(v) as Box<dyn SystemVision>)
    }
}

impl SystemVision for Vision {
    fn analyze(&self, path: &Path) -> Option<VisionOut> {
        Vision::analyze(self, path)
    }

    fn takes_pixels(&self) -> bool {
        true
    }

    fn analyze_pixels(&self, img: &DynamicImage) -> Option<VisionOut> {
        // 分析副本本来就是 RGB8（`fast_resize` 的输出）；别的格式（退回 `image::resize` 的那条路）转一下
        let out = match img.as_rgb8() {
            Some(rgb) => self.analyze_many(&[rgb]),
            None => self.analyze_many(&[&img.to_rgb8()]),
        };
        out.into_iter().next().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 要有编好的边车（`bin/fg-vision`），手动跑：`cargo test -p fleetinggleam --lib vision -- --ignored --nocapture`。
    #[test]
    #[ignore]
    fn the_helper_answers_from_a_file_and_from_pixels_and_keeps_in_step() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let v = Vision::start(&root.join("bin/fg-vision"), 4, false).expect("fg-vision (macOS only)");
        let dir = std::env::temp_dir().join(format!("fg_vision_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let img = image::RgbImage::from_fn(640, 480, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]));
        let good = dir.join("plain.jpg");
        img.save(&good).unwrap();
        let from_file = v.analyze(&good).expect("a plain photo gets an answer from its file");
        let from_pixels = v.analyze_pixels(&DynamicImage::ImageRgb8(img.clone())).expect("and from its pixels");
        // 合成的渐变图经 JPEG 一压，Vision 的美学分会变很多（这张图上两条路差 1 以上），这里只看两条路都有答案、都没看见脸；
        // 真实照片上两条路的对比见 bench.rs 的 `vision_pixels_match_the_file_path`
        assert_eq!(from_file.faces.len(), 0);
        assert_eq!(from_pixels.faces.len(), 0);
        assert!(from_file.aesthetic.is_some() && from_pixels.aesthetic.is_some());
        // 路径里的换行以前会错位后面所有回复；现在请求是 JSON，照常回答
        let odd = dir.join("line\nbreak.jpg");
        img.save(&odd).unwrap();
        assert!(v.analyze(&odd).is_some(), "a path with a line break is sent as JSON and answered");
        assert!(v.analyze(&good).is_some(), "and the helper still answers in step afterwards");
        // 一次请求几张、几条线程同时问，都各得其所
        let many = v.analyze_many(&[&img, &img, &img]);
        assert_eq!(many.len(), 3);
        assert!(many.iter().all(|o| o.is_some()));
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| assert!(v.analyze_pixels(&DynamicImage::ImageRgb8(img.clone())).is_some()));
            }
        });
        // 像素数对不上的请求：边车答 ok=false，这张就是 None，但进程还活着
        let bad = image::RgbImage::new(0, 0);
        assert!(v.analyze_pixels(&DynamicImage::ImageRgb8(bad)).is_none());
        assert!(v.analyze(&good).is_some(), "a refused picture does not take the helper down");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
