//! 没有系统人脸检测的平台（Android）上的人脸流水线，作为核心库自己的 `platform::SystemVision` 实现：
//! 一个很小的检测器（UltraFace RFB-640，1.5 MB，`detect` 包）找脸，再由 MediaPipe 的网格模型（`eyes` 包）对每张脸量眼睛、
//! 定出人物识别要的五个点。给出的 `VisionOut` 和 Apple Vision 的形状一样，分析流水线不用区分来源；只有 Apple 的美学分这里没有。
//!
//! 路线图 A4 说的"人脸统一用 MediaPipe"：检测器本该也是 MediaPipe 的 BlazeFace，但它只有 TFLite，转 ONNX 要 tf2onnx；
//! UltraFace 是现成的 ONNX（MIT），先用它，以后换检测器只动这个文件和清单。

use std::path::Path;
use crate::onnx::Pool;

use image::imageops::FilterType;
use image::DynamicImage;
use ort::value::Tensor;

use crate::faces::FaceMark;
use crate::mediapipe::MediapipeEyes;
use crate::modelfiles;
use crate::platform::{SystemVision, VisionOut};

pub const DETECTOR_FILE: &str = "face_detector_rfb640.onnx";
/// 检测器的输入尺寸（宽 x 高）。
const DET_W: u32 = 640;
const DET_H: u32 = 480;
/// 低于这个分的候选框不算脸。
const SCORE_MIN: f32 = 0.7;
/// 非极大值抑制：和已选的框重叠超过这个比例的去掉。
const NMS_IOU: f32 = 0.3;
/// 一张照片最多看这么多张脸（按分数取）。
const MAX_FACES: usize = 24;
/// UltraFace 的框比 Apple Vision 的松（多出额头和两颊），而 MediaPipe 的网格与眨眼模型对框很敏感：框越松，睁眼值越低、网格越没把握。
/// 在作者的 80 张已由 Mac 端分析过的照片上对比（`agreement_with_apple_vision_on_a_real_cache`）：原框下"闭眼"（< 0.5）占 44%，
/// 收到 0.85 后占 17%，与 Mac 端 Vision 框下的 16% 一致，人脸数 96% 一致。
const BOX_TIGHTEN: f32 = 0.85;

pub fn available(dir: &Path) -> bool {
    modelfiles::has_model(dir, DETECTOR_FILE) && crate::mediapipe::available(dir)
}

/// 人脸检测器：UltraFace。输出已经解码成相对坐标的角点框。
pub struct FaceDetector {
    session: Pool,
}

impl FaceDetector {
    pub fn load(dir: &Path) -> Result<Self, String> {
        Ok(Self { session: Pool::load(&dir.join(DETECTOR_FILE), crate::onnx::small_copies(), Some(2))? })
    }

    /// 脸框 `[x, y, w, h]`，相对 `img` 归一化、左上为原点，分数高的在前；每个框带它的分数。
    pub fn detect(&self, img: &DynamicImage) -> Vec<([f32; 4], f32)> {
        let rgb = img.resize_exact(DET_W, DET_H, FilterType::Triangle).to_rgb8();
        let plane = (DET_W * DET_H) as usize;
        let mut data = vec![0f32; 3 * plane];
        for (i, p) in rgb.pixels().enumerate() {
            for c in 0..3 {
                data[c * plane + i] = (p[c] as f32 - 127.0) / 128.0;
            }
        }
        let Ok(input) = Tensor::from_array(([1usize, 3, DET_H as usize, DET_W as usize], data)) else { return Vec::new() };
        let Some((scores, boxes)) = self.session.with(|s| {
            let out = s.run(ort::inputs![input]).ok()?;
            let (_, scores) = out[0].try_extract_tensor::<f32>().ok()?;
            let (_, boxes) = out[1].try_extract_tensor::<f32>().ok()?;
            Some((scores.to_vec(), boxes.to_vec()))
        }) else { return Vec::new() };
        let n = scores.len() / 2;
        let mut cands: Vec<([f32; 4], f32)> = (0..n)
            .filter(|&i| scores[i * 2 + 1] >= SCORE_MIN && boxes.len() >= (i + 1) * 4)
            .map(|i| {
                let b = &boxes[i * 4..i * 4 + 4];
                let (x1, y1, x2, y2) = (b[0].clamp(0.0, 1.0), b[1].clamp(0.0, 1.0), b[2].clamp(0.0, 1.0), b[3].clamp(0.0, 1.0));
                ([x1, y1, (x2 - x1).max(0.0), (y2 - y1).max(0.0)], scores[i * 2 + 1])
            })
            .filter(|(b, _)| b[2] > 0.0 && b[3] > 0.0)
            .collect();
        cands.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut kept: Vec<([f32; 4], f32)> = Vec::new();
        for c in cands {
            if kept.len() >= MAX_FACES {
                break;
            }
            if kept.iter().all(|k| iou(&k.0, &c.0) < NMS_IOU) {
                kept.push(c);
            }
        }
        kept
    }
}

/// 两个 `[x, y, w, h]` 框的交并比。
pub fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let x1 = a[0].max(b[0]);
    let y1 = a[1].max(b[1]);
    let x2 = (a[0] + a[2]).min(b[0] + b[2]);
    let y2 = (a[1] + a[3]).min(b[1] + b[3]);
    let inter = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let union = a[2] * a[3] + b[2] * b[3] - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}

/// 检测 + 网格，合起来就是一份"系统视觉信号"。
pub struct FacePipeline {
    detector: FaceDetector,
    mesh: MediapipeEyes,
}

impl FacePipeline {
    pub fn load(dir: &Path) -> Result<Self, String> {
        Ok(Self { detector: FaceDetector::load(dir)?, mesh: MediapipeEyes::load(dir)? })
    }

    pub fn analyze_image(&self, img: &DynamicImage) -> VisionOut {
        let mut out = VisionOut { aesthetic: None, utility: false, faces: Vec::new(), eyes: Vec::new(), eye_boxes: Vec::new(), face_marks: Vec::new(), eyes_mediapipe: true };
        for (bbox, _score) in self.detector.detect(img) {
            let bbox = [bbox[0] + bbox[2] * (1.0 - BOX_TIGHTEN) / 2.0, bbox[1] + bbox[3] * (1.0 - BOX_TIGHTEN) / 2.0, bbox[2] * BOX_TIGHTEN, bbox[3] * BOX_TIGHTEN];
            // 网格模型不认的框（侧得太厉害、被挡住）不算脸：和 Vision 一样，只报有把握的
            let Some(mesh) = self.mesh.mesh(img, bbox) else { continue };
            out.faces.push(mesh.confidence);
            out.eye_boxes.push(bbox);
            // 眨眼读不出来时按"睁着"记，和 `analyze` 对没量到眼睛的脸的处理一致
            out.eyes.push(self.mesh.blink(&mesh).map_or(1.0, |b| (1.0 - b).clamp(0.0, 1.0)));
            out.face_marks.push(FaceMark { bbox, pts: mesh.five_points() });
        }
        out
    }
}

/// 注册给 `platform::set_vision` 的工厂：模型齐了（`detect` + `eyes` 包）就起一条流水线，否则没有视觉信号。
/// 流水线本身线程安全（会话各自加锁），`workers` 用不上。
pub struct Factory;

impl crate::platform::VisionFactory for Factory {
    fn start(&self, model_dir: &Path, _workers: usize, _cpu_only: bool) -> Option<Box<dyn SystemVision>> {
        if !available(model_dir) {
            return None;
        }
        FacePipeline::load(model_dir).map_err(|e| eprintln!("faces disabled: {e}")).ok().map(|p| Box::new(p) as Box<dyn SystemVision>)
    }
}

impl SystemVision for FacePipeline {
    fn analyze(&self, path: &Path) -> Option<VisionOut> {
        let img = image::open(path).ok()?;
        Some(self.analyze_image(&img))
    }

    /// 流水线在进程内，分析时直接看内存里的副本，省掉每张照片一次 JPEG 编码加解码。
    fn takes_pixels(&self) -> bool {
        true
    }

    fn analyze_pixels(&self, img: &DynamicImage) -> Option<VisionOut> {
        Some(self.analyze_image(img))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iou_of_boxes() {
        assert!((iou(&[0.0, 0.0, 1.0, 1.0], &[0.0, 0.0, 1.0, 1.0]) - 1.0).abs() < 1e-6);
        assert!((iou(&[0.0, 0.0, 1.0, 1.0], &[0.5, 0.0, 1.0, 1.0]) - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(iou(&[0.0, 0.0, 0.5, 0.5], &[0.5, 0.5, 0.5, 0.5]), 0.0);
    }

    /// 和 Apple Vision 的结果对一对：FG_CACHE=<analysis-cache.json>（Mac 端分析过的照片，缩略图在本机），抽 FG_N（默认 60）张，
    /// 看两边人脸数一致的比例和眼睛值的对照。只打印，不断言——两端一致是路线图阶段 B 第 6 步的事。
    #[test]
    #[ignore = "needs models/ (detect + eyes packs) and FG_CACHE"]
    fn agreement_with_apple_vision_on_a_real_cache() {
        let Ok(cache) = std::env::var("FG_CACHE") else { return };
        let n: usize = std::env::var("FG_N").ok().and_then(|v| v.parse().ok()).unwrap_or(60);
        let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(&cache).unwrap()).unwrap();
        let pipe = FacePipeline::load(&crate::dev_models_dir()).expect("load");
        let mut rows: Vec<(u32, Option<f32>, String)> = raw["entries"].as_object().unwrap().values()
            .filter_map(|e| Some((e["photo"]["face_count"].as_u64()? as u32, e["photo"]["eyes_open"].as_f64().map(|v| v as f32), e["photo"]["thumb"].as_str()?.to_string())))
            .filter(|(_, _, t)| Path::new(t).exists())
            .collect();
        // 每隔几张取一张，人脸多的少的都有
        let step = (rows.len() / n).max(1);
        rows = rows.into_iter().step_by(step).take(n).collect();
        let (mut same, mut both_faces, mut total) = (0, 0, 0);
        let mut diffs: Vec<String> = Vec::new();
        for (vision_faces, vision_eyes, thumb) in &rows {
            // 分析时交给视觉信号的是 1024 px 的分析副本（`.a.jpg`），不是 640 px 的缩略图；有副本就用副本
            let copy = thumb.replace(".jpg", ".a.jpg");
            let Ok(img) = image::open(if Path::new(&copy).exists() { &copy } else { thumb }) else { continue };
            let out = pipe.analyze_image(&img);
            total += 1;
            let ours = out.faces.len() as u32;
            if ours == *vision_faces { same += 1; } else { diffs.push(format!("vision {vision_faces} / ours {ours}")); }
            if ours > 0 && *vision_faces > 0 { both_faces += 1; }
            if ours > 0 {
                println!("{:<24} vision faces {vision_faces} eyes {:?} | ours faces {ours} eyes {:?} conf {:?}", Path::new(thumb).file_name().unwrap().to_string_lossy(), vision_eyes, out.eyes.iter().cloned().reduce(f32::min), out.faces.iter().cloned().reduce(f32::min));
            }
        }
        println!("{total} photos: same face count {same} ({:.0}%), both found faces {both_faces}; differences: {diffs:?}", 100.0 * same as f32 / total.max(1) as f32);
    }

    /// 真照片上整条流水线：FG_FACE_IMG=<一张有人的照片>（需要 models/ 里的 detect 与 eyes 包）。
    #[test]
    #[ignore = "needs models/ (detect + eyes packs) and FG_FACE_IMG"]
    fn faces_eyes_and_marks_on_a_real_photo() {
        let Ok(p) = std::env::var("FG_FACE_IMG") else { return };
        let pipe = FacePipeline::load(&crate::dev_models_dir()).expect("load");
        let img = image::open(&p).unwrap();
        let out = pipe.analyze_image(&img);
        println!("{} faces: confidence {:?} eyes {:?}", out.faces.len(), out.faces, out.eyes);
        for m in &out.face_marks {
            println!("  box {:?} pts {:?}", m.bbox, m.pts);
        }
        assert!(!out.faces.is_empty(), "a photo with people in it should have faces");
        assert_eq!(out.faces.len(), out.eyes.len());
        assert_eq!(out.faces.len(), out.face_marks.len());
        for m in &out.face_marks {
            // the five points sit inside the box (give or take the box's own looseness)
            for pt in m.pts {
                assert!(pt[0] >= m.bbox[0] - 0.05 && pt[0] <= m.bbox[0] + m.bbox[2] + 0.05, "{pt:?} in {:?}", m.bbox);
                assert!(pt[1] >= m.bbox[1] - 0.05 && pt[1] <= m.bbox[1] + m.bbox[3] + 0.05, "{pt:?} in {:?}", m.bbox);
            }
            // 画面左眼在画面右眼的左边，嘴在眼睛下面
            assert!(m.pts[0][0] < m.pts[1][0]);
            assert!(m.pts[3][1] > m.pts[0][1] && m.pts[4][1] > m.pts[1][1]);
        }
    }
}
