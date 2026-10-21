//! 两端一致性的固定用例（路线图阶段 B 第 6 步）。
//!
//! 一组由代码画出来的照片（几组连拍 + 几张各不相同的场景，每次都一模一样，不进仓库），按 Android 走的那条路分析——不注册任何平台能力
//! （没有 Apple Vision、没有系统解码器、没有 GPU），然后把每张照片的**分组、场景、保留 / 待复查 / 淘汰**和综合分记下来，
//! 和仓库里的 `consistency.golden.json` 比。同一份比对在 macOS（CI 的 `cargo test`）和 Android 真机 / 模拟器（`fleetinggleam-android`
//! 的测试，经 cargo-ndk 编成 arm64 跑）上都要通过——两端用的是同一份核心代码，差别只可能来自浮点与解码，这个用例就是盯着它的。
//!
//! 分组和场景的编号在不同运行里可能不同，所以记的是"这一组里名字最小的那张"；分数允许 ±0.02（不同 CPU 的浮点细节）。
//! 要更新基准：`FG_WRITE_GOLDEN=1 cargo test -p fleetinggleam-core consistency`，再把改动连同原因一起提交。

use crate::engine::{Engine, NoEvents, Paths, ScanArgs};
use image::{Rgb, RgbImage};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// 没有任何模型时的基准（CI 上就是这样）。
pub const GOLDEN: &str = include_str!("../consistency.golden.json");
/// 装了标准档模型（CLIP ViT-B/32 与美学头）时的基准：分组、场景、观感分都由模型决定，这才是两端真正要一致的那一份。
pub const GOLDEN_MODELS: &str = include_str!("../consistency.models.golden.json");

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Row {
    pub name: String,
    /// 同组里名字最小的那张（只有一张的组就是它自己）。
    pub group: String,
    pub scene: String,
    pub state: String,
    pub best: bool,
    pub score: f32,
}

/// 一个确定的伪随机数（线性同余），保证两端画出来的像素完全一样。
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32) / ((1u64 << 31) as f32)
    }
}

/// 一个场景：天空渐变、地平线、一个太阳 / 月亮、地面上的纹理（让清晰度有东西可量）。
fn scene(seed: u64, w: u32, h: u32) -> RgbImage {
    let mut r = Lcg(seed);
    let sky_top = [r.next() * 120.0, r.next() * 140.0 + 40.0, r.next() * 120.0 + 120.0];
    let sky_bot = [r.next() * 80.0 + 170.0, r.next() * 90.0 + 140.0, r.next() * 80.0 + 100.0];
    let ground = [r.next() * 90.0 + 30.0, r.next() * 100.0 + 60.0, r.next() * 60.0 + 20.0];
    let horizon = (0.45 + r.next() * 0.2) * h as f32;
    let (sx, sy, sr) = (r.next() * w as f32, horizon * (0.3 + r.next() * 0.4), 30.0 + r.next() * 50.0);
    let freq = 0.04 + r.next() * 0.08;
    let mut img = RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let (xf, yf) = (x as f32, y as f32);
            let mut c = if yf < horizon {
                let t = yf / horizon;
                [sky_top[0] + (sky_bot[0] - sky_top[0]) * t, sky_top[1] + (sky_bot[1] - sky_top[1]) * t, sky_top[2] + (sky_bot[2] - sky_top[2]) * t]
            } else {
                let tex = ((xf * freq).sin() * (yf * freq * 1.7).cos() * 0.5 + 0.5) * 60.0;
                [ground[0] + tex, ground[1] + tex, ground[2] + tex * 0.6]
            };
            let d = ((xf - sx).powi(2) + (yf - sy).powi(2)).sqrt();
            if d < sr { c = [250.0, 235.0, 200.0]; }
            img.put_pixel(x, y, Rgb([c[0].clamp(0.0, 255.0) as u8, c[1].clamp(0.0, 255.0) as u8, c[2].clamp(0.0, 255.0) as u8]));
        }
    }
    img
}

fn brightness(img: &RgbImage, k: f32) -> RgbImage {
    RgbImage::from_fn(img.width(), img.height(), |x, y| {
        let p = img.get_pixel(x, y);
        Rgb([(p[0] as f32 * k).min(255.0) as u8, (p[1] as f32 * k).min(255.0) as u8, (p[2] as f32 * k).min(255.0) as u8])
    })
}

/// 稍微挪一下取景（裁掉边上一点再放回原大小），像连拍里手动了一下。
fn nudge(img: &RgbImage, px: u32) -> RgbImage {
    let (w, h) = img.dimensions();
    let crop = image::imageops::crop_imm(img, px, px / 2, w - px, h - px / 2).to_image();
    image::imageops::resize(&crop, w, h, image::imageops::FilterType::Triangle)
}

/// 画出固定用例的照片到 `dir`：三组连拍（各自有清晰的、手抖的、过暗 / 过亮的）、两张单独的场景。
pub fn fixture(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let (w, h) = (960, 640);
    let mut out: Vec<(String, RgbImage)> = Vec::new();
    let a = scene(11, w, h);
    out.push(("a1".into(), a.clone()));
    out.push(("a2".into(), nudge(&a, 12)));
    out.push(("a3".into(), image::imageops::blur(&a, 4.0)));
    out.push(("a4".into(), brightness(&a, 1.6)));
    let b = scene(29, w, h);
    out.push(("b1".into(), b.clone()));
    out.push(("b2".into(), brightness(&b, 0.35)));
    out.push(("b3".into(), image::imageops::blur(&nudge(&b, 8), 2.5)));
    let c = scene(47, w, h);
    out.push(("c1".into(), c.clone()));
    out.push(("c2".into(), nudge(&c, 20)));
    out.push(("d1".into(), scene(83, w, h)));
    out.push(("e1".into(), image::imageops::blur(&scene(101, w, h), 6.0)));
    for (name, img) in out {
        img.save_with_format(dir.join(format!("{name}.jpg")), image::ImageFormat::Jpeg).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 分析一个文件夹（不注册平台能力、只用 CPU、标准档），取舍按默认（不设保留上限、兼顾不同场景）。
pub fn run(photos: &Path, work: &Path, models: &Path) -> Result<Vec<Row>, String> {
    let paths = Paths { data: work.join("data"), thumbs: work.join("thumbs"), models: models.to_path_buf(), downloads: work.join("downloads") };
    let engine = Engine::new(paths, Box::new(NoEvents), "consistency");
    let dir = photos.to_string_lossy().into_owned();
    let list = engine.scan_project(ScanArgs {
        key: "consistency".into(), dirs: vec![dir], compute: Some("cpu".into()), quality: Some("fast".into()),
        learn: false, quiet: true, style: None, force: true, eye_method: Some("mediapipe".into()),
    })?;
    let decisions = engine.decide_photos("consistency", 1.0, None, false, &HashMap::new())?;
    let stem = |p: &str| Path::new(p).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    // 编号换成"这一组里名字最小的那张"
    let mut group_min: HashMap<i64, String> = HashMap::new();
    let mut scene_min: HashMap<i64, String> = HashMap::new();
    for p in &list {
        let n = stem(&p.path);
        let g = group_min.entry(p.group as i64).or_insert_with(|| n.clone());
        if n < *g { *g = n.clone(); }
        let s = scene_min.entry(p.scene as i64).or_insert_with(|| n.clone());
        if n < *s { *s = n.clone(); }
    }
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    for p in &list {
        let n = stem(&p.path);
        let state = decisions.get(&p.path).map(|d| serde_json::to_value(&d.state).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()).unwrap_or_default();
        rows.insert(n.clone(), Row {
            name: n, group: group_min[&(p.group as i64)].clone(), scene: scene_min[&(p.scene as i64)].clone(), state,
            best: p.group_best, score: (p.score * 1000.0).round() / 1000.0,
        });
    }
    Ok(rows.into_values().collect())
}

/// 和基准比：分组、场景、取舍、组里最好的必须一样；分数差不超过 0.02。返回所有不一致的地方。
pub fn compare(golden: &[Row], got: &[Row]) -> Result<(), String> {
    let mut diffs = Vec::new();
    if golden.len() != got.len() {
        diffs.push(format!("照片数不同：基准 {}，这次 {}", golden.len(), got.len()));
    }
    for g in golden {
        match got.iter().find(|r| r.name == g.name) {
            None => diffs.push(format!("{}：这次没有", g.name)),
            Some(r) => {
                if r.group != g.group { diffs.push(format!("{}：分组 {} ≠ 基准 {}", g.name, r.group, g.group)); }
                if r.scene != g.scene { diffs.push(format!("{}：场景 {} ≠ 基准 {}", g.name, r.scene, g.scene)); }
                if r.state != g.state { diffs.push(format!("{}：取舍 {} ≠ 基准 {}", g.name, r.state, g.state)); }
                if r.best != g.best { diffs.push(format!("{}：组内最佳 {} ≠ 基准 {}", g.name, r.best, g.best)); }
                if (r.score - g.score).abs() > 0.02 { diffs.push(format!("{}：综合分 {:.3} ≠ 基准 {:.3}", g.name, r.score, g.score)); }
            }
        }
    }
    if diffs.is_empty() { Ok(()) } else { Err(diffs.join("\n")) }
}

/// 画用例、分析、和基准比（`golden` 是基准的内容）；`FG_WRITE_GOLDEN` 设了就改写 `golden_path`（只在开发机上用）。两端的测试都调它。
pub fn check(work: &Path, models: &Path, golden: &str, golden_path: Option<&Path>) -> Result<Vec<Row>, String> {
    let photos = work.join("photos");
    fixture(&photos)?;
    let got = run(&photos, work, models)?;
    if let (Some(p), true) = (golden_path, std::env::var("FG_WRITE_GOLDEN").is_ok()) {
        std::fs::write(p, serde_json::to_string_pretty(&got).map_err(|e| e.to_string())? + "\n").map_err(|e| e.to_string())?;
        return Ok(got);
    }
    let golden: Vec<Row> = serde_json::from_str(golden).map_err(|e| format!("基准读不出来：{e}"))?;
    compare(&golden, &got).map(|_| got)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 没有任何模型时（CI 上就是这样）的固定用例。Android 端的同名测试比的是同一份基准。
    #[test]
    fn consistency_without_models() {
        let work = std::env::temp_dir().join(format!("fg-consistency-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("consistency.golden.json");
        let r = check(&work, &work.join("no-models"), GOLDEN, Some(&golden));
        let _ = std::fs::remove_dir_all(&work);
        if let Err(e) = r { panic!("和基准不一致：\n{e}"); }
    }

    /// 装了标准档模型时的固定用例：`FG_MODELS=<模型文件夹> cargo test -p fleetinggleam-core consistency -- --ignored`。
    /// CI 上没有模型，所以默认不跑；开发机上用 apps/desktop/models，Android 上见 apps/android/scripts/consistency.sh。
    #[test]
    #[ignore]
    fn consistency_with_standard_models() {
        let models = std::path::PathBuf::from(std::env::var("FG_MODELS").expect("FG_MODELS 指向模型文件夹"));
        let work = std::env::temp_dir().join(format!("fg-consistency-m-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("consistency.models.golden.json");
        let r = check(&work, &models, GOLDEN_MODELS, Some(&golden));
        let _ = std::fs::remove_dir_all(&work);
        if let Err(e) = r { panic!("和基准不一致：\n{e}"); }
    }
}
