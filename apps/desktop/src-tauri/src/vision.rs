//! Optional platform signals from a helper process (Apple Vision on macOS): the desktop's `platform::SystemVision`.
//! Everything degrades gracefully: no binary, wrong OS or a crashed helper just means "no signal".

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Mutex;
use std::time::Duration;

/// A photo the helper has not answered for by then is given up on: the helper is killed and started afresh, and that photo
/// gets no signal. Vision normally answers in well under a second; the limit only matters when the Neural Engine path hangs.
const TIMEOUT: Duration = Duration::from_secs(20);
/// How many times the helpers may be restarted in one scan before the scan goes on without them.
const MAX_RESTARTS: usize = 8;

use fg_core::platform::{SystemVision, VisionOut};
use serde::Deserialize;


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

struct Proc {
    child: Child,
    stdin: ChildStdin,
    /// Replies, one line each, read by a thread so that waiting for one can have a deadline.
    replies: Receiver<String>,
    /// Set once the helper stopped answering (killed, crashed or closed its pipe); the slot is restarted on the next request.
    dead: bool,
}

impl Proc {
    fn spawn(bin: &Path, cpu_only: bool) -> Option<Proc> {
        let mut cmd = Command::new(bin);
        if cpu_only {
            cmd.arg("--cpu");
        }
        let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
        let stdin = child.stdin.take()?;
        let out = BufReader::new(child.stdout.take()?);
        let (tx, replies) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in out.lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Some(Proc { child, stdin, replies, dead: false })
    }

    fn ask(&mut self, path: &Path) -> Option<VisionOut> {
        // the protocol is one line per request: a path with a line break in it would be read as two requests and every reply
        // after it would belong to the photo before (macOS allows such names; they get no signal)
        let shown = path.display().to_string();
        if shown.contains(['\n', '\r']) {
            return None;
        }
        if writeln!(self.stdin, "{shown}").and_then(|_| self.stdin.flush()).is_err() {
            self.dead = true;
            return None;
        }
        let line = match self.replies.recv_timeout(TIMEOUT) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => {
                // hung or gone: a later reply, if any ever came, would be out of step with the next request
                self.dead = true;
                let _ = self.child.kill();
                let _ = self.child.wait();
                return None;
            }
        };
        let r: Reply = serde_json::from_str(&line).ok()?;
        if !r.ok {
            return None;
        }
        Some(VisionOut {
            aesthetic: r.aesthetic?,
            utility: r.utility.unwrap_or(false),
            faces: r.faces.unwrap_or_default(),
            eyes: r.eyes.unwrap_or_default(),
            eye_boxes: r.eye_boxes.unwrap_or_default(),
            face_marks: r.face_marks.unwrap_or_default().iter().map(fg_core::faces::FaceMark::from_row).collect(),
        })
    }
}

pub struct Vision {
    bin: PathBuf,
    cpu_only: bool,
    /// One helper per worker; `None` once a slot gave up (crashed too often).
    procs: Vec<Mutex<Option<Proc>>>,
    restarts: AtomicUsize,
}

impl Vision {
    pub fn start(bin: &Path, workers: usize, cpu_only: bool) -> Option<Self> {
        if !cfg!(target_os = "macos") || !bin.exists() {
            return None;
        }
        let procs: Vec<_> = (0..workers).filter_map(|_| Proc::spawn(bin, cpu_only)).map(|p| Mutex::new(Some(p))).collect();
        (!procs.is_empty()).then_some(Self { bin: bin.to_path_buf(), cpu_only, procs, restarts: AtomicUsize::new(0) })
    }

    /// Asks the helper in this slot, starting a fresh one first if the last one died; `None` when the slot has given up.
    fn ask_slot(&self, slot: &mut Option<Proc>, path: &Path) -> Option<VisionOut> {
        if slot.as_ref().map_or(true, |p| p.dead) {
            if self.restarts.fetch_add(1, Ordering::Relaxed) >= MAX_RESTARTS {
                *slot = None;
                return None;
            }
            *slot = Proc::spawn(&self.bin, self.cpu_only);
        }
        slot.as_mut()?.ask(path)
    }

    pub fn analyze(&self, path: &Path) -> Option<VisionOut> {
        for p in &self.procs {
            if let Ok(mut g) = p.try_lock() {
                return self.ask_slot(&mut g, path);
            }
        }
        let mut g = self.procs[0].lock().ok()?;
        self.ask_slot(&mut g, path)
    }
}

impl SystemVision for Vision {
    fn analyze(&self, path: &Path) -> Option<VisionOut> {
        Vision::analyze(self, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs the built helper (`bin/fg-vision`), so it is run by hand: `cargo test --lib vision -- --ignored`.
    #[test]
    #[ignore]
    fn the_helper_answers_and_a_path_with_a_line_break_does_not_derail_it() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let v = Vision::start(&root.join("bin/fg-vision"), 1, false).expect("fg-vision (macOS only)");
        let dir = std::env::temp_dir().join(format!("fg_vision_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let img = image::RgbImage::from_fn(640, 480, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]));
        let good = dir.join("plain.jpg");
        img.save(&good).unwrap();
        assert!(v.analyze(&good).is_some(), "a plain photo gets an answer");
        let odd = dir.join("line\nbreak.jpg");
        img.save(&odd).unwrap();
        assert!(v.analyze(&odd).is_none(), "a path with a line break is refused rather than sent");
        assert!(v.analyze(&good).is_some(), "and the helper still answers in step afterwards");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

impl Drop for Vision {
    fn drop(&mut self) {
        for p in &self.procs {
            if let Ok(mut g) = p.lock() {
                if let Some(proc_) = g.as_mut() {
                    let _ = proc_.child.kill();
                    let _ = proc_.child.wait();
                }
            }
        }
    }
}
