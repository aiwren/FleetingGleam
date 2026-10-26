//! Models are downloaded when they are first needed, one pack at a time, not shipped inside the app and not fetched all at once.
//! Every file has a fixed address, size and SHA-256 in `models.manifest.json`, which is built into the app. Most come straight from
//! the public repositories they were published in (Hugging Face). A download is checked against its SHA-256 and only then moved into
//! place, so a broken or tampered file can never be used. Photos are never sent anywhere: this is the only network traffic the app
//! makes, and it only ever fetches these files.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::modelfiles;

pub struct Pack {
    pub id: &'static str,
    pub files: &'static [&'static str],
}

pub const PACKS: [Pack; 8] = [
    Pack { id: "standard", files: &["clip_vit_b32_vision_fp16.onnx", "clip_vit_b32_vision_int8.onnx", "aesthetic_head_vit_b_32.json"] },
    Pack { id: "high", files: &["clip_vit_l14_vision_fp16.onnx", "aesthetic_head_vit_l_14.json"] },
    Pack { id: "ultra", files: &["siglip_so400m_vision_fp16.onnx", "aesthetic_v25_head.json"] },
    Pack { id: "search", files: &["text_multilingual_qint8.onnx", "text_multilingual_tokenizer.json", "text_multilingual_dense.bin"] },
    Pack { id: "eyes", files: &[crate::mediapipe::LANDMARKS_FILE, crate::mediapipe::BLENDSHAPES_FILE] },
    Pack { id: "faces", files: &[crate::faces::MODEL_FILE] },
    // 没有系统人脸检测的平台（Android）用它找脸，再交给 MediaPipe 的网格模型（`eyes` 包）量眼睛、定五官
    Pack { id: "detect", files: &[crate::facepipe::DETECTOR_FILE] },
    Pack { id: "scenetext", files: &[
        crate::florence::VISION, crate::florence::EMBED, crate::florence::ENCODER, crate::florence::DECODER, crate::florence::TOKENIZER,
        crate::translate::ENCODER, crate::translate::DECODER, crate::translate::TOKENIZER,
    ] },
];

pub fn pack(id: &str) -> Option<&'static Pack> {
    PACKS.iter().find(|p| p.id == id)
}

/// 一组里一台设备只用一份的文件：standard 包的 CLIP B/32 有 fp16 和 int8 两份（176 MB + 89 MB），以前两份都下。
/// 用哪份看平台（`scorer::b32_file_for_device`）：有加速器的（Mac 的 CoreML / Metal、Android 的 QNN，QNN 起不来时 CPU 上也是跑 fp16）
/// 用 fp16，没有加速器的用 int8。已经两份都在的老安装不动；装了另一份的也算装好（`scorer` 两份都会用）。
const ONE_OF: [&[&str]; 1] = [&[crate::scorer::B32_FP16_FILE, crate::scorer::INT8_FILE]];

fn group_of(f: &str) -> Option<&'static [&'static str]> {
    ONE_OF.iter().copied().find(|g| g.contains(&f))
}

/// 这台设备要的那份：不在任何一组里的都要；组里只要这台设备用的那份。
fn wanted_here(f: &str) -> bool {
    group_of(f).map_or(true, |_| f == crate::scorer::b32_file_for_device())
}

#[derive(Serialize)]
pub struct PackStatus {
    pub id: String,
    pub installed: bool,
    /// 整包随应用安装（不用下载，也不能删）。
    #[serde(default)]
    pub bundled: bool,
    /// Bytes still to download (known once the manifest was read; otherwise 0).
    pub bytes: u64,
}

pub fn installed(dir: &Path, p: &Pack) -> bool {
    p.files.iter().all(|f| match group_of(f) {
        Some(g) => g.iter().any(|x| modelfiles::has_model(dir, x)),
        None => modelfiles::has_model(dir, f),
    })
}

/// 整包都随应用带着。
pub fn bundled(p: &Pack) -> bool {
    p.files.iter().all(|f| modelfiles::bundled(f).is_some())
}

/// 清单里某个文件应有的 SHA-256（测试用）。
pub fn manifest_sha(name: &str) -> Option<String> {
    manifest().get(name).map(|i| i.sha256.to_lowercase())
}

#[derive(Deserialize)]
struct FileInfo {
    /// None for the files that ship inside the app.
    url: Option<String>,
    size: u64,
    sha256: String,
}

/// The built-in list of every model file: where it comes from, how big it is and what its SHA-256 must be.
fn manifest() -> &'static HashMap<String, FileInfo> {
    static MANIFEST: OnceLock<HashMap<String, FileInfo>> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        #[derive(Deserialize)]
        struct Manifest {
            files: HashMap<String, FileInfo>,
        }
        serde_json::from_str::<Manifest>(include_str!("../models.manifest.json")).expect("models.manifest.json is not valid").files
    })
}

#[derive(Serialize, Clone)]
pub struct Progress {
    pub pack: String,
    pub file: String,
    pub done: u64,
    pub total: u64,
}

/// 网络出错（连不上、断了、服务器 5xx）后最多再试几次，每次等得更久（1、2、4 秒）。
const RETRIES: u32 = 3;
#[cfg(not(test))]
const BACKOFF: std::time::Duration = std::time::Duration::from_secs(1);
#[cfg(test)]
const BACKOFF: std::time::Duration = std::time::Duration::from_millis(20);
/// 下载前要的空间是还要下的字节数乘上它：留点余地给系统和别的写入。
const SPACE_MARGIN: f64 = 1.1;

/// 一次请求的失败：网络的可以重试，别的（磁盘满、取消、校验失败）不重试。
enum Fail {
    Net(String),
    Fatal(String),
}

/// 下了一半的文件：`名字.part`，取消、断网都留着，下次从它的末尾接着下。
fn part_of(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.part"))
}

/// 已经下了多少（不超过应有大小；更大的说明不对，当作没有）。
fn have(part: &Path, size: u64) -> u64 {
    std::fs::metadata(part).map(|m| m.len()).ok().filter(|&n| n <= size).unwrap_or(0)
}

/// The files of a pack that still have to be downloaded, with what the manifest says about each.
fn missing<'a>(dir: &Path, p: &'a Pack) -> Result<Vec<(&'a str, &'static FileInfo, &'static str)>, String> {
    let m = manifest();
    p.files
        .iter()
        .filter(|f| wanted_here(f) && !modelfiles::has_model(dir, f))
        .filter(|f| group_of(f).map_or(true, |g| !g.iter().any(|x| modelfiles::has_model(dir, x))))
        .map(|f| {
            let info = m.get(*f).ok_or_else(|| format!("{f} is not in the model list"))?;
            let url = info.url.as_deref().ok_or_else(|| format!("{f} has no download address"))?;
            Ok((*f, info, url))
        })
        .collect()
}

/// How many bytes downloading this pack would take on this device (what is already in `dir`, and what earlier attempts left in `.part`
/// files, is not counted).
pub fn manifest_sizes(dir: &Path, id: &str) -> Result<u64, String> {
    let p = pack(id).ok_or("unknown pack")?;
    Ok(missing(dir, p)?.iter().map(|(name, i, _)| i.size - have(&part_of(dir, name), i.size)).sum())
}

/// Downloads what a pack is missing into `dir`. 先看空间（还要下的 × 1.1），不够就 `space:` 错误、一个字节都不下；
/// 每个文件断点续传、网络出错退避重试，校验通过才改成正式名字。`cancel` stops it between chunks, keeping what arrived.
/// 错误都带 `errors` 约定的前缀。
pub fn download(dir: &Path, id: &str, cancel: &AtomicBool, on_progress: &dyn Fn(Progress)) -> Result<(), String> {
    let p = pack(id).ok_or("unknown pack")?;
    std::fs::create_dir_all(dir).map_err(|e| crate::errors::io(&e, dir, 0))?;
    let todo = missing(dir, p)?;
    let total: u64 = todo.iter().map(|(_, i, _)| i.size).sum();
    let left: u64 = todo.iter().map(|(name, i, _)| i.size - have(&part_of(dir, name), i.size)).sum();
    crate::errors::check_space(dir, (left as f64 * SPACE_MARGIN).ceil() as u64)?;
    let mut done_before = 0u64;
    for (name, info, url) in todo {
        let part = part_of(dir, name);
        fetch(url, &part, info.size, &info.sha256, cancel, &|got| on_progress(Progress { pack: id.into(), file: name.into(), done: done_before + got, total }))
            .map_err(|e| crate::errors::context(name, &e))?;
        std::fs::rename(&part, dir.join(name)).map_err(|e| crate::errors::io(&e, dir, 0))?;
        done_before += info.size;
        on_progress(Progress { pack: id.into(), file: name.into(), done: done_before, total });
    }
    Ok(())
}

/// 把 `url` 下到 `part`（`size` 字节、SHA-256 是 `sha`）：接着 `part` 里已有的下（`Range`），网络出错退避重试 `RETRIES` 次，
/// 下完校验；校验不对才删掉 `part`（`verify:`），别的失败都留着它，下次接着下。`progress` 收到的是这个文件已有的字节数。
fn fetch(url: &str, part: &Path, size: u64, sha: &str, cancel: &AtomicBool, progress: &dyn Fn(u64)) -> Result<(), String> {
    let mut tries = 0;
    loop {
        match fetch_once(url, part, size, cancel, progress) {
            Ok(()) => break,
            Err(Fail::Net(e)) if tries < RETRIES => {
                let wait = BACKOFF * 2u32.pow(tries);
                tries += 1;
                crate::platform::log(&format!("模型下载出错，{} 秒后第 {tries} 次重试：{e}", wait.as_secs_f32()));
                let until = std::time::Instant::now() + wait;
                while std::time::Instant::now() < until {
                    if cancel.load(Ordering::Relaxed) {
                        return Err(crate::errors::CANCELLED.into());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50).min(wait));
                }
            }
            Err(Fail::Net(e)) => return Err(format!("net:{e}")),
            Err(Fail::Fatal(e)) => return Err(e),
        }
    }
    let got = modelfiles::sha256_file(part).map_err(|e| format!("verify:{e}"))?;
    if got != sha.to_lowercase() {
        let _ = std::fs::remove_file(part);
        return Err("verify:the file did not match its checksum".into());
    }
    Ok(())
}

/// 一次请求：从 `part` 的末尾接着要，收到的追加进去，直到 `size` 字节。服务器不认 `Range`（回 200）就从头写。
fn fetch_once(url: &str, part: &Path, size: u64, cancel: &AtomicBool, progress: &dyn Fn(u64)) -> Result<(), Fail> {
    let mut got = have(part, size);
    if got == size && size > 0 {
        return Ok(());
    }
    let agent = crate::platform::http_agent(std::time::Duration::from_secs(15), std::time::Duration::from_secs(30));
    let mut req = agent.get(url);
    if got > 0 {
        req = req.set("Range", &format!("bytes={got}-"));
    }
    let resp = match req.call() {
        Ok(r) => r,
        // 要的范围不对（服务器上的文件变了，或者 `.part` 坏了）：扔掉重来
        Err(ureq::Error::Status(416, _)) => {
            let _ = std::fs::remove_file(part);
            return Err(Fail::Net("the server refused to resume; starting over".into()));
        }
        Err(ureq::Error::Status(code, _)) if code >= 500 || code == 429 || code == 408 => return Err(Fail::Net(format!("the server answered HTTP {code}"))),
        Err(ureq::Error::Status(code, _)) => return Err(Fail::Fatal(format!("net:the server answered HTTP {code}"))),
        Err(e) => return Err(Fail::Net(e.to_string())),
    };
    let resumed = got > 0 && resp.status() == 206;
    if !resumed {
        got = 0;
    }
    let file = std::fs::OpenOptions::new().create(true).write(true).append(resumed).truncate(!resumed).open(part).map_err(|e| Fail::Fatal(crate::errors::io(&e, part, size - got)))?;
    let mut out = std::io::BufWriter::new(file);
    let mut reader = resp.into_reader();
    let mut buf = vec![0u8; 256 * 1024];
    let mut last = std::time::Instant::now();
    progress(got);
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = out.flush();
            return Err(Fail::Fatal(crate::errors::CANCELLED.into()));
        }
        let n = match reader.read(&mut buf) {
            Ok(n) => n,
            Err(e) => {
                let _ = out.flush();
                return Err(Fail::Net(format!("download interrupted: {e}")));
            }
        };
        if n == 0 {
            break;
        }
        if got + n as u64 > size {
            drop(out);
            let _ = std::fs::remove_file(part);
            return Err(Fail::Fatal("verify:the server sent more than the file's size".into()));
        }
        out.write_all(&buf[..n]).map_err(|e| Fail::Fatal(crate::errors::io(&e, part, size - got)))?;
        got += n as u64;
        if last.elapsed().as_millis() > 120 {
            last = std::time::Instant::now();
            progress(got);
        }
    }
    out.flush().map_err(|e| Fail::Fatal(crate::errors::io(&e, part, size - got)))?;
    if got < size {
        // 连接提前断了：留着收到的，重试时接着下
        return Err(Fail::Net(format!("the connection closed after {got} of {size} bytes")));
    }
    Ok(())
}

pub fn remove(dir: &Path, id: &str) -> Result<(), String> {
    let p = pack(id).ok_or("unknown pack")?;
    for f in p.files {
        let _ = std::fs::remove_file(dir.join(f));
        let _ = std::fs::remove_file(part_of(dir, f));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_sha256(s: &str) -> bool {
        s.len() == 64 && s.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    }

    /// The built-in list must cover every file of every pack, and nothing else; the files that ship inside the app must match it exactly.
    #[test]
    fn manifest_agrees_with_the_packs_and_the_bundled_files() {
        let m = manifest();
        for p in PACKS.iter() {
            for f in p.files {
                let info = m.get(*f).unwrap_or_else(|| panic!("{f} (pack {}) is not in models.manifest.json", p.id));
                assert!(is_sha256(&info.sha256), "{f}: sha256 must be 64 lowercase hex characters");
                match modelfiles::bundled(f) {
                    Some(bytes) => {
                        assert!(info.url.is_none(), "{f} ships inside the app, so it has no download address");
                        assert_eq!(info.size, bytes.len() as u64, "{f}: size");
                        assert_eq!(info.sha256, modelfiles::sha256_hex(bytes), "{f}: the copy in model-data/ differs from the manifest");
                    }
                    None => {
                        let url = info.url.as_deref().unwrap_or_else(|| panic!("{f} is neither bundled nor has a download address"));
                        assert!(url.starts_with("https://"), "{f}: {url}");
                        assert!(info.size > 0, "{f}: size");
                    }
                }
            }
        }
        for name in m.keys() {
            assert!(PACKS.iter().any(|p| p.files.contains(&name.as_str())), "{name} is in the manifest but in no pack");
        }
    }

    #[test]
    fn sizes_count_only_what_has_to_be_downloaded() {
        let nowhere = Path::new("/nonexistent-model-dir");
        // the search pack: the text model and its tokenizer are downloaded, the projection (dense.bin) ships inside the app
        let m = manifest();
        let want = m["text_multilingual_qint8.onnx"].size + m["text_multilingual_tokenizer.json"].size;
        assert_eq!(manifest_sizes(nowhere, "search").unwrap(), want);
        assert!(manifest_sizes(nowhere, "nope").is_err());
        // nothing left to download once the files are there
        let dir = std::env::temp_dir().join(format!("fg-packs-size-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in pack("search").unwrap().files {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        assert_eq!(manifest_sizes(&dir, "search").unwrap(), 0);
        assert!(installed(&dir, pack("search").unwrap()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// standard 包这台设备只下 B/32 的一份；另一份已经在（老安装、或别的平台拷来的）也算装好；`.part` 里已有的不再算进要下的。
    #[test]
    fn the_standard_pack_wants_one_b32_file_per_device() {
        let m = manifest();
        let nowhere = Path::new("/nonexistent-model-dir");
        let here = crate::scorer::b32_file_for_device();
        let head = "aesthetic_head_vit_b_32.json";
        let want = m[here].size + if modelfiles::has_model(nowhere, head) { 0 } else { m[head].size };
        assert_eq!(manifest_sizes(nowhere, "standard").unwrap(), want);
        let other = if here == crate::scorer::INT8_FILE { crate::scorer::B32_FP16_FILE } else { crate::scorer::INT8_FILE };
        let dir = std::env::temp_dir().join(format!("fg-packs-one-of-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{here}.part")), vec![0u8; 1000]).unwrap();
        assert_eq!(manifest_sizes(&dir, "standard").unwrap(), want - 1000, "已经下了的 1000 字节不再算");
        std::fs::write(dir.join(other), b"x").unwrap();
        std::fs::write(dir.join(head), b"x").unwrap();
        assert!(installed(&dir, pack("standard").unwrap()), "另一份在也能用");
        assert_eq!(manifest_sizes(&dir, "standard").unwrap(), 0);
        remove(&dir, "standard").unwrap();
        assert!(std::fs::read_dir(&dir).unwrap().next().is_none(), "删包连 .part 一起删");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 本地的小 HTTP 服务器，只回一个文件：认 `Range`；`cuts[i]` 是第 i 次连接只发多少字节就断（没有就发完）。
    /// 返回地址和每次请求带的 Range 头。
    fn serve(body: Vec<u8>, cuts: Vec<usize>) -> (String, std::sync::Arc<std::sync::Mutex<Vec<Option<String>>>>) {
        use std::io::BufRead;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/model.onnx", listener.local_addr().unwrap());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for (i, conn) in listener.incoming().enumerate() {
                let Ok(mut conn) = conn else { continue };
                let mut reader = std::io::BufReader::new(conn.try_clone().unwrap());
                let mut range = None;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("range:") {
                        range = Some(v.trim().to_string());
                    }
                }
                log.lock().unwrap().push(range.clone());
                let from: usize = range.as_deref().and_then(|r| r.strip_prefix("bytes=")).and_then(|r| r.trim_end_matches('-').parse().ok()).unwrap_or(0);
                let rest = &body[from..];
                let head = if from > 0 {
                    format!("HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {from}-{}/{}\r\nConnection: close\r\n\r\n", rest.len(), body.len() - 1, body.len())
                } else {
                    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", rest.len())
                };
                let n = cuts.get(i).copied().unwrap_or(rest.len()).min(rest.len());
                let _ = conn.write_all(head.as_bytes());
                let _ = conn.write_all(&rest[..n]);
                let _ = conn.flush();
            }
        });
        (url, seen)
    }

    fn body() -> (Vec<u8>, String) {
        let body: Vec<u8> = (0..700_000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
        let sha = modelfiles::sha256_hex(&body);
        (body, sha)
    }

    #[test]
    fn a_dropped_connection_resumes_where_it_stopped() {
        let (body, sha) = body();
        let (url, seen) = serve(body.clone(), vec![300_000]);
        let dir = std::env::temp_dir().join(format!("fg-packs-resume-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("model.onnx.part");
        let cancel = AtomicBool::new(false);
        fetch(&url, &part, body.len() as u64, &sha, &cancel, &|_| {}).unwrap();
        assert_eq!(std::fs::read(&part).unwrap(), body);
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 2, "断了一次，重试一次：{seen:?}");
        assert_eq!(seen[0], None);
        let from: usize = seen[1].as_deref().unwrap().strip_prefix("bytes=").unwrap().trim_end_matches('-').parse().unwrap();
        assert!(from > 0 && from <= 300_000, "从断的地方接着要：{from}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_part_left_from_last_time_is_continued_and_a_bad_file_is_thrown_away() {
        let (body, sha) = body();
        let (url, seen) = serve(body.clone(), vec![]);
        let dir = std::env::temp_dir().join(format!("fg-packs-part-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("model.onnx.part");
        std::fs::write(&part, &body[..123_456]).unwrap();
        let cancel = AtomicBool::new(false);
        fetch(&url, &part, body.len() as u64, &sha, &cancel, &|_| {}).unwrap();
        assert_eq!(std::fs::read(&part).unwrap(), body);
        assert_eq!(seen.lock().unwrap()[0].as_deref(), Some("bytes=123456-"));
        // 校验不对：删掉 .part，说 verify:
        std::fs::write(&part, &body[..1000]).unwrap();
        let e = fetch(&url, &part, body.len() as u64, &"0".repeat(64), &cancel, &|_| {}).unwrap_err();
        assert!(e.starts_with("verify:"), "{e}");
        assert!(!part.exists());
        // 取消：留着 .part
        std::fs::write(&part, &body[..1000]).unwrap();
        cancel.store(true, Ordering::Relaxed);
        assert_eq!(fetch(&url, &part, body.len() as u64, &sha, &cancel, &|_| {}).unwrap_err(), "cancelled");
        assert!(part.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_server_that_cannot_be_reached_is_retried_then_called_a_network_problem() {
        // 先占一个端口再放掉：这个地址上没人听
        let addr = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
        let dir = std::env::temp_dir().join(format!("fg-packs-net-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let t = std::time::Instant::now();
        let e = fetch(&format!("http://{addr}/x.onnx"), &dir.join("x.part"), 10, &"0".repeat(64), &AtomicBool::new(false), &|_| {}).unwrap_err();
        assert!(e.starts_with("net:"), "{e}");
        assert!(t.elapsed() >= BACKOFF * 7, "退避了三次（1 + 2 + 4 个单位）");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// End to end against the real address (38 MB): download, checksum, load the model, nothing to do the second time, cancel is not installed.
    #[test]
    #[ignore = "downloads 38 MB from huggingface.co"]
    fn downloads_verifies_and_loads() {
        let dir = std::env::temp_dir().join(format!("fg-packs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cancel = AtomicBool::new(false);
        let seen = std::sync::Mutex::new(0u64);
        let size = manifest_sizes(&dir, "faces").unwrap();
        println!("faces pack: {size} bytes");
        download(&dir, "faces", &cancel, &|p| { *seen.lock().unwrap() = p.done; }).unwrap();
        assert!(installed(&dir, pack("faces").unwrap()));
        assert_eq!(*seen.lock().unwrap(), size);
        assert!(std::fs::read_dir(&dir).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".part")));
        // nothing to do the second time
        download(&dir, "faces", &cancel, &|_| panic!("should not download again")).unwrap();
        // the downloaded model works
        crate::faces::FaceEmbedder::load(&dir).unwrap();
        // a cancelled download is not installed; what arrived stays as a .part for next time, and removing the pack clears it
        remove(&dir, "faces").unwrap();
        cancel.store(true, Ordering::Relaxed);
        assert_eq!(download(&dir, "faces", &cancel, &|_| {}).unwrap_err(), "cancelled");
        assert!(!installed(&dir, pack("faces").unwrap()));
        remove(&dir, "faces").unwrap();
        assert!(std::fs::read_dir(&dir).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".part")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
