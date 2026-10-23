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

/// 一个进程（自己或已回收的子进程）到现在为止用掉的 CPU 秒数（用户态 + 内核态）。
fn cpu_seconds(who: libc::c_int) -> f64 {
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(who, &mut ru) };
    let secs = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
    secs(ru.ru_utime) + secs(ru.ru_stime)
}

/// 本进程的边车子进程（名字含 `fg-`）此刻各用掉的 CPU 秒数，按名字汇总：问 `ps`。常驻的（`fg-image serve`）不会被回收，
/// `getrusage` 的子进程统计里没有它们；已回收的（分析结束被杀的 fg-vision）则相反，所以两边都要看。
fn live_helper_cpu() -> std::collections::BTreeMap<String, f64> {
    let mut out = std::collections::BTreeMap::new();
    let Ok(all) = std::process::Command::new("ps").args(["-axo", "ppid=,time=,comm="]).output() else { return out };
    let me = std::process::id().to_string();
    for l in String::from_utf8_lossy(&all.stdout).lines() {
        let mut f = l.split_whitespace();
        let (Some(ppid), Some(time), Some(comm)) = (f.next(), f.next(), f.next()) else { continue };
        if ppid != me || !comm.contains("fg-") {
            continue;
        }
        // 格式 [[dd-]hh:]mm:ss.cc
        let parts: Vec<f64> = time.split(':').filter_map(|x| x.parse().ok()).collect();
        let secs: f64 = parts.iter().rev().enumerate().map(|(i, v)| v * 60f64.powi(i as i32)).sum();
        let name = Path::new(comm).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        *out.entry(name).or_insert(0.0) += secs;
    }
    out
}

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

    /// 像素路径（分析现在走的）与文件路径（以前先写 JPEG 副本再读回）在真实照片上给出的信号是否一致：
    /// `FG_LIBRARY=<文件夹> [FG_N=60] cargo test -p fleetinggleam --lib vision_pixels_match_the_file_path -- --ignored --nocapture`。
    #[test]
    #[ignore = "needs FG_LIBRARY and bin/fg-vision"]
    fn vision_pixels_match_the_file_path() {
        let Ok(dir) = std::env::var("FG_LIBRARY") else { return };
        let n: usize = std::env::var("FG_N").ok().and_then(|v| v.parse().ok()).unwrap_or(60);
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let vision = Vision::start(&root.join("bin/fg-vision"), 2, false).expect("fg-vision (macOS only)");
        let tmp = std::env::temp_dir().join("fg_vision_match_test");
        std::fs::create_dir_all(&tmp).unwrap();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| is_image(p)).collect();
        files.sort();
        files.truncate(n);
        let (mut same_faces, mut total, mut max_aes, mut max_eyes, mut with_faces, mut max_aes_decoded) = (0, 0, 0f32, 0f32, 0, 0f32);
        for f in &files {
            let work = analyze::load_for_embedding(f).unwrap();
            let copy = save_analysis_copy(&work, f, &tmp).unwrap();
            // 第三路：同一张 JPEG 副本由 image crate 解码后再走像素路径——它和像素路径接近、和文件路径差得远，就说明差异来自边车读 JPEG 的方式，而不是 JPEG 压缩
            let decoded = image::open(&copy).unwrap();
            let (Some(a), Some(b), Some(c)) = (vision.analyze(&copy), fg_core::platform::SystemVision::analyze_pixels(&vision, &work), fg_core::platform::SystemVision::analyze_pixels(&vision, &decoded)) else { continue };
            total += 1;
            if a.faces.len() == b.faces.len() { same_faces += 1; } else { println!("{}: faces {} (file) vs {} (pixels) vs {} (decoded copy)", f.file_name().unwrap().to_string_lossy(), a.faces.len(), b.faces.len(), c.faces.len()); }
            let d = (a.aesthetic.unwrap() - b.aesthetic.unwrap()).abs();
            // 量过（2026-10-22，120 张）：差超过 0.05 的只有两张雾蒙蒙的天空——image crate 的 q90 JPEG 把噪点抹平，Vision 的美学模型对这类画面敏感；
            // 同一张 JPEG 解码后再走像素路径与文件路径一致，原始像素怎么喂都是另一个值，所以差异来自 JPEG 再编码，不是协议
            if d > 0.05 { println!("{}: aesthetic file {:.4} pixels {:.4} decoded copy {:.4}", f.file_name().unwrap().to_string_lossy(), a.aesthetic.unwrap(), b.aesthetic.unwrap(), c.aesthetic.unwrap()); }
            max_aes = max_aes.max(d);
            max_aes_decoded = max_aes_decoded.max((c.aesthetic.unwrap() - b.aesthetic.unwrap()).abs());
            if !a.eyes.is_empty() && a.eyes.len() == b.eyes.len() {
                with_faces += 1;
                let ea = a.eyes.iter().cloned().reduce(f32::min).unwrap();
                let eb = b.eyes.iter().cloned().reduce(f32::min).unwrap();
                max_eyes = max_eyes.max((ea - eb).abs());
                println!("{}: faces {} aesthetic {:.4} / {:.4} eyes {:.4} / {:.4}", f.file_name().unwrap().to_string_lossy(), a.faces.len(), a.aesthetic.unwrap(), b.aesthetic.unwrap(), ea, eb);
            }
        }
        println!("{total} photos: same face count on {same_faces}; max |Δaesthetic| file vs pixels {max_aes:.4}, decoded copy vs pixels {max_aes_decoded:.4}; {with_faces} with measurable eyes, max |Δeyes| {max_eyes:.4}");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// 解码这一段单独看：常驻 ImageIO 边车（`SystemImages::decode`）与进程内整张解码再缩（老路）给出的图是否一样大、像素差多少；
    /// 一条线程与十条线程时各要等多久、缩略图又占多少；边车用了多少 CPU。`FG_LIBRARY=<文件夹> [FG_N=60]`。
    #[test]
    #[ignore = "timing only: FG_LIBRARY=<folder of photos>; bin/fg-image"]
    fn decode_stage_alone() {
        use rayon::prelude::*;
        use std::time::Instant;
        let Ok(dir) = std::env::var("FG_LIBRARY") else { return };
        let n: usize = std::env::var("FG_N").ok().and_then(|v| v.parse().ok()).unwrap_or(60);
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        crate::imageio_macos::Helper::register(root.join("bin/fg-image"));
        let images = fg_core::platform::images().expect("fg-image");
        let tmp = std::env::temp_dir().join("fg_decode_stage_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| is_image(p)).collect();
        files.sort();
        files.truncate(n);
        // 一致性：前 12 张
        let mut max_diff = 0f64;
        for f in files.iter().take(12) {
            let (new, ow, oh) = images.decode(f, ANALYZE_SIZE).expect("decode");
            let old_full = load_oriented(f).unwrap();
            let old = fast_resize(&old_full, ANALYZE_SIZE);
            assert_eq!((ow, oh), (old_full.width(), old_full.height()), "{}: displayed size", f.display());
            assert_eq!((new.width(), new.height()), (old.width(), old.height()), "{}: analysis size", f.display());
            let (a, b) = (new.to_rgb8(), old.to_rgb8());
            let diff = a.as_raw().iter().zip(b.as_raw()).map(|(x, y)| (*x as f64 - *y as f64).abs()).sum::<f64>() / a.as_raw().len() as f64;
            max_diff = max_diff.max(diff);
        }
        println!("12 photos: sizes identical, mean |Δpixel| at most {max_diff:.2} (resampler and colour management differ)");
        // 两条解码路径给分析的信号差多少：Vision 的人脸数与美学分、清晰度、CLIP 分（前 FG_N 张）
        let vision = Vision::start(&root.join("bin/fg-vision"), 4, false).expect("fg-vision");
        let scorer = scorer::ClipScorer::load(&dev_models_dir(), true, scorer::Quality::Fast).expect("models/");
        let (mut same_faces, mut max_aes, mut max_sharp, mut max_clip, mut cmp) = (0, 0f32, 0f32, 0f32, 0);
        let (mut sharps, mut clips) = (Vec::new(), Vec::new());
        for f in &files {
            let (new, _, _) = images.decode(f, ANALYZE_SIZE).unwrap();
            let old = fast_resize(&load_oriented(f).unwrap(), ANALYZE_SIZE);
            let (Some(va), Some(vb)) = (SystemVision::analyze_pixels(&vision, &new), SystemVision::analyze_pixels(&vision, &old)) else { continue };
            cmp += 1;
            if va.faces.len() == vb.faces.len() { same_faces += 1; } else { println!("{}: faces {} (resident decoder) vs {} (in-process decode)", f.file_name().unwrap().to_string_lossy(), va.faces.len(), vb.faces.len()); }
            max_aes = max_aes.max((va.aesthetic.unwrap() - vb.aesthetic.unwrap()).abs());
            let (sa, sb) = (fg_core::scoring::sharp_score(subject_sharpness(&new.to_luma8())), fg_core::scoring::sharp_score(subject_sharpness(&old.to_luma8())));
            max_sharp = max_sharp.max((sa - sb).abs());
            sharps.push((sa - sb).abs());
            if let (Ok(ca), Ok(cb)) = (scorer.analyze(&new), scorer.analyze(&old)) {
                max_clip = max_clip.max((ca.score - cb.score).abs());
                clips.push((ca.score - cb.score).abs());
            }
        }
        let pct = |v: &mut Vec<f32>, p: f64| { v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v.get(((v.len() as f64 - 1.0) * p).round() as usize).copied().unwrap_or(0.0) };
        println!(
            "{cmp} photos, resident decoder vs in-process (FG_IMAGE_OVERSAMPLE={}): same face count on {same_faces}; max |Δaesthetic| {max_aes:.3}; |Δsharp_score| median {:.3} p95 {:.3} max {max_sharp:.3}; |ΔCLIP score| median {:.3} p95 {:.3} max {max_clip:.3}",
            std::env::var("FG_IMAGE_OVERSAMPLE").unwrap_or_else(|_| "1".into()), pct(&mut sharps, 0.5), pct(&mut sharps, 0.95), pct(&mut clips, 0.5), pct(&mut clips, 0.95)
        );
        let timed = |f: &PathBuf| -> (f64, f64) {
            let t = Instant::now();
            let (img, _, _) = images.decode(f, ANALYZE_SIZE).unwrap();
            let d = t.elapsed().as_secs_f64() * 1000.0;
            let t = Instant::now();
            let _ = analyze::save_thumb(&img, f, &tmp).unwrap();
            (d, t.elapsed().as_secs_f64() * 1000.0)
        };
        let _ = timed(&files[0]);
        let helper = |m: &std::collections::BTreeMap<String, f64>| m.values().sum::<f64>();
        let cpu0 = helper(&live_helper_cpu());
        let t = Instant::now();
        let rows: Vec<(f64, f64)> = files.iter().map(timed).collect();
        let seq = t.elapsed();
        println!("one at a time: {:.0} ms/photo wall; decode wait {:.0} ms, thumbnail {:.0} ms", seq.as_secs_f64() * 1000.0 / n as f64, rows.iter().map(|r| r.0).sum::<f64>() / n as f64, rows.iter().map(|r| r.1).sum::<f64>() / n as f64);
        let cpu1 = helper(&live_helper_cpu());
        let t = Instant::now();
        let rows: Vec<(f64, f64)> = files.par_iter().map(timed).collect();
        let par = t.elapsed();
        let cpu2 = helper(&live_helper_cpu());
        println!("{} threads: {:.0} ms/photo wall ({:.0} photos/s); per thread decode wait {:.0} ms, thumbnail {:.0} ms", rayon::current_num_threads(), par.as_secs_f64() * 1000.0 / n as f64, n as f64 / par.as_secs_f64(), rows.iter().map(|r| r.0).sum::<f64>() / n as f64, rows.iter().map(|r| r.1).sum::<f64>() / n as f64);
        println!("fg-image serve CPU: {:.0} ms/photo one at a time, {:.0} ms/photo in parallel", (cpu1 - cpu0) * 1000.0 / n as f64, (cpu2 - cpu1) * 1000.0 / n as f64);
        let _ = std::fs::remove_dir_all(&tmp);
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
        // 和应用一样：CLIP 走 Metal（FG_GPU=0 时只用 CPU），解码用 ImageIO 边车
        if std::env::var("FG_GPU").map_or(true, |v| v != "0") {
            fg_core::platform::set_gpu_clip(Box::new(crate::clipgpu::Factory));
        }
        crate::imageio_macos::Helper::register(root.join("bin/fg-image"));
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
        // 标记：[开始, "decode"（第一次回调）, "quality"（解码、缩放、缩略图完成）, "ai"（统计完成）, "system"（CLIP 完成，Vision 同时在做）, "thumb"（等到 Vision、人脸完成）]，然后结束
        let names = ["decode+resize+thumb", "quality stats", "CLIP (Vision alongside)", "Vision rest + faces", "tail (exif, media)"];
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


    /// 整条分析流水线在一个真实文件夹上的吞吐（调并行度用）：`FG_LIBRARY=<文件夹> [FG_N=64] [FG_QUALITY=fast] [FG_VISION_WORKERS=n]`，
    /// 睁眼用 MediaPipe、人物识别开着，所有模型都参与。`cargo test -p fleetinggleam --lib whole_scan_on_a_real_folder -- --ignored --nocapture`。
    #[test]
    #[ignore = "timing only: FG_LIBRARY=<folder of photos> [FG_N=64]; models/ and bin/"]
    fn whole_scan_on_a_real_folder() {
        use fg_core::engine::{Engine, NoEvents, Paths, ScanArgs};
        let Ok(dir) = std::env::var("FG_LIBRARY") else { return };
        let n: usize = std::env::var("FG_N").ok().and_then(|v| v.parse().ok()).unwrap_or(64);
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        crate::imageio_macos::Helper::register(root.join("bin/fg-image"));
        fg_core::platform::set_vision(Box::new(crate::vision::Factory { bin: root.join("bin/fg-vision") }));
        fg_core::platform::set_gpu_clip(Box::new(crate::clipgpu::Factory));
        let tmp = std::env::temp_dir().join(format!("fg-whole-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let photos_dir = tmp.join("photos");
        std::fs::create_dir_all(&photos_dir).unwrap();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| is_image(p)).collect();
        files.sort();
        files.truncate(n);
        for f in &files {
            std::fs::copy(f, photos_dir.join(f.file_name().unwrap())).unwrap();
        }
        let e = Engine::new(Paths { data: tmp.join("data"), thumbs: tmp.join("thumbs"), models: dev_models_dir(), downloads: dev_models_dir() }, Box::new(NoEvents), "test");
        let args = ScanArgs {
            key: "bench".into(),
            dirs: vec![photos_dir.to_string_lossy().into_owned()],
            quality: Some(std::env::var("FG_QUALITY").unwrap_or_else(|_| "fast".into())),
            eye_method: Some("mediapipe".into()),
            force: true,
            ..Default::default()
        };
        // 第一遍含模型加载；第二遍（force）才是稳定的吞吐
        let t = std::time::Instant::now();
        let first = e.scan_project(args.clone()).expect("scan");
        let warm = t.elapsed();
        // 第二遍的 CPU 时间：本进程（分析线程）与子进程（边车；分析结束时边车被杀并回收，所以算得进来）
        let (own0, reaped0, live0) = (cpu_seconds(libc::RUSAGE_SELF), cpu_seconds(libc::RUSAGE_CHILDREN), live_helper_cpu());
        let t = std::time::Instant::now();
        let photos = e.scan_project(args).expect("scan");
        let d = t.elapsed();
        let (own, reaped, live) = (cpu_seconds(libc::RUSAGE_SELF) - own0, cpu_seconds(libc::RUSAGE_CHILDREN) - reaped0, live_helper_cpu());
        let live_delta: Vec<String> = live.iter().map(|(n, s)| format!("{n} {:.1} s", s - live0.get(n).copied().unwrap_or(0.0))).collect();
        let live_total: f64 = live.iter().map(|(n, s)| s - live0.get(n).copied().unwrap_or(0.0)).sum();
        println!(
            "{} photos: first pass {:?} (models loading included), second pass {:?} = {:.0} ms/photo, {:.2} photos/s; faces on {} photos, eyes measured on {}, Vision answered on {}",
            photos.len(), warm, d, d.as_secs_f64() * 1000.0 / photos.len() as f64, photos.len() as f64 / d.as_secs_f64(),
            photos.iter().filter(|p| p.face_count > 0).count(), photos.iter().filter(|p| p.eye_method.as_deref() == Some("mediapipe")).count(), photos.iter().filter(|p| p.sys_score.is_some()).count()
        );
        println!(
            "second pass CPU: this process {own:.1} s = {:.1} cores; helpers reaped {reaped:.1} s, still running [{}] = {:.1} cores; together {:.1} cores",
            own / d.as_secs_f64(), live_delta.join(", "), (reaped + live_total) / d.as_secs_f64(), (own + reaped + live_total) / d.as_secs_f64()
        );
        assert_eq!(first.len(), photos.len());
        let _ = std::fs::remove_dir_all(&tmp);
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
