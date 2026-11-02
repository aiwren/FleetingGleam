//! 相机 RAW 里内嵌的 JPEG 预览（大 RAW 秒开，路线图 A7）。
//!
//! 几乎所有 RAW（NEF / CR2 / CR3 / ARW / RAF / ORF / RW2 / PEF / DNG……）都带一张相机自己压好的 JPEG，多数是全尺寸或长边 1600 以上。
//! 这里不解析每家的容器格式，而是在文件里找 JPEG 流（SOI `FF D8 FF` 到 EOI `FF D9`），取尺寸最大的那张：查看器的大图先用它，
//! 不等几秒钟的完整解码；放大看细节时仍走 `photo_detail` 解真正的 RAW。RAW 自己的无损压缩数据也是 JPEG 流（SOF3），这里不要。
//!
//! 预览像素通常没摆正：方向先看预览 JPEG 自己的 EXIF，没有就看 RAW 容器的 EXIF（TIFF 系直接读；ORF / RW2 的魔数改成 TIFF 的再读；
//! CR3 读 CMT1 盒里的 IFD），还拿不准就按分析时记下的显示尺寸（横竖）兜底。

use std::io::Cursor;
use std::path::Path;

/// 预览至少这么大才值得用（再小就不如完整解码一次）。
pub const MIN_LONG_EDGE: u32 = 1600;

/// 一张内嵌预览：JPEG 字节、帧头里的尺寸（还没摆正）、要顺时针转多少度才正（0 / 90 / 180 / 270）。
pub struct Embedded {
    pub bytes: Vec<u8>,
    pub w: u32,
    pub h: u32,
    pub rotation: u16,
}

/// 帧头：尺寸、分量数、是不是有损 JPEG（SOF0 / SOF1 / SOF2）。
struct Frame {
    w: u32,
    h: u32,
    lossy: bool,
    /// 预览 JPEG 自己 EXIF 里的方向（EXIF 的 1–8），没有就是 None
    orientation: Option<u32>,
    /// SOS 段开始的位置（相对流起点）
    sos: usize,
}

/// 读一个 JPEG 流的头：段落逐个往后走，到 SOS 为止。读不懂给 None。
fn frame(b: &[u8]) -> Option<Frame> {
    if b.len() < 4 || b[0] != 0xFF || b[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    let mut dims: Option<(u32, u32, bool)> = None;
    let mut orientation = None;
    loop {
        if i + 4 > b.len() || b[i] != 0xFF {
            return None;
        }
        let mut marker = b[i + 1];
        i += 2;
        while marker == 0xFF {
            marker = *b.get(i)?;
            i += 1;
        }
        if marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        if marker == 0xD9 {
            return None;
        }
        if marker == 0xDA {
            let (w, h, lossy) = dims?;
            return Some(Frame { w, h, lossy, orientation, sos: i - 2 });
        }
        let len = u16::from_be_bytes([*b.get(i)?, *b.get(i + 1)?]) as usize;
        if len < 2 || i + len > b.len() {
            return None;
        }
        let body = &b[i + 2..i + len];
        match marker {
            0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => {
                if body.len() < 6 {
                    return None;
                }
                let (h, w) = (u16::from_be_bytes([body[1], body[2]]) as u32, u16::from_be_bytes([body[3], body[4]]) as u32);
                let comps = body[5];
                if w == 0 || h == 0 {
                    return None;
                }
                // 只要有损的彩色 JPEG：SOF3（无损）是 RAW 的像素数据，不是预览
                dims = Some((w, h, matches!(marker, 0xC0 | 0xC1 | 0xC2) && comps == 3));
            }
            0xE1 if body.starts_with(b"Exif\0\0") => {
                if let Ok(ex) = exif::Reader::new().read_raw(body[6..].to_vec()) {
                    orientation = ex.get_field(exif::Tag::Orientation, exif::In::PRIMARY).and_then(|f| f.value.get_uint(0));
                }
            }
            _ => {}
        }
        i += len;
    }
}

/// 从 SOS 往后找 EOI：熵编码数据里 0xFF 后面只会是 0x00 或 RST（D0–D7），所以第一个 `FF D9` 就是结尾。
fn end_of_stream(b: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while let Some(off) = memchr::memchr(0xFF, &b[i..]) {
        let at = i + off;
        match b.get(at + 1) {
            Some(0xD9) => return Some(at + 2),
            Some(_) => i = at + 1,
            None => return None,
        }
    }
    None
}

/// EXIF 的方向值换成顺时针角度（镜像的按不镜像算）。
fn degrees(orientation: u32) -> u16 {
    match orientation {
        3 | 4 => 180,
        5 | 6 => 90,
        7 | 8 => 270,
        _ => 0,
    }
}

/// RAW 容器自己记的方向。TIFF 系（NEF / CR2 / ARW / PEF / DNG……）直接读；ORF（`IIRO` / `IIRS`）和 RW2（`IIU\0`）
/// 的结构是 TIFF、只是魔数不同，改回 TIFF 的魔数再读；CR3 的 EXIF 在 `CMT1` 盒里（盒内就是一个 TIFF 结构）。
fn container_rotation(b: &[u8]) -> Option<u16> {
    let head = &b[..b.len().min(8 * 1024 * 1024)];
    let read = |bytes: &[u8]| exif::Reader::new().read_raw(bytes.to_vec()).ok();
    let ex = if head.starts_with(b"II*\0") || head.starts_with(b"MM\0*") {
        exif::Reader::new().read_from_container(&mut Cursor::new(head)).ok()
    } else if head.starts_with(b"IIRO") || head.starts_with(b"IIRS") || head.starts_with(b"IIU\0") {
        let mut patched = head.to_vec();
        patched[2] = b'*';
        patched[3] = 0;
        read(&patched)
    } else if head.starts_with(b"FUJIFILM") {
        // RAF：头里记着内嵌 JPEG 的偏移（第 84 字节起，大端），那张 JPEG 自带 EXIF，`frame` 已经读了；这里不用再找
        None
    } else {
        // CR3（ISO BMFF）：找 CMT1 盒，盒头 8 字节后就是 TIFF 结构
        let tag = memchr::memmem::find(head, b"CMT1")?;
        read(&head[tag + 4..])
    }?;
    ex.get_field(exif::Tag::Orientation, exif::In::PRIMARY).and_then(|f| f.value.get_uint(0)).map(degrees)
}

/// 文件里尺寸最大的有损 JPEG 流（长边至少 `MIN_LONG_EDGE`），连同它该转多少度。
/// `expect` 是分析时记下的显示尺寸（宽、高），转完后横竖对不上时据此纠正；没有就传 None。
pub fn embedded_jpeg(path: &Path, expect: Option<(u32, u32)>) -> Option<Embedded> {
    let b = std::fs::read(path).ok()?;
    embedded_in(&b, expect)
}

fn embedded_in(b: &[u8], expect: Option<(u32, u32)>) -> Option<Embedded> {
    let mut best: Option<(usize, Frame)> = None;
    let mut i = 0;
    while let Some(off) = memchr::memmem::find(&b[i..], &[0xFF, 0xD8, 0xFF]) {
        let at = i + off;
        i = at + 2;
        let Some(f) = frame(&b[at..]) else { continue };
        if !f.lossy || f.w.max(f.h) < MIN_LONG_EDGE {
            continue;
        }
        if best.as_ref().map_or(true, |(_, g)| f.w * f.h > g.w * g.h) {
            best = Some((at, f));
        }
    }
    let (at, f) = best?;
    let end = end_of_stream(b, at + f.sos)?;
    let bytes = b[at..end].to_vec();
    // 方向：预览自己的 EXIF 优先，其次容器的 EXIF
    let mut rotation = f.orientation.map(degrees).or_else(|| container_rotation(b)).unwrap_or(0);
    // 兜底：转完的横竖要和分析时的显示尺寸一致（CR3 / 少见格式读不到方向时靠这个）
    if let Some((ew, eh)) = expect {
        if ew != eh && f.w != f.h {
            let landscape_after = (f.w > f.h) == (rotation % 180 == 0);
            if landscape_after != (ew > eh) {
                rotation = if rotation % 180 == 0 { 90 } else { 0 };
            }
        }
    }
    Some(Embedded { bytes, w: f.w, h: f.h, rotation })
}

/// 把内嵌预览写成查看器要的大图：不用转的直接落盘（零解码，秒开）；要转的解出来转正、长边缩到 `max` 再存。
/// 没有合用的预览（太小、找不到）给 Err，调用方照旧完整渲染。
pub fn write_preview(src: &Path, out: &Path, expect: Option<(u32, u32)>, max: u32) -> Result<(), String> {
    let e = embedded_jpeg(src, expect).ok_or_else(|| format!("{}: no embedded preview", src.display()))?;
    if e.rotation == 0 && e.w.max(e.h) <= max {
        return crate::files::write_synced(out, &e.bytes).map_err(|err| err.to_string());
    }
    let img = image::load_from_memory_with_format(&e.bytes, image::ImageFormat::Jpeg).map_err(|err| format!("{}: {err}", src.display()))?;
    let img = crate::analyze::fast_resize(&crate::analyze::rotate_image(img, e.rotation), max.min(2560));
    let file = std::fs::File::create(out).map_err(|err| err.to_string())?;
    image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(file), 88).encode_image(&img.to_rgb8()).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一张最小的有损 JPEG 头（SOF0，w × h，3 分量），后面接 SOS 和 EOI。
    fn jpeg(w: u16, h: u16, sof: u8, comps: u8, exif_orientation: Option<u8>) -> Vec<u8> {
        let mut v = vec![0xFF, 0xD8];
        if let Some(o) = exif_orientation {
            // APP1 Exif：小端 TIFF 头 + 一条 Orientation 记录
            let tiff: Vec<u8> = [b"II*\0".as_slice(), &8u32.to_le_bytes(), &1u16.to_le_bytes(), &0x0112u16.to_le_bytes(), &3u16.to_le_bytes(), &1u32.to_le_bytes(), &[o, 0, 0, 0], &0u32.to_le_bytes()].concat();
            let body = [b"Exif\0\0".as_slice(), &tiff].concat();
            v.extend([0xFF, 0xE1]);
            v.extend(((body.len() + 2) as u16).to_be_bytes());
            v.extend(body);
        }
        v.extend([0xFF, sof, 0, 17, 8]);
        v.extend(h.to_be_bytes());
        v.extend(w.to_be_bytes());
        v.extend([comps, 1, 0x11, 0, 2, 0x11, 0, 3, 0x11, 0]);
        v.extend([0xFF, 0xDA, 0, 2, 0x12, 0xFF, 0x00, 0x34, 0xFF, 0xD9]);
        v
    }

    #[test]
    fn picks_the_largest_lossy_stream_and_skips_raw_data() {
        let small = jpeg(160, 120, 0xC0, 3, None);
        let big = jpeg(6000, 4000, 0xC0, 3, None);
        let lossless = jpeg(8000, 6000, 0xC3, 2, None); // RAW 像素数据
        let file = [b"II*\0junk".as_slice(), &small, b"xx", &lossless, &big, b"tail"].concat();
        let e = embedded_in(&file, None).expect("a preview");
        assert_eq!((e.w, e.h, e.rotation), (6000, 4000, 0));
        assert_eq!(e.bytes, big, "the stream is cut at its own EOI");
        assert!(embedded_in(&[b"II*\0".as_slice(), &small].concat(), None).is_none(), "a thumbnail alone is too small to use");
    }

    #[test]
    fn rotation_comes_from_the_previews_exif_then_the_expected_shape() {
        let turned = jpeg(6000, 4000, 0xC0, 3, Some(6));
        let e = embedded_in(&[b"FUJIFILM".as_slice(), &turned].concat(), None).unwrap();
        assert_eq!(e.rotation, 90, "orientation 6 = rotate 90° clockwise");
        // 没有任何方向信息、分析时记的是竖幅：按显示尺寸转 90°
        let plain = jpeg(6000, 4000, 0xC0, 3, None);
        let e = embedded_in(&[b"zzzz".as_slice(), &plain].concat(), Some((4000, 6000))).unwrap();
        assert_eq!(e.rotation, 90);
        // 预览的 EXIF 说转 90° 而显示尺寸是横幅：显示尺寸说了算
        let e = embedded_in(&[b"FUJIFILM".as_slice(), &turned].concat(), Some((6000, 4000))).unwrap();
        assert_eq!(e.rotation, 0);
    }

    /// 真实样片：`FG_RAW_DIR=目录 cargo test --lib rawpreview::tests::real_samples -- --ignored --nocapture`，
    /// 每个文件打印预览尺寸、方向和耗时，并把预览写到同目录的 `<名字>.preview.jpg`。
    #[test]
    #[ignore]
    fn real_samples() {
        let dir = std::env::var("FG_RAW_DIR").expect("FG_RAW_DIR");
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = entry.path();
            if !crate::imageio::is_raw(&p) {
                continue;
            }
            let t = std::time::Instant::now();
            match embedded_jpeg(&p, None) {
                Some(e) => {
                    let out = p.with_extension("preview.jpg");
                    write_preview(&p, &out, None, 30000).unwrap();
                    println!("{}: {}x{} rot {} ({} KB) in {:?}", p.file_name().unwrap().to_string_lossy(), e.w, e.h, e.rotation, e.bytes.len() / 1024, t.elapsed());
                }
                None => println!("{}: no preview ({:?})", p.display(), t.elapsed()),
            }
        }
    }
}
