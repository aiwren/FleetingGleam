//! Eye openness from Google's MediaPipe face landmark and blendshape models, as an alternative to the geometric ratio
//! computed from Apple Vision's own landmarks (`scoring::EYES_CLOSED`). Where Vision finds the faces (its detector is
//! already proven in this app), these two small models look closely at each one: a 478-point mesh, then a classifier
//! trained to read expression from that mesh, including how closed each eye is. Converted from Google's own
//! `face_landmarker.task` (Apache-2.0); see `docs/` for the conversion notes.
//!
//! A face crop the mesh model itself is unsure about (a hard angle, a hand across the face) is skipped rather than
//! trusted: its blink score would be noise, not signal.

use std::path::Path;
use std::sync::Mutex;

use image::imageops::FilterType;
use image::DynamicImage;
use ort::session::Session;
use ort::value::Tensor;

use crate::modelfiles;

pub const LANDMARKS_FILE: &str = "face_landmarks.onnx";
pub const BLENDSHAPES_FILE: &str = "face_blendshapes.onnx";

const CROP: u32 = 256;
/// How far the face box is expanded (centred, both axes) before cropping: the same 1.5x MediaPipe's own pipeline
/// uses to go from a detected face to the region its landmark model was trained on.
const ROI_SCALE: f32 = 1.5;
/// Below this the landmark model itself is not confident there is a well-posed face in the crop; its blink read is
/// not trusted at that point, so the face is left out rather than mixed in.
const CONFIDENCE_MIN: f32 = 0.5;

/// The 146 of the mesh's 478 points the blendshape classifier was trained on, in this exact order.
/// `mediapipe/tasks/cc/vision/face_landmarker/face_blendshapes_graph.cc`, `kLandmarksSubsetIdxs`.
const SUBSET: [usize; 146] = [
    0, 1, 4, 5, 6, 7, 8, 10, 13, 14, 17, 21, 33, 37, 39, 40, 46, 52, 53, 54, 55, 58, 61, 63, 65, 66, 67, 70, 78, 80, 81, 82, 84, 87, 88, 91,
    93, 95, 103, 105, 107, 109, 127, 132, 133, 136, 144, 145, 146, 148, 149, 150, 152, 153, 154, 155, 157, 158, 159, 160, 161, 162, 163, 168,
    172, 173, 176, 178, 181, 185, 191, 195, 197, 234, 246, 249, 251, 263, 267, 269, 270, 276, 282, 283, 284, 285, 288, 291, 293, 295, 296, 297,
    300, 308, 310, 311, 312, 314, 317, 318, 321, 323, 324, 332, 334, 336, 338, 356, 361, 362, 365, 373, 374, 375, 377, 378, 379, 380, 381, 382,
    384, 385, 386, 387, 388, 389, 390, 397, 398, 400, 402, 405, 409, 415, 454, 466, 468, 469, 470, 471, 472, 473, 474, 475, 476, 477,
];
/// Index of `eyeBlinkLeft` and `eyeBlinkRight` among the 52 blendshapes, in the order the model was trained on
/// (`kBlendshapeNames` in the same source file). We only ever read these two.
const BLINK_LEFT: usize = 9;
const BLINK_RIGHT: usize = 10;

pub fn available(dir: &Path) -> bool {
    modelfiles::has_model(dir, LANDMARKS_FILE) && modelfiles::has_model(dir, BLENDSHAPES_FILE)
}

pub struct MediapipeEyes {
    landmarks: Mutex<Session>,
    blendshapes: Mutex<Session>,
}

impl MediapipeEyes {
    pub fn load(dir: &Path) -> Result<Self, String> {
        let build = |file: &str| -> Result<Session, String> {
            let mut b = Session::builder().map_err(|e| e.to_string())?;
            b.commit_from_file(dir.join(file)).map_err(|e| e.to_string())
        };
        Ok(Self { landmarks: Mutex::new(build(LANDMARKS_FILE)?), blendshapes: Mutex::new(build(BLENDSHAPES_FILE)?) })
    }

    /// Openness of the most closed eye among the given faces (1.0 open .. 0.0 closed), matching the scale
    /// `scoring::MEDIAPIPE_EYES_CLOSED` is calibrated against. `boxes` are `[x, y, w, h]`, top-left origin, normalised
    /// to `img`, one per face (see `vision::VisionOut::eye_boxes`). `None` when no face was confident enough to read.
    pub fn openness(&self, img: &DynamicImage, boxes: &[[f32; 4]]) -> Option<f32> {
        boxes.iter().filter_map(|b| self.face_openness(img, *b)).reduce(f32::min)
    }

    fn face_openness(&self, img: &DynamicImage, [x, y, w, h]: [f32; 4]) -> Option<f32> {
        let (iw, ih) = (img.width() as f32, img.height() as f32);
        let side = (w.max(h) * ROI_SCALE).min(1.0);
        let x0 = (x + w / 2.0 - side / 2.0).clamp(0.0, 1.0 - side);
        let y0 = (y + h / 2.0 - side / 2.0).clamp(0.0, 1.0 - side);
        // Pixels, in `img`'s own frame: this crop's offset and size are what turns the mesh's local 0..1 coordinates
        // back into positions the blendshape model expects (it was trained on landmarks scaled by the *frame* size).
        let px0 = (x0 * iw).round().clamp(0.0, iw - 1.0) as u32;
        let py0 = (y0 * ih).round().clamp(0.0, ih - 1.0) as u32;
        let pw = ((side * iw).round().max(1.0) as u32).min(img.width() - px0);
        let ph = ((side * ih).round().max(1.0) as u32).min(img.height() - py0);
        let crop = img.crop_imm(px0, py0, pw, ph);
        let face = crop.resize_exact(CROP, CROP, FilterType::Triangle).to_rgb8();

        let mut data = vec![0f32; (CROP * CROP * 3) as usize];
        for (i, p) in face.pixels().enumerate() {
            for c in 0..3 {
                data[i * 3 + c] = p[c] as f32 / 255.0;
            }
        }
        let input = Tensor::from_array(([1usize, CROP as usize, CROP as usize, 3usize], data)).ok()?;
        let (mesh, confidence) = {
            let mut s = self.landmarks.lock().ok()?;
            let out = s.run(ort::inputs!["input_12" => input]).ok()?;
            let (_, points) = out["Identity"].try_extract_tensor::<f32>().ok()?;
            let (_, flag) = out["Identity_1"].try_extract_tensor::<f32>().ok()?;
            (points.to_vec(), *flag.first()?)
        };
        // A raw logit: the model's own confidence there is a well-posed face here, before the classifier is asked anything.
        if sigmoid(confidence) < CONFIDENCE_MIN || mesh.len() < 478 * 3 {
            return None;
        }

        // Local 0..1 mesh coordinates, the 146-point subset, projected back into this frame's pixel space.
        let mut points = vec![0f32; 146 * 2];
        for (i, &idx) in SUBSET.iter().enumerate() {
            let (lx, ly) = (mesh[idx * 3], mesh[idx * 3 + 1]);
            points[i * 2] = px0 as f32 + lx * pw as f32;
            points[i * 2 + 1] = py0 as f32 + ly * ph as f32;
        }
        let input = Tensor::from_array(([1usize, 146usize, 2usize], points)).ok()?;
        let blink = {
            let mut s = self.blendshapes.lock().ok()?;
            let out = s.run(ort::inputs!["serving_default_input_points:0" => input]).ok()?;
            let (_, scores) = out["StatefulPartitionedCall:0"].try_extract_tensor::<f32>().ok()?;
            scores[BLINK_LEFT].max(scores[BLINK_RIGHT])
        };
        Some((1.0 - blink).clamp(0.0, 1.0))
    }
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "needs models/ and a real photo (set PROBE_IMG); prints its own openness, no assertion on a value we cannot pin down"]
    fn eyes_on_a_real_photo() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../models");
        let m = MediapipeEyes::load(&dir).expect("load");
        let path = std::env::var("PROBE_IMG").expect("set PROBE_IMG=<path> and PROBE_BOX=x,y,w,h (from fg-vision's eyeBoxes)");
        let img = image::open(&path).expect("open");
        let box_str = std::env::var("PROBE_BOX").expect("PROBE_BOX=x,y,w,h");
        let parts: Vec<f32> = box_str.split(',').map(|s| s.trim().parse().unwrap()).collect();
        let b = [parts[0], parts[1], parts[2], parts[3]];
        println!("openness = {:?}", m.openness(&img, &[b]));
    }
}
