//! 对真实相机 JPEG 的端到端检查（手动运行）：FG_XMP_JPEG=路径 cargo test --test xmp_e2e -- --ignored --nocapture
use std::path::Path;

#[test]
#[ignore]
fn a_real_camera_jpeg_round_trips() {
    let p = std::env::var("FG_XMP_JPEG").expect("FG_XMP_JPEG");
    let p = Path::new(&p);
    let before_bytes = std::fs::read(p).unwrap();
    let before_img = image::open(p).unwrap().to_rgb8();
    let m0 = fg_core::xmp::read(p);
    println!("before: {:?} / {:?}", m0.rating, m0.label);
    let b = fg_core::xmp::write(p, &fg_core::xmp::Mark { rating: Some(4), label: Some("Green".into()) }).unwrap().unwrap();
    let m1 = fg_core::xmp::read(p);
    println!("after: {:?} / {:?}  size {} -> {}", m1.rating, m1.label, before_bytes.len(), std::fs::metadata(p).unwrap().len());
    assert_eq!((m1.rating, m1.label.as_deref()), (Some(4), Some("Green")));
    let after_img = image::open(p).unwrap().to_rgb8();
    assert_eq!(before_img.as_raw(), after_img.as_raw(), "pixels identical");
    // 段顺序：XMP 应紧跟 Exif 之后
    let bytes = std::fs::read(p).unwrap();
    let mut pos = 2;
    let mut order = vec![];
    while pos + 4 <= bytes.len() && bytes[pos] == 0xFF {
        let m = bytes[pos + 1];
        let len = u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]) as usize;
        let tag = if m == 0xE1 { if bytes[pos + 4..].starts_with(b"http://ns.adobe.com/xap/1.0/\0") { "APP1(XMP)".to_string() } else { "APP1(Exif)".to_string() } } else { format!("{m:02X}") };
        order.push(tag);
        if m == 0xDA { break; }
        pos += 2 + len;
    }
    println!("segments: {}", order.join(" "));
    fg_core::xmp::restore(&b).unwrap();
    assert_eq!(std::fs::read(p).unwrap(), before_bytes, "restored byte-for-byte");
}
