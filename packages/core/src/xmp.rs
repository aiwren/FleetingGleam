//! 写回 Lightroom / Bridge：把选片结果以 XMP 元数据放到照片上——RAW 写旁边的 `.xmp` 旁车文件（Lightroom / Bridge 只认这种），
//! JPEG 写进文件自己的 APP1 XMP 段（它们不读 JPEG 的旁车）。只写两项：`xmp:Rating`（1–5 星；-1 是 Lightroom 的"拒绝"）和
//! `xmp:Label`（颜色标签文字）。旁车或 XMP 段里别的内容原样保留；改之前把原样记下来（`Before`），最近一次可以整体回滚。
//! JPEG 改完把修改时间设回原值，像素数据一个字节不动。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const XMP_NS: &str = "http://ns.adobe.com/xap/1.0/";
/// JPEG 里 XMP 段的签名（APP1，后面跟 \0）。
const JPEG_XMP_SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
/// 一个 JPEG 段最多 65533 字节载荷；留出签名和余量。
const JPEG_PACKET_MAX: usize = 65_000;

/// 要写的内容：`None` 表示这一项不动。
#[derive(Clone, Debug, Default)]
pub struct Mark {
    pub rating: Option<i32>,
    pub label: Option<String>,
}

/// 这张照片的元数据写在哪里。
pub enum Carrier {
    /// RAW（以及其它不能往里写的格式）：旁边的同名 `.xmp`。
    Sidecar(PathBuf),
    /// JPEG：文件自己的 APP1 XMP 段。
    Jpeg,
    /// 不写（HEIC / PNG / WebP 等：往里写要改文件结构，Lightroom 也不读它们的旁车）。
    Unsupported,
}

pub fn carrier_of(p: &Path) -> Carrier {
    let e = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match e.as_str() {
        "jpg" | "jpeg" | "jpe" => Carrier::Jpeg,
        _ if crate::imageio::is_raw(p) || matches!(e.as_str(), "tif" | "tiff") => Carrier::Sidecar(p.with_extension("xmp")),
        _ => Carrier::Unsupported,
    }
}

/// 改动前的原样，回滚用：旁车是整个文件（没有就是 `None`），JPEG 是原来的 XMP 段载荷（没有就是 `None`）以及原来的修改时间。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Before {
    /// 照片本身的路径。
    pub path: String,
    /// `"sidecar"` 或 `"jpeg"`。
    pub carrier: String,
    /// 真正被改的文件：旁车文件，或 JPEG 自己。
    pub file: String,
    /// 原来的内容（旁车全文 / JPEG 的 XMP 段载荷），base64；`None` = 原来没有。
    pub was: Option<String>,
    /// JPEG 原来的修改时间（Unix 秒），回滚后设回去。
    #[serde(default)]
    pub mtime: i64,
}

fn b64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
fn unb64(s: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(s).map_err(|e| format!("undo record is damaged: {e}"))
}

fn mtime_of(p: &Path) -> i64 {
    std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64)
}

fn set_mtime(p: &Path, secs: i64) {
    if secs > 0 {
        if let Ok(f) = std::fs::File::options().write(true).open(p) {
            let _ = f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64));
        }
    }
}

/// 一个从零开始的 XMP 包。
fn fresh_packet(m: &Mark) -> String {
    let mut attrs = String::new();
    if let Some(r) = m.rating {
        attrs.push_str(&format!(" xmp:Rating=\"{r}\""));
    }
    if let Some(l) = &m.label {
        attrs.push_str(&format!(" xmp:Label=\"{}\"", escape(l)));
    }
    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"FleetingGleam\">\n <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  <rdf:Description rdf:about=\"\" xmlns:xmp=\"{XMP_NS}\"{attrs}/>\n </rdf:RDF>\n</x:xmpmeta>\n<?xpacket end=\"w\"?>"
    )
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}

/// 在已有的 XMP 文本里改两项：先删掉旧的（属性形式和元素形式都删），再加到第一个 `rdf:Description` 上；
/// 它没有 `xmp` 命名空间就补上。找不到 `rdf:Description` 就当它不是 XMP，整个换成新包。
fn apply(xml: &str, m: &Mark) -> String {
    if !xml.contains("<rdf:Description") {
        return fresh_packet(m);
    }
    // 只动要写的那两项；`None` 的一项原样保留
    let names: Vec<&str> = [m.rating.is_some().then_some("xmp:Rating"), m.label.is_some().then_some("xmp:Label")].into_iter().flatten().collect();
    // 先删元素形式的旧值：<xmp:Rating>4</xmp:Rating> 或 <xmp:Rating .../>
    let mut xml = xml.to_string();
    for name in &names {
        let open = format!("<{name}");
        let close = format!("</{name}>");
        while let Some(i) = xml.find(&open) {
            let after = &xml[i..];
            let Some(gt) = after.find('>') else { break };
            let end = if after[..gt].ends_with('/') { i + gt + 1 } else if let Some(c) = after.find(&close) { i + c + close.len() } else { break };
            xml.replace_range(i..end, "");
        }
    }
    let start = xml.find("<rdf:Description").unwrap_or(0);
    let tag_end = xml[start..].find('>').map(|i| start + i).unwrap_or(xml.len());
    let tag = &xml[start..tag_end];
    let self_closing = tag.ends_with('/');
    let mut body = if self_closing { tag[..tag.len() - 1].to_string() } else { tag.to_string() };
    // 再删属性形式的旧值
    for name in &names {
        let key = format!(" {name}=\"");
        while let Some(i) = body.find(&key) {
            let Some(j) = body[i + key.len()..].find('"') else { break };
            body.replace_range(i..i + key.len() + j + 1, "");
        }
    }
    if !body.contains("xmlns:xmp=") {
        body.push_str(&format!(" xmlns:xmp=\"{XMP_NS}\""));
    }
    if let Some(r) = m.rating {
        body.push_str(&format!(" xmp:Rating=\"{r}\""));
    }
    if let Some(l) = &m.label {
        body.push_str(&format!(" xmp:Label=\"{}\"", escape(l)));
    }
    if self_closing {
        body.push('/');
    }
    format!("{}{}{}", &xml[..start], body, &xml[tag_end..])
}

/// 读出照片上现在的评分和标签（旁车或 JPEG 段），没有就是空的 `Mark`。
pub fn read(p: &Path) -> Mark {
    let xml = match carrier_of(p) {
        Carrier::Sidecar(sc) => std::fs::read_to_string(sc).unwrap_or_default(),
        Carrier::Jpeg => jpeg_xmp(p).ok().flatten().map(|(_, payload)| String::from_utf8_lossy(&payload).into_owned()).unwrap_or_default(),
        Carrier::Unsupported => String::new(),
    };
    parse_mark(&xml)
}

fn parse_mark(xml: &str) -> Mark {
    let attr = |name: &str| -> Option<String> {
        let key = format!("{name}=\"");
        let i = xml.find(&key)?;
        let rest = &xml[i + key.len()..];
        let j = rest.find('"')?;
        Some(rest[..j].to_string())
    };
    let elem = |name: &str| -> Option<String> {
        let open = format!("<{name}>");
        let i = xml.find(&open)?;
        let rest = &xml[i + open.len()..];
        let j = rest.find('<')?;
        Some(rest[..j].trim().to_string())
    };
    Mark { rating: attr("xmp:Rating").or_else(|| elem("xmp:Rating")).and_then(|s| s.parse().ok()), label: attr("xmp:Label").or_else(|| elem("xmp:Label")).map(|s| s.replace("&quot;", "\"").replace("&amp;", "&")) }
}

/// JPEG 的段表：返回 (段起点, 段总长含标记) 列表，以及 XMP 段（若有）的下标。只走到 SOS 为止。
fn jpeg_segments(bytes: &[u8]) -> Result<Vec<(usize, usize, u8)>, String> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return Err("not a JPEG".to_string());
    }
    let mut segs = Vec::new();
    let mut pos = 2;
    while pos + 4 <= bytes.len() {
        if bytes[pos] != 0xFF {
            return Err("damaged JPEG header".to_string());
        }
        let marker = bytes[pos + 1];
        if marker == 0xFF {
            pos += 1;
            continue;
        }
        if marker == 0xD8 || (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            segs.push((pos, 2, marker));
            pos += 2;
            continue;
        }
        let len = u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]) as usize;
        if len < 2 || pos + 2 + len > bytes.len() {
            return Err("damaged JPEG header".to_string());
        }
        segs.push((pos, 2 + len, marker));
        if marker == 0xDA {
            break; // 扫描数据开始，后面不再有元数据段
        }
        pos += 2 + len;
    }
    Ok(segs)
}

/// JPEG 里现有的 XMP 段：(段下标, 载荷)。
fn jpeg_xmp(p: &Path) -> Result<Option<(usize, Vec<u8>)>, String> {
    let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
    let segs = jpeg_segments(&bytes)?;
    for (i, &(start, len, marker)) in segs.iter().enumerate() {
        if marker == 0xE1 && bytes[start + 4..start + len].starts_with(JPEG_XMP_SIG) {
            return Ok(Some((i, bytes[start + 4 + JPEG_XMP_SIG.len()..start + len].to_vec())));
        }
    }
    Ok(None)
}

/// JPEG 改写用的临时文件：同目录、隐藏、仍以 .jpg 结尾——安卓的 DCIM / Pictures 只许应用建图片文件，别的扩展名直接拒绝。
fn jpeg_tmp(p: &Path) -> PathBuf {
    p.with_file_name(format!(".{}.fgtmp.jpg", p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))
}

/// 把 XMP 段写进 JPEG：有就原地换，没有就插在 APP0 / Exif 之后、其它段之前。写到临时文件再换名，修改时间设回原值。
fn jpeg_write(p: &Path, packet: &[u8], original_mtime: i64) -> Result<(), String> {
    if packet.len() > JPEG_PACKET_MAX {
        return Err("the XMP packet is too large for a JPEG segment".to_string());
    }
    let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
    let segs = jpeg_segments(&bytes)?;
    let mut seg = Vec::with_capacity(4 + JPEG_XMP_SIG.len() + packet.len());
    seg.extend_from_slice(&[0xFF, 0xE1]);
    seg.extend_from_slice(&((2 + JPEG_XMP_SIG.len() + packet.len()) as u16).to_be_bytes());
    seg.extend_from_slice(JPEG_XMP_SIG);
    seg.extend_from_slice(packet);
    let mut out = Vec::with_capacity(bytes.len() + seg.len());
    out.extend_from_slice(&bytes[..2]);
    let existing = segs.iter().position(|&(s, l, m)| m == 0xE1 && bytes[s + 4..s + l].starts_with(JPEG_XMP_SIG));
    // 插在最后一个 APP0 / Exif(APP1 但不是 XMP) 之后
    let insert_after = segs.iter().enumerate().filter(|(i, &(_, _, m))| Some(*i) != existing && (m == 0xE0 || m == 0xE1)).map(|(i, _)| i).last();
    let mut inserted = false;
    for (i, &(start, len, _)) in segs.iter().enumerate() {
        if Some(i) == existing {
            out.extend_from_slice(&seg);
            inserted = true;
            continue;
        }
        out.extend_from_slice(&bytes[start..start + len]);
        if !inserted && existing.is_none() && Some(i) == insert_after {
            out.extend_from_slice(&seg);
            inserted = true;
        }
    }
    if !inserted {
        // 没有任何 APP 段：紧跟 SOI
        let tail = out.split_off(2);
        out.extend_from_slice(&seg);
        out.extend_from_slice(&tail);
    }
    let last = segs.last().map(|&(s, l, _)| s + l).unwrap_or(2);
    out.extend_from_slice(&bytes[last..]);
    let tmp = jpeg_tmp(p);
    crate::files::write_synced(&tmp, &out).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, p).map_err(|e| e.to_string())?;
    set_mtime(p, original_mtime);
    Ok(())
}

/// 把 `m` 写到照片上，返回改之前的原样（回滚用）。不支持的格式返回 `Ok(None)`。
pub fn write(p: &Path, m: &Mark) -> Result<Option<Before>, String> {
    match carrier_of(p) {
        Carrier::Unsupported => Ok(None),
        Carrier::Sidecar(sc) => {
            let was = std::fs::read(&sc).ok();
            let xml = match &was {
                Some(bytes) => apply(&String::from_utf8_lossy(bytes), m),
                None => fresh_packet(m),
            };
            let tmp = sc.with_extension("xmp.tmp");
            crate::files::write_synced(&tmp, xml.as_bytes()).map_err(|e| e.to_string())?;
            std::fs::rename(&tmp, &sc).map_err(|e| e.to_string())?;
            Ok(Some(Before { path: p.to_string_lossy().into_owned(), carrier: "sidecar".into(), file: sc.to_string_lossy().into_owned(), was: was.as_deref().map(b64), mtime: 0 }))
        }
        Carrier::Jpeg => {
            let mtime = mtime_of(p);
            let was = jpeg_xmp(p)?.map(|(_, payload)| payload);
            let packet = match &was {
                Some(bytes) => apply(&String::from_utf8_lossy(bytes), m),
                None => fresh_packet(m),
            };
            jpeg_write(p, packet.as_bytes(), mtime)?;
            Ok(Some(Before { path: p.to_string_lossy().into_owned(), carrier: "jpeg".into(), file: p.to_string_lossy().into_owned(), was: was.as_deref().map(b64), mtime }))
        }
    }
}

/// 回滚一次写回：旁车放回原样（原来没有就删掉），JPEG 的 XMP 段放回原样（原来没有就去掉）。
pub fn restore(b: &Before) -> Result<(), String> {
    let file = Path::new(&b.file);
    match b.carrier.as_str() {
        "sidecar" => match &b.was {
            Some(s) => {
                let bytes = unb64(s)?;
                crate::files::write_synced(file, &bytes).map_err(|e| e.to_string())
            }
            None => match std::fs::remove_file(file) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.to_string()),
            },
        },
        "jpeg" => match &b.was {
            Some(s) => jpeg_write(file, &unb64(s)?, b.mtime),
            None => jpeg_strip(file, b.mtime),
        },
        other => Err(format!("unknown carrier {other}")),
    }
}

/// 去掉 JPEG 里的 XMP 段（回滚"原来没有"的情况）。
fn jpeg_strip(p: &Path, original_mtime: i64) -> Result<(), String> {
    let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
    let segs = jpeg_segments(&bytes)?;
    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&bytes[..2]);
    for &(start, len, marker) in &segs {
        if marker == 0xE1 && bytes[start + 4..start + len].starts_with(JPEG_XMP_SIG) {
            continue;
        }
        out.extend_from_slice(&bytes[start..start + len]);
    }
    let last = segs.last().map(|&(s, l, _)| s + l).unwrap_or(2);
    out.extend_from_slice(&bytes[last..]);
    let tmp = jpeg_tmp(p);
    crate::files::write_synced(&tmp, &out).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, p).map_err(|e| e.to_string())?;
    set_mtime(p, original_mtime);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fg_xmp_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    /// 一个最小的合法 JPEG：SOI、APP0(JFIF)、SOS、一点数据、EOI。
    fn tiny_jpeg() -> Vec<u8> {
        let mut v = vec![0xFF, 0xD8];
        v.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
        v.extend_from_slice(b"JFIF\0\x01\x01\x00\x00\x01\x00\x01\x00\x00");
        v.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
        v.extend_from_slice(&[0x12, 0x34, 0x56]);
        v.extend_from_slice(&[0xFF, 0xD9]);
        v
    }

    #[test]
    fn a_sidecar_is_made_updated_and_restored() {
        let raw = tmp("IMG_1.CR3");
        std::fs::write(&raw, b"raw").unwrap();
        let sc = raw.with_extension("xmp");
        let _ = std::fs::remove_file(&sc);
        let b1 = write(&raw, &Mark { rating: Some(4), label: Some("Green".into()) }).unwrap().unwrap();
        assert!(b1.was.is_none());
        let m = read(&raw);
        assert_eq!((m.rating, m.label.as_deref()), (Some(4), Some("Green")));
        // Lightroom 之类已经写过的旁车：别的内容保留，只换我们的两项
        let lr = "<?xpacket begin=\"\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?><x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description rdf:about=\"\" xmlns:crs=\"http://ns.adobe.com/camera-raw-settings/1.0/\" xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" crs:Temperature=\"5200\" xmp:Rating=\"2\"><xmp:Label>Blue</xmp:Label></rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end=\"w\"?>";
        std::fs::write(&sc, lr).unwrap();
        let b2 = write(&raw, &Mark { rating: Some(-1), label: Some("Red".into()) }).unwrap().unwrap();
        let now = std::fs::read_to_string(&sc).unwrap();
        assert!(now.contains("crs:Temperature=\"5200\""), "other settings stay: {now}");
        assert!(now.contains("xmp:Rating=\"-1\"") && now.contains("xmp:Label=\"Red\"") && !now.contains("<xmp:Label>"), "{now}");
        assert_eq!(now.matches("xmp:Rating=").count(), 1);
        restore(&b2).unwrap();
        assert_eq!(std::fs::read_to_string(&sc).unwrap(), lr);
        restore(&b1).unwrap();
        assert!(!sc.exists(), "the sidecar we created is removed on undo");
    }

    #[test]
    fn a_jpeg_gets_an_xmp_segment_and_keeps_its_pixels_and_mtime() {
        let jpg = tmp("IMG_2.jpg");
        std::fs::write(&jpg, tiny_jpeg()).unwrap();
        set_mtime(&jpg, 1_700_000_000);
        let b1 = write(&jpg, &Mark { rating: Some(5), label: Some("Green".into()) }).unwrap().unwrap();
        let bytes = std::fs::read(&jpg).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
        assert!(bytes.ends_with(&[0x12, 0x34, 0x56, 0xFF, 0xD9]), "scan data untouched");
        assert_eq!(mtime_of(&jpg), 1_700_000_000, "modification time preserved");
        let m = read(&jpg);
        assert_eq!((m.rating, m.label.as_deref()), (Some(5), Some("Green")));
        // 再写一次：段原地换，不会多出第二个
        let b2 = write(&jpg, &Mark { rating: Some(3), label: None }).unwrap().unwrap();
        assert!(b2.was.is_some());
        let bytes = std::fs::read(&jpg).unwrap();
        assert_eq!(jpeg_segments(&bytes).unwrap().iter().filter(|s| s.2 == 0xE1).count(), 1);
        assert_eq!(read(&jpg).rating, Some(3));
        assert_eq!(read(&jpg).label.as_deref(), Some("Green"), "a None leaves that item alone");
        restore(&b2).unwrap();
        assert_eq!(read(&jpg).rating, Some(5));
        restore(&b1).unwrap();
        assert_eq!(std::fs::read(&jpg).unwrap(), tiny_jpeg(), "back to the exact original bytes");
    }

    #[test]
    fn unsupported_formats_are_skipped() {
        assert!(matches!(carrier_of(Path::new("a.heic")), Carrier::Unsupported));
        assert!(matches!(carrier_of(Path::new("a.NEF")), Carrier::Sidecar(_)));
        assert!(matches!(carrier_of(Path::new("a.JPG")), Carrier::Jpeg));
        assert!(write(Path::new("/nonexistent/a.png"), &Mark::default()).unwrap().is_none());
    }
}
