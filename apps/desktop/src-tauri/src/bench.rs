//! 需要真实照片、模型与 macOS 边车的端到端与计时测试（全部 `#[ignore]`，手动跑）。它们从核心库搬到这里，因为要用 Apple Vision 边车：
//! `cargo test -p fleetinggleam --lib bench -- --ignored --nocapture`，各测试的环境变量见各自的 `ignore` 说明。

use std::path::{Path, PathBuf};

use fg_core::analyze::{self, analyze, group_and_score, is_image, EyeMethod, Photo, ANALYZE_SIZE};
use fg_core::analyze::{dhash, fast_resize, load_oriented, save_analysis_copy, subject_sharpness};
use fg_core::faces::{align, assign_people, FaceEmbedder, MIN_FACE};
use fg_core::mediapipe::MediapipeEyes;
use fg_core::platform::SystemVision;
use fg_core::scorer::{self, AiScorer};
use image::imageops::FilterType;
use image::RgbImage;

use crate::vision::Vision;

/// 开发机上的模型目录（`apps/desktop/models/`，不进仓库）。
fn dev_models_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../models")
}

mod tests {
    use super::*;

    /// End to end: original + degraded copies through the real pipeline (CLIP, Apple helper, grouping).
    #[test]
    #[ignore = "needs PROBE_IMG, TMP_DIR, models/ and bin/fg-vision"]
    fn pipeline_smoke() {
        let (Ok(p), Ok(tmp)) = (std::env::var("PROBE_IMG"), std::env::var("TMP_DIR")) else { return };
        let tmp = PathBuf::from(tmp);
        let base = image::open(p).unwrap().resize(1600, 1600, FilterType::Triangle).to_rgb8();
        let variants: Vec<(&str, image::RgbImage)> = vec![
            ("a_original", base.clone()),
            ("b_blur_light", image::imageops::blur(&base, 1.5)),
            ("c_blur_heavy", image::imageops::blur(&base, 6.0)),
            ("d_dark", image::imageops::brighten(&base, -90)),
            ("e_over", image::imageops::brighten(&base, 110)),
        ];
        let mut paths = Vec::new();
        for (n, im) in &variants {
            let path = tmp.join(format!("{n}.jpg"));
            im.save(&path).unwrap();
            paths.push(path);
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let scorer = scorer::ClipScorer::load(&dev_models_dir(), true, scorer::Quality::Fast).ok();
        let vision = Vision::start(&root.join("bin/fg-vision"), 2, false);
        println!("clip={} vision={}", scorer.is_some(), vision.is_some());
        let mut photos: Vec<Photo> = paths
            .iter()
            .map(|p| analyze(p, &tmp, scorer.as_ref().map(|s| s as &dyn AiScorer), vision.as_ref().map(|v| v as &dyn SystemVision), EyeMethod::Vision, None, None, &|_, _, _| {}).unwrap())
            .collect();
        group_and_score(&mut photos, None);
        for p in &photos {
            println!(
                "{:<14} score={:.2} sharp={:.2} expo={:.2} clip={:?} sys={:?} faces={} flags={:?} group={} scene={} best={}",
                p.name, p.score, p.sharp_score, p.expo_score,
                p.clip_score.map(|v| (v * 100.0).round() / 100.0), p.sys_score.map(|v| (v * 100.0).round() / 100.0),
                p.face_count, p.flags, p.group, p.scene, p.group_best
            );
        }
    }

    /// The real code path (`analyze()`, not a hand-rolled crop) on real photos, both eye methods side by side: this is
    /// what actually ships, not a numeric spot check of the models in isolation.
    #[test]
    #[ignore = "needs FG_LIBRARY (a folder of real photos with faces), models/ and bin/fg-vision"]
    fn eye_methods_on_real_photos() {
        let Ok(dir) = std::env::var("FG_LIBRARY") else { return };
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let model_dir = dev_models_dir();
        let vision = Vision::start(&root.join("bin/fg-vision"), 2, false).expect("fg-vision (macOS only)");
        let mp = MediapipeEyes::load(&model_dir).expect("load MediaPipe eyes (needs models/ from the `eyes` pack)");
        let tmp = std::env::temp_dir().join("fg_eye_methods_test");
        std::fs::create_dir_all(&tmp).unwrap();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| is_image(p)).collect();
        files.sort();
        files.truncate(40);
        println!("{} photos", files.len());
        for f in &files {
            let a = analyze(f, &tmp, None, Some(&vision as &dyn SystemVision), EyeMethod::Vision, None, None, &|_, _, _| {});
            let b = analyze(f, &tmp, None, Some(&vision as &dyn SystemVision), EyeMethod::Mediapipe, Some(&mp), None, &|_, _, _| {});
            match (a, b) {
                (Ok(a), Ok(b)) if a.face_count > 0 => {
                    println!("{:<28} faces={} vision={:>6.3} mediapipe={:>6.3}", a.name, a.face_count, a.eyes_open.unwrap_or(-1.0), b.eyes_open.unwrap_or(-1.0));
                }
                _ => {}
            }
        }
    }

    /// Where the time of a scan goes, on real photos: one at a time (per stage), then all at once the way a scan runs them.
    #[test]
    #[ignore = "timing only: FG_LIBRARY=<folder of photos> [FG_N=24] [FG_QUALITY=fast|high|ultra] [FG_COPIES=n] [FG_VISION=workers], models/ and bin/fg-vision; run with --ignored --nocapture"]
    fn where_the_scan_time_goes() {
        use rayon::prelude::*;
        use std::time::Instant;
        let Ok(dir) = std::env::var("FG_LIBRARY") else { return };
        let n: usize = std::env::var("FG_N").ok().and_then(|v| v.parse().ok()).unwrap_or(16);
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let model_dir = dev_models_dir();
        let quality = scorer::Quality::parse(std::env::var("FG_QUALITY").ok().as_deref());
        let copies = std::env::var("FG_COPIES").ok().and_then(|v| v.parse().ok());
        let scorer = scorer::ClipScorer::load_with(&model_dir, std::env::var("FG_ACCEL").map_or(true, |v| v != "0"), quality, copies).expect("models/");
        println!("CLIP: {} via {}", scorer.model, scorer.backend);
        let workers: usize = std::env::var("FG_VISION").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
        let vision = Vision::start(&root.join("bin/fg-vision"), workers, false).expect("fg-vision (macOS only)");
        let tmp = std::env::temp_dir().join("fg_timing_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| is_image(p)).collect();
        files.sort();
        files.truncate(n);
        // warm up the models (the first call pays for loading and compiling them)
        let _ = analyze(&files[0], &tmp, Some(&scorer as &dyn AiScorer), Some(&vision as &dyn SystemVision), EyeMethod::Vision, None, None, &|_, _, _| {});
        // marks: [start, "decode" (the first call), "quality" (decode, resize and the thumbnail are done), "ai" (stats done), "system" (CLIP done), "thumb" (Vision, faces done)], then the end
        let names = ["decode+resize+thumb", "quality stats", "CLIP", "Vision+faces", "tail (exif, media)"];
        let timed = |f: &PathBuf| -> Option<[f64; 5]> {
            let marks: std::sync::Mutex<Vec<Instant>> = std::sync::Mutex::new(vec![Instant::now()]);
            let r = analyze(f, &tmp, Some(&scorer as &dyn AiScorer), Some(&vision as &dyn SystemVision), EyeMethod::Vision, None, None, &|_, _, _| marks.lock().unwrap().push(Instant::now()));
            let end = Instant::now();
            r.ok()?;
            let m = marks.lock().unwrap();
            let at = [m[1], m[2], m[3], m[4], m[5], end];
            let mut out = [0f64; 5];
            for i in 0..5 { out[i] = at[i + 1].duration_since(at[i]).as_secs_f64() * 1000.0; }
            Some(out)
        };
        let report = |label: &str, wall: std::time::Duration, rows: &[[f64; 5]]| {
            let n = rows.len() as f64;
            let mut sum = [0f64; 5];
            for r in rows { for i in 0..5 { sum[i] += r[i]; } }
            println!("\n{label}: {} photos, {:?} wall ({:.0} ms per photo)", rows.len(), wall, wall.as_secs_f64() * 1000.0 / n);
            for i in 0..5 { println!("  {:<22} {:>7.0} ms/photo", names[i], sum[i] / n); }
        };
        let one = Instant::now();
        let rows: Vec<[f64; 5]> = files.iter().filter_map(|f| timed(f)).collect();
        let seq = one.elapsed();
        report("one at a time", seq, &rows);
        let par = Instant::now();
        let rows: Vec<[f64; 5]> = files.par_iter().filter_map(|f| timed(f)).collect();
        let pt = par.elapsed();
        report(&format!("all at once ({} threads)", rayon::current_num_threads()), pt, &rows);
        println!("  -> {:.1}x faster than one at a time", seq.as_secs_f64() / pt.as_secs_f64());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Per-stage wall time on one full-size photo, and CLIP on CPU vs the CoreML provider.
    #[test]
    #[ignore = "needs PROBE_IMG, models/ and bin/fg-vision"]
    fn bench_stages() {
        use std::time::Instant;
        let Ok(p) = std::env::var("PROBE_IMG") else { return };
        let ms = |t: Instant| t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let full = load_oriented(Path::new(&p)).unwrap();
        println!("decode {}x{}      {:>7.1} ms", full.width(), full.height(), ms(t));
        let t = Instant::now();
        let work = fast_resize(&full, ANALYZE_SIZE);
        println!("resize to 1024       {:>7.1} ms  -> {}x{}", ms(t), work.width(), work.height());
        let gray = work.to_luma8();
        let t = Instant::now();
        let _ = subject_sharpness(&gray);
        println!("sharpness (tiles)    {:>7.1} ms", ms(t));
        let t = Instant::now();
        let _ = dhash(&work);
        println!("dhash                {:>7.1} ms", ms(t));
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for (label, accel) in [("CLIP  CPU only      ", false), ("CLIP  CoreML provider", true)] {
            let load = Instant::now();
            let s = scorer::ClipScorer::load(&dev_models_dir(), accel, scorer::Quality::Fast).unwrap();
            let load_ms = ms(load);
            let _ = s.analyze(&work); // warm-up (CoreML compiles on first run)
            let t = Instant::now();
            for _ in 0..8 { s.analyze(&work).unwrap(); }
            println!("{label} load {:>7.0} ms, per image {:>7.1} ms", load_ms, ms(t) / 8.0);
        }
        if let Some(v) = Vision::start(&root.join("bin/fg-vision"), 1, false) {
            let small = save_analysis_copy(&work, Path::new(&p), &std::env::temp_dir()).unwrap();
            let _ = v.analyze(&small);
            let t = Instant::now();
            for _ in 0..4 { v.analyze(&small); }
            println!("Apple Vision (1024px copy) {:>7.1} ms per image", ms(t) / 4.0);
            let t = Instant::now();
            for _ in 0..2 { v.analyze(Path::new(&p)); }
            println!("Apple Vision (full original) {:>5.1} ms per image", ms(t) / 2.0);
        }
    }

    /// Whole-pipeline throughput on N copies of one full-size photo: CPU int8 vs CoreML fp16 (with and without Apple Vision).
    #[test]
    #[ignore = "needs PROBE_IMG, TMP_DIR, models/ (incl. fp16) and bin/fg-vision"]
    fn pipeline_throughput() {
        use rayon::prelude::*;
        use std::time::Instant;
        let (Ok(p), Ok(tmp)) = (std::env::var("PROBE_IMG"), std::env::var("TMP_DIR")) else { return };
        let tmp = PathBuf::from(tmp);
        let n = 30;
        let paths: Vec<PathBuf> = (0..n)
            .map(|i| {
                let d = tmp.join(format!("tp_{i}.jpg"));
                if !d.exists() {
                    std::fs::copy(&p, &d).unwrap();
                }
                d
            })
            .collect();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for with_vision in [false, true] {
            for accel in [false, true] {
                let scorer = scorer::ClipScorer::load(&dev_models_dir(), accel, scorer::Quality::Fast).unwrap();
                let vision = if with_vision { Vision::start(&root.join("bin/fg-vision"), 2, false) } else { None };
                let t = Instant::now();
                let done: usize = paths
                    .par_iter()
                    .map(|q| analyze(q, &tmp, Some(&scorer as &dyn AiScorer), vision.as_ref().map(|v| v as &dyn SystemVision), EyeMethod::Vision, None, None, &|_, _, _| {}).is_ok() as usize)
                    .sum();
                let secs = t.elapsed().as_secs_f64();
                println!("vision={:<5} clip={:<12} {done} photos in {secs:>5.1}s  ({:.0} ms/photo)", with_vision, scorer.backend, secs * 1000.0 / done as f64);
            }
        }
    }

    /// Every format in a folder goes through the real analysis: HEIC, TIFF, BMP, PNG and an HDR JPEG here, camera RAW on a photographer's machine.
    #[test]
    #[ignore = "needs FG_FORMATS (a folder with one file of each format) and bin/fg-image (macOS)"]
    fn every_format_is_analysed() {
        let Ok(dir) = std::env::var("FG_FORMATS") else { return };
        let tmp = std::env::temp_dir().join("fg_formats_test");
        std::fs::create_dir_all(&tmp).unwrap();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| is_image(p)).collect();
        files.sort();
        assert!(!files.is_empty());
        for f in files {
            let p = analyze(&f, &tmp, None, None, EyeMethod::Vision, None, None, &|_, _, _| {}).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
            assert!(p.width > 0 && p.height > 0 && Path::new(&p.thumb).exists(), "{}", f.display());
            println!("{:<10} {}x{} hdr={} motion={} score={:.2}", f.file_name().unwrap().to_string_lossy(), p.width, p.height, p.hdr, p.motion, p.score);
        }
    }


    /// Runs the real pipeline over a folder whose analysis copies are already in the app cache and reports how the
    /// faces cluster: PROBE_DIR=<photos> [FG_THUMBS=<cache dir>] [SHEET=<png to write, face crops of the biggest people>].
    #[test]
    #[ignore = "needs models/, bin/fg-vision and a real, already analysed folder"]
    fn people_in_a_real_folder() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let dir = std::env::var("PROBE_DIR").expect("PROBE_DIR");
        let thumbs = std::env::var("FG_THUMBS").unwrap_or_else(|_| format!("{}/Library/Caches/com.fleetinggleam.app/thumbs", std::env::var("HOME").unwrap()));
        let vision = Vision::start(&root.join("bin/fg-vision"), 2, false).expect("vision");
        let m = FaceEmbedder::load(&dev_models_dir()).expect("load");
        let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| analyze::is_image(p)).collect();
        files.sort();
        let mut photos = Vec::new();
        let mut crops: Vec<(usize, usize, RgbImage)> = Vec::new();
        for f in &files {
            let copy = analyze::cache_name(f, ".a.jpg", Path::new(&thumbs));
            let Some(v) = vision.analyze(&copy) else { continue };
            let Ok(img) = image::open(&copy) else { continue };
            let found = m.embed(&img, &v.face_marks);
            let rgb = img.to_rgb8();
            let (w, h) = (rgb.width() as f32, rgb.height() as f32);
            for (fi, mk) in v.face_marks.iter().filter(|mk| mk.bbox[3] * h >= MIN_FACE * w.max(h)).enumerate() {
                let pts: Vec<[f32; 2]> = mk.pts.iter().map(|p| [p[0] * w, p[1] * h]).collect();
                if let Some(c) = align(&rgb, &pts) {
                    crops.push((photos.len(), fi, c));
                }
            }
            photos.push(Photo { path: f.to_string_lossy().into_owned(), face_boxes: found.iter().map(|x| x.bbox).collect(), face_embs: found.into_iter().map(|x| x.emb).collect(), ..Default::default() });
        }
        assign_people(&mut photos);
        let mut per: std::collections::BTreeMap<i32, usize> = Default::default();
        for p in &photos {
            for &id in &p.people {
                *per.entry(id).or_default() += 1;
            }
        }
        let with_faces = photos.iter().filter(|p| !p.face_boxes.is_empty()).count();
        println!("{} photos, {} with recognisable faces, faces per person id (-1 = nobody in particular): {:?}", photos.len(), with_faces, per);
        if let Ok(sheet) = std::env::var("SHEET") {
            // the first 16 faces of each of the first four people, one person per row
            let mut img = RgbImage::new(16 * 112, 4 * 112);
            for person in 0..4i32 {
                let mut col = 0u32;
                for (pi, fi, c) in &crops {
                    if photos[*pi].people.get(*fi) == Some(&person) && col < 16 {
                        image::imageops::replace(&mut img, c, (col * 112) as i64, (person as u32 * 112) as i64);
                        col += 1;
                    }
                }
            }
            img.save(sheet).unwrap();
        }
    }


    /// 引擎在 Mac 上的完整一遍：注册 Apple Vision、ImageIO、Metal CLIP 与废纸篓后，用真模型分析开发用的示例照片，第二次全部走缓存。
    /// `cargo test -p fleetinggleam --lib engine_with_macos_helpers -- --ignored --nocapture`（需要 models/ 与 bin/）。
    #[test]
    #[ignore = "needs models/ and bin/fg-vision, bin/fg-image"]
    fn engine_with_macos_helpers() {
        use fg_core::engine::{Engine, NoEvents, Paths, ScanArgs};
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        crate::imageio_macos::Helper::register(root.join("bin/fg-image"));
        fg_core::platform::set_vision(Box::new(crate::vision::Factory { bin: root.join("bin/fg-vision") }));
        fg_core::platform::set_gpu_clip(Box::new(crate::clipgpu::Factory));
        let tmp = std::env::temp_dir().join(format!("fg-engine-mac-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let e = Engine::new(Paths { data: tmp.join("data"), thumbs: tmp.join("thumbs"), models: dev_models_dir(), downloads: dev_models_dir() }, Box::new(NoEvents), "test");
        let dirs = vec![root.join("../public/dev-photos").to_string_lossy().into_owned()];
        let args = ScanArgs { key: "dev".into(), dirs: dirs.clone(), quality: Some("fast".into()), eye_method: Some("vision".into()), ..Default::default() };
        let t = std::time::Instant::now();
        let photos = e.scan_project(args.clone()).expect("scan");
        println!("{} photos in {:?}", photos.len(), t.elapsed());
        assert!(!photos.is_empty());
        assert!(photos.iter().all(|p| p.clip_score.is_some()), "CLIP ran on every photo");
        assert!(photos.iter().any(|p| p.sys_score.is_some()), "Apple Vision answered");
        for p in photos.iter().take(5) {
            println!("  {:<10} score {:.2} clip {:?} sys {:?} faces {} kind {}", p.name, p.score, p.clip_score, p.sys_score, p.face_count, p.kind);
        }
        assert_eq!(e.probe_folder(&dirs, None, Some("fast")).unwrap().uncached, 0);
        let t = std::time::Instant::now();
        let again = e.scan_project(args).unwrap();
        println!("again from the cache in {:?}", t.elapsed());
        assert_eq!(again.len(), photos.len());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
