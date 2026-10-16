//! Optional platform signals from a helper process (Apple Vision on macOS).
//! Everything degrades gracefully: no binary, wrong OS or a crashed helper just means "no signal".

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

use serde::Deserialize;

pub struct VisionOut {
    /// Apple's overallScore, roughly -1..1.
    pub aesthetic: f32,
    pub utility: bool,
    /// One capture-quality value (0..1) per detected face.
    pub faces: Vec<f32>,
    /// One eye-openness value per face whose eyes could be located (about 0.25-0.4 open, under 0.15 closed).
    pub eyes: Vec<f32>,
    /// Same faces, same order as `eyes`: `[x, y, w, h]`, top-left origin, normalised to the image handed to `analyze`.
    /// The crop a learned model (`mediapipe.rs`) looks at, when one is asked to look instead of the geometric ratio.
    pub eye_boxes: Vec<[f32; 4]>,
    /// Every face with a full set of landmarks, for telling people apart (see `faces.rs`).
    pub face_marks: Vec<crate::faces::FaceMark>,
}

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
    out: BufReader<ChildStdout>,
}

impl Proc {
    fn ask(&mut self, path: &Path) -> Option<VisionOut> {
        writeln!(self.stdin, "{}", path.display()).ok()?;
        self.stdin.flush().ok()?;
        let mut line = String::new();
        if self.out.read_line(&mut line).ok()? == 0 {
            return None;
        }
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
            face_marks: r.face_marks.unwrap_or_default().iter().map(crate::faces::FaceMark::from_row).collect(),
        })
    }
}

pub struct Vision {
    procs: Vec<Mutex<Proc>>,
}

impl Vision {
    pub fn start(bin: &Path, workers: usize, cpu_only: bool) -> Option<Self> {
        if !cfg!(target_os = "macos") || !bin.exists() {
            return None;
        }
        let procs: Vec<_> = (0..workers)
            .filter_map(|_| {
                let mut cmd = Command::new(bin);
                if cpu_only {
                    cmd.arg("--cpu");
                }
                let mut child = cmd
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                    .ok()?;
                let stdin = child.stdin.take()?;
                let out = BufReader::new(child.stdout.take()?);
                Some(Mutex::new(Proc { child, stdin, out }))
            })
            .collect();
        (!procs.is_empty()).then_some(Self { procs })
    }

    pub fn analyze(&self, path: &Path) -> Option<VisionOut> {
        for p in &self.procs {
            if let Ok(mut g) = p.try_lock() {
                return g.ask(path);
            }
        }
        self.procs[0].lock().ok()?.ask(path)
    }
}

impl Drop for Vision {
    fn drop(&mut self) {
        for p in &self.procs {
            if let Ok(mut g) = p.lock() {
                let _ = g.child.kill();
            }
        }
    }
}
