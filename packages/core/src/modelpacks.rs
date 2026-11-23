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
    /// 当前速率（字节/秒）；连接中、重连中或校验中为 0。
    #[serde(default)]
    pub speed: u64,
    /// 当前阶段："connecting" | "downloading" | "retrying" | "verifying"。
    #[serde(default)]
    pub stage: String,
    /// 第几次重连（0 表示首次）。
    #[serde(default)]
    pub retry: u32,
}

/// 连续没有进展的网络出错后最多再试几次，每次等得更久（1、2、4 秒）；只要本轮多下了 `PROGRESS_RESET_BYTES` 以上，连续失败计数清零。
const RETRIES: u32 = 3;
/// 单文件总共最多重连次数（防止极端网络每次只吐几十 KB 无限循环）。
const MAX_RECONNECTS: u32 = 60;
/// 本轮至少多下这么多字节，才算有实质进展并重置连续失败计数。
const PROGRESS_RESET_BYTES: u64 = 64 * 1024;
#[cfg(not(test))]
const BACKOFF: std::time::Duration = std::time::Duration::from_secs(1);
#[cfg(test)]
const BACKOFF: std::time::Duration = std::time::Duration::from_millis(20);
#[cfg(not(test))]
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
#[cfg(test)]
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
#[cfg(not(test))]
const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(12);
#[cfg(test)]
const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
/// 连续这么久平均速率低于 `MIN_SPEED_BPS` 就判定链路卡死，主动断开重连（有镜像时切源）。
#[cfg(not(test))]
const STALL_WINDOW: std::time::Duration = std::time::Duration::from_secs(12);
#[cfg(test)]
const STALL_WINDOW: std::time::Duration = std::time::Duration::from_millis(250);
const MIN_SPEED_BPS: u64 = 12 * 1024;
/// 下载前要的空间是还要下的字节数乘上它：留点余地给系统和别的写入。
const SPACE_MARGIN: f64 = 1.1;

/// 同一次运行里如果镜像源已经跑通，后续文件直接优先走镜像，不必每个文件都在主站再等一次超时。
static PREFER_MIRROR: AtomicBool = AtomicBool::new(false);

const HF_PREFIX: &str = "https://huggingface.co/";
const HF_MIRROR_PREFIX: &str = "https://hf-mirror.com/";

/// 为下载地址生成候选源：Hugging Face 地址带国内镜像 hf-mirror.com 作自动回退；所有文件下完都做 SHA-256 校验。
fn candidate_urls(url: &str) -> Vec<String> {
    if let Some(rest) = url.strip_prefix(HF_PREFIX) {
        let primary = url.to_string();
        let mirror = format!("{HF_MIRROR_PREFIX}{rest}");
        if PREFER_MIRROR.load(Ordering::Relaxed) {
            return vec![mirror, primary];
        }
        return vec![primary, mirror];
    }
    vec![url.to_string()]
}

/// 一次请求的失败：网络的可以重试，别的（磁盘满、取消、校验失败）不重试。
enum Fail {
    /// `stalled` = 链路龟速卡死被主动掐断（有备用源时优先切源）。
    Net { msg: String, stalled: bool },
    Fatal(String),
}

impl Fail {
    fn net(msg: impl Into<String>) -> Self {
        Self::Net { msg: msg.into(), stalled: false }
    }
    fn stalled(msg: impl Into<String>) -> Self {
        Self::Net { msg: msg.into(), stalled: true }
    }
}

struct FileProg {
    got: u64,
    speed: u64,
    stage: &'static str,
    retry: u32,
}

/// 下了一半的文件：`名字.<SHA-256 前 12 位>.part`，取消、断网都留着，下次从它的末尾接着下。名字里带着应有的校验值：
/// 清单换了版本（同名文件换了内容）时旧的 `.part` 对不上名字，不会被拿来盲目续传。
fn part_of(dir: &Path, name: &str, sha: &str) -> PathBuf {
    let tag: String = sha.chars().take(12).collect::<String>().to_lowercase();
    dir.join(format!("{name}.{tag}.part"))
}

/// 这个文件别的版本留下的 `.part`（以前不带校验值的 `名字.part`，或校验值不同的）：不能续传，删掉。
fn drop_stale_parts(dir: &Path, name: &str, keep: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.filter_map(Result::ok) {
        let n = e.file_name().to_string_lossy().into_owned();
        if n.starts_with(name) && n.ends_with(".part") && e.path() != keep {
            let mid = &n[name.len()..n.len() - ".part".len()];
            // `名字.part` 或 `名字.<十六进制>.part`：别的文件名只是开头一样的（`a.onnx` 与 `a.onnx_data`）不碰
            if mid.is_empty() || (mid.starts_with('.') && mid[1..].chars().all(|c| c.is_ascii_hexdigit())) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
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
    Ok(missing(dir, p)?.iter().map(|(name, i, _)| i.size - have(&part_of(dir, name, &i.sha256), i.size)).sum())
}

/// Downloads what a pack is missing into `dir`. 先看空间（还要下的 × 1.1），不够就 `space:` 错误、一个字节都不下；
/// 每个文件断点续传、卡慢自动重连（Hugging Face 自动回退国内镜像）、有进展重置重试计数，校验通过才改成正式名字。
/// `Progress.done` / `Progress.total` 统一按“本次还要下的字节数”算，和 `pack_size()` 对齐，避免续传时进度条瞬间跳满。
/// `cancel` stops it between chunks, keeping what arrived. 错误都带 `errors` 约定的前缀。
pub fn download(dir: &Path, id: &str, cancel: &AtomicBool, on_progress: &dyn Fn(Progress)) -> Result<(), String> {
    let p = pack(id).ok_or("unknown pack")?;
    std::fs::create_dir_all(dir).map_err(|e| crate::errors::io(&e, dir, 0))?;
    let todo = missing(dir, p)?;
    let mut plan = Vec::with_capacity(todo.len());
    let mut left_total = 0u64;
    for (name, info, url) in todo {
        let part = part_of(dir, name, &info.sha256);
        drop_stale_parts(dir, name, &part);
        let initial = have(&part, info.size);
        let remaining = info.size - initial;
        left_total += remaining;
        plan.push((name, info, url, part, initial, remaining));
    }
    crate::errors::check_space(dir, (left_total as f64 * SPACE_MARGIN).ceil() as u64)?;
    let mut done_before = 0u64;
    for (name, info, url, part, initial, remaining) in plan {
        let urls = candidate_urls(url);
        fetch_urls(&urls, &part, info.size, &info.sha256, cancel, &|fp| {
            let file_done = session_file_done(fp.got, initial, info.size, remaining);
            on_progress(Progress {
                pack: id.into(),
                file: name.into(),
                done: done_before + file_done,
                total: left_total,
                speed: fp.speed,
                stage: fp.stage.into(),
                retry: fp.retry,
            });
        })
        .map_err(|e| crate::errors::context(name, &e))?;
        std::fs::rename(&part, dir.join(name)).map_err(|e| crate::errors::io(&e, dir, 0))?;
        done_before += remaining;
        on_progress(Progress {
            pack: id.into(),
            file: name.into(),
            done: done_before,
            total: left_total,
            speed: 0,
            stage: "downloading".into(),
            retry: 0,
        });
    }
    Ok(())
}

/// 单个文件在本次下载会话里的进度（`0..=remaining`）：正常续传是 `got - initial`；
/// 如果服务器拒绝续传回 200 从头重下（`got < initial`），按比例映射到 `0..remaining`，既不倒退也不提前满格。
fn session_file_done(got: u64, initial: u64, size: u64, remaining: u64) -> u64 {
    if got >= initial {
        (got - initial).min(remaining)
    } else if size > 0 {
        ((got as u128 * remaining as u128) / size as u128) as u64
    } else {
        0
    }
}

/// 测试用的便捷入口：单地址下载并按字节回报进度。
#[cfg(test)]
fn fetch(url: &str, part: &Path, size: u64, sha: &str, cancel: &AtomicBool, progress: &dyn Fn(u64)) -> Result<(), String> {
    fetch_urls(&candidate_urls(url), part, size, sha, cancel, &|fp| progress(fp.got))
}

/// 按候选地址列表把文件下到 `part`（`size` 字节、SHA-256 是 `sha`）：
/// - 接着 `part` 里已有的下（`Range`）
/// - 连续无进展失败最多退避重试 `RETRIES` 次（1、2、4 秒）；只要本轮多下了 ≥ 64 KB，连续失败计数清零（最多总重连 `MAX_RECONNECTS` 次）
/// - 连不上或龟速卡死时自动切下一个候选源（Hugging Face ↔ hf-mirror），成功后记住镜像偏好
/// - 下完发 `verifying` 阶段并校验 SHA-256；校验不对才删 `part`（`verify:`），别的失败都留着它，下次接着下
fn fetch_urls(urls: &[String], part: &Path, size: u64, sha: &str, cancel: &AtomicBool, progress: &dyn Fn(FileProg)) -> Result<(), String> {
    let mut consecutive_fails = 0u32;
    let mut total_reconnects = 0u32;
    let mut idx = 0usize;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(crate::errors::CANCELLED.into());
        }
        let before = have(part, size);
        if total_reconnects == 0 {
            progress(FileProg { got: before, speed: 0, stage: "connecting", retry: 0 });
        }
        let current_url = &urls[idx];
        match fetch_once(current_url, part, size, cancel, progress) {
            Ok(()) => {
                if urls.len() > 1 {
                    PREFER_MIRROR.store(current_url.starts_with(HF_MIRROR_PREFIX), Ordering::Relaxed);
                }
                break;
            }
            Err(Fail::Net { msg, stalled }) => {
                if cancel.load(Ordering::Relaxed) {
                    return Err(crate::errors::CANCELLED.into());
                }
                let after = have(part, size);
                let made_progress = after >= before.saturating_add(PROGRESS_RESET_BYTES);
                if made_progress {
                    consecutive_fails = 0;
                }
                if (!made_progress && consecutive_fails >= RETRIES) || total_reconnects >= MAX_RECONNECTS {
                    return Err(format!("net:{msg}"));
                }
                let wait = if made_progress { BACKOFF } else { BACKOFF * 2u32.pow(consecutive_fails) };
                if !made_progress {
                    consecutive_fails += 1;
                }
                total_reconnects += 1;
                // 连不上、无进展，或因龟速卡死被掐断：有备用源就立刻切源
                if urls.len() > 1 && (!made_progress || stalled) {
                    idx = (idx + 1) % urls.len();
                }
                crate::platform::log(&format!(
                    "模型下载出错，{} 秒后第 {total_reconnects} 次重试（目标 {}）：{msg}",
                    wait.as_secs_f32(),
                    urls[idx].split('/').nth(2).unwrap_or(&urls[idx])
                ));
                progress(FileProg { got: after, speed: 0, stage: "retrying", retry: total_reconnects });
                let until = std::time::Instant::now() + wait;
                while std::time::Instant::now() < until {
                    if cancel.load(Ordering::Relaxed) {
                        return Err(crate::errors::CANCELLED.into());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50).min(wait));
                }
            }
            Err(Fail::Fatal(e)) => return Err(e),
        }
    }
    progress(FileProg { got: size, speed: 0, stage: "verifying", retry: 0 });
    let got = modelfiles::sha256_file(part).map_err(|e| format!("verify:{e}"))?;
    if got != sha.to_lowercase() {
        let _ = std::fs::remove_file(part);
        return Err("verify:the file did not match its checksum".into());
    }
    Ok(())
}

/// 一次请求：从 `part` 的末尾接着要，收到的追加进去，直到 `size` 字节。服务器不认 `Range`（回 200）就从头写。
/// 带低速卡死检测（连续 `STALL_WINDOW` 平均速率低于 `MIN_SPEED_BPS` 就主动断开重连）和瞬时速率回报。
fn fetch_once(url: &str, part: &Path, size: u64, cancel: &AtomicBool, progress: &dyn Fn(FileProg)) -> Result<(), Fail> {
    if cancel.load(Ordering::Relaxed) {
        return Err(Fail::Fatal(crate::errors::CANCELLED.into()));
    }
    let mut got = have(part, size);
    if got == size && size > 0 {
        return Ok(());
    }
    let agent = crate::platform::http_agent(url, CONNECT_TIMEOUT, READ_TIMEOUT);
    let mut req = agent.get(url);
    if got > 0 {
        req = req.set("Range", &format!("bytes={got}-"));
    }
    let resp = match req.call() {
        Ok(r) => r,
        // 要的范围不对（服务器上的文件变了，或者 `.part` 坏了）：扔掉重来
        Err(ureq::Error::Status(416, _)) => {
            let _ = std::fs::remove_file(part);
            return Err(Fail::net("the server refused to resume; starting over"));
        }
        Err(ureq::Error::Status(code, _)) if code >= 500 || code == 429 || code == 408 => return Err(Fail::net(format!("the server answered HTTP {code}"))),
        Err(ureq::Error::Status(code, _)) => return Err(Fail::Fatal(format!("net:the server answered HTTP {code}"))),
        Err(e) => {
            if cancel.load(Ordering::Relaxed) {
                return Err(Fail::Fatal(crate::errors::CANCELLED.into()));
            }
            return Err(Fail::net(e.to_string()));
        }
    };
    if cancel.load(Ordering::Relaxed) {
        return Err(Fail::Fatal(crate::errors::CANCELLED.into()));
    }
    let resumed = got > 0 && resp.status() == 206;
    if resumed {
        // 服务器说它从哪里接着发（`Content-Range: bytes 起点-终点/总长`）：起点不是我们要的、或总长不是这个文件的，
        // 接上去就是一个坏文件，要等下完校验才发现。扔掉 `.part` 从头来
        let range = resp.header("Content-Range").and_then(parse_content_range);
        if !matches!(range, Some((start, total)) if start == got && total.map_or(true, |t| t == size)) {
            let _ = std::fs::remove_file(part);
            return Err(Fail::net(format!("the server resumed at the wrong place ({:?}); starting over", resp.header("Content-Range"))));
        }
    } else {
        got = 0;
    }
    let file = std::fs::OpenOptions::new().create(true).write(true).append(resumed).truncate(!resumed).open(part).map_err(|e| Fail::Fatal(crate::errors::io(&e, part, size - got)))?;
    let mut out = std::io::BufWriter::new(file);
    let mut reader = resp.into_reader();
    let mut buf = vec![0u8; 64 * 1024];
    let now = std::time::Instant::now();
    let mut last_emit = now;
    let mut speed_start = now;
    let mut speed_bytes = 0u64;
    let mut speed_bps = 0u64;
    let mut stall_start = now;
    let mut stall_bytes = 0u64;
    progress(FileProg { got, speed: 0, stage: "downloading", retry: 0 });
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = out.flush();
            return Err(Fail::Fatal(crate::errors::CANCELLED.into()));
        }
        let n = match reader.read(&mut buf) {
            Ok(n) => n,
            Err(e) => {
                let _ = out.flush();
                if cancel.load(Ordering::Relaxed) {
                    return Err(Fail::Fatal(crate::errors::CANCELLED.into()));
                }
                return Err(Fail::net(format!("download interrupted: {e}")));
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
        speed_bytes += n as u64;
        stall_bytes += n as u64;

        let speed_dt = speed_start.elapsed();
        if speed_dt.as_millis() >= 250 {
            let inst = (speed_bytes as f64 / speed_dt.as_secs_f64()) as u64;
            speed_bps = if speed_bps == 0 { inst } else { (speed_bps * 3 + inst * 2) / 5 };
            if speed_dt >= std::time::Duration::from_secs(1) {
                speed_start = std::time::Instant::now();
                speed_bytes = 0;
            }
        }

        if last_emit.elapsed().as_millis() > 120 {
            last_emit = std::time::Instant::now();
            progress(FileProg { got, speed: speed_bps, stage: "downloading", retry: 0 });
        }

        let stall_dt = stall_start.elapsed();
        if stall_dt >= STALL_WINDOW {
            let avg = (stall_bytes as f64 / stall_dt.as_secs_f64()) as u64;
            if avg < MIN_SPEED_BPS {
                let _ = out.flush();
                return Err(Fail::stalled(format!("download stalled ({avg} B/s)")));
            }
            stall_start = std::time::Instant::now();
            stall_bytes = 0;
        }
    }
    out.flush().map_err(|e| Fail::Fatal(crate::errors::io(&e, part, size - got)))?;
    if got < size {
        // 连接提前断了：留着收到的，重试时接着下
        return Err(Fail::net(format!("the connection closed after {got} of {size} bytes")));
    }
    Ok(())
}

/// `bytes 起点-终点/总长`（总长可以是 `*`）→ (起点, 总长)。
fn parse_content_range(v: &str) -> Option<(u64, Option<u64>)> {
    let rest = v.trim().strip_prefix("bytes")?.trim_start();
    let (span, total) = rest.split_once('/')?;
    let (start, _) = span.split_once('-')?;
    let total = match total.trim() {
        "*" => None,
        t => Some(t.parse().ok()?),
    };
    Some((start.trim().parse().ok()?, total))
}

pub fn remove(dir: &Path, id: &str) -> Result<(), String> {
    let p = pack(id).ok_or("unknown pack")?;
    for f in p.files {
        let _ = std::fs::remove_file(dir.join(f));
        // 各个版本的 `.part` 都删
        drop_stale_parts(dir, f, Path::new(""));
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
        std::fs::write(part_of(&dir, here, &m[here].sha256), vec![0u8; 1000]).unwrap();
        assert_eq!(manifest_sizes(&dir, "standard").unwrap(), want - 1000, "已经下了的 1000 字节不再算");
        // 不带校验值的老 `.part`（不知道是哪个版本的）不算
        std::fs::write(dir.join(format!("{here}.part")), vec![0u8; 500]).unwrap();
        assert_eq!(manifest_sizes(&dir, "standard").unwrap(), want - 1000);
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
        serve_shifted(body, cuts, 0)
    }

    /// 同 `serve`，但第一次续传时从要的地方往前 `shift` 字节发（并如实写在 Content-Range 里）：一个不按要求续传的服务器。
    fn serve_shifted(body: Vec<u8>, cuts: Vec<usize>, shift: usize) -> (String, std::sync::Arc<std::sync::Mutex<Vec<Option<String>>>>) {
        use std::io::BufRead;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/model.onnx", listener.local_addr().unwrap());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            let mut shift = shift;
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
                let mut from: usize = range.as_deref().and_then(|r| r.strip_prefix("bytes=")).and_then(|r| r.trim_end_matches('-').parse().ok()).unwrap_or(0);
                if from > 0 {
                    from = from.saturating_sub(std::mem::take(&mut shift)).max(1);
                }
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

    /// 服务器续传的起点不是要的那里：不接上去（那样下完才校验失败），扔掉 `.part` 从头下。
    #[test]
    fn a_resume_from_the_wrong_place_starts_over() {
        let (body, sha) = body();
        let (url, seen) = serve_shifted(body.clone(), vec![], 1000);
        let dir = std::env::temp_dir().join(format!("fg-packs-shift-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("model.onnx.part");
        std::fs::write(&part, &body[..50_000]).unwrap();
        fetch(&url, &part, body.len() as u64, &sha, &AtomicBool::new(false), &|_| {}).unwrap();
        assert_eq!(std::fs::read(&part).unwrap(), body);
        assert_eq!(seen.lock().unwrap().clone(), vec![Some("bytes=50000-".to_string()), None], "第二次从头要");
        assert_eq!(parse_content_range("bytes 100-199/1000"), Some((100, Some(1000))));
        assert_eq!(parse_content_range("bytes 5-9/*"), Some((5, None)));
        assert_eq!(parse_content_range("items 1-2/3"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `.part` 的名字带着应有的校验值：别的版本留下的（老的不带校验值的、校验值不同的）下载前删掉，删包时各版本一起删。
    #[test]
    fn parts_of_another_version_are_not_resumed() {
        let dir = std::env::temp_dir().join(format!("fg-packs-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mine = part_of(&dir, "a.onnx", &"AB".repeat(32));
        assert_eq!(mine.file_name().unwrap().to_string_lossy(), "a.onnx.abababababab.part");
        for n in ["a.onnx.part", "a.onnx.0123456789ab.part", "a.onnx_data.part", "b.onnx.part"] {
            std::fs::write(dir.join(n), b"x").unwrap();
        }
        std::fs::write(&mine, b"x").unwrap();
        drop_stale_parts(&dir, "a.onnx", &mine);
        let mut left: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, vec!["a.onnx.abababababab.part", "a.onnx_data.part", "b.onnx.part"]);
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

    /// 每次断线前都有实质进展（≥ 64 KB）：连续失败计数会清零，断 5 次也能靠断点续传下完。
    #[test]
    fn progress_resets_retry_count_across_many_drops() {
        let (body, sha) = body();
        let (url, seen) = serve(body.clone(), vec![120_000, 120_000, 120_000, 120_000, 120_000]);
        let dir = std::env::temp_dir().join(format!("fg-packs-many-drops-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("model.onnx.part");
        let stages = std::sync::Mutex::new(Vec::<String>::new());
        fetch_urls(&[url], &part, body.len() as u64, &sha, &AtomicBool::new(false), &|fp| {
            let mut s = stages.lock().unwrap();
            if s.last().map(String::as_str) != Some(fp.stage) {
                s.push(fp.stage.to_string());
            }
        })
        .unwrap();
        assert_eq!(std::fs::read(&part).unwrap(), body);
        assert_eq!(seen.lock().unwrap().len(), 6, "5 次中途断线 + 1 次收尾");
        let stages = stages.lock().unwrap();
        assert!(stages.contains(&"connecting".to_string()), "{stages:?}");
        assert!(stages.contains(&"retrying".to_string()), "{stages:?}");
        assert_eq!(stages.last().map(String::as_str), Some("verifying"), "{stages:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 主源龟速卡死（< 12 KB/s）：主动断开并切到备用源续传下完。
    #[test]
    fn stalled_source_switches_to_mirror_and_resumes() {
        use std::io::BufRead;
        let (body, sha) = body();
        // 主源：每次只吐 512 字节、间隔 80 ms（约 6.4 KB/s < 12 KB/s），触发卡死检测
        let slow_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let slow_url = format!("http://{}/model.onnx", slow_listener.local_addr().unwrap());
        let slow_body = body.clone();
        std::thread::spawn(move || {
            for conn in slow_listener.incoming().take(2) {
                let Ok(mut conn) = conn else { continue };
                let mut reader = std::io::BufReader::new(conn.try_clone().unwrap());
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                }
                let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", slow_body.len());
                if conn.write_all(head.as_bytes()).is_err() {
                    continue;
                }
                for chunk in slow_body.chunks(512) {
                    if conn.write_all(chunk).is_err() || conn.flush().is_err() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(80));
                }
            }
        });
        let (fast_url, fast_seen) = serve(body.clone(), vec![]);
        let dir = std::env::temp_dir().join(format!("fg-packs-stall-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("model.onnx.part");
        fetch_urls(&[slow_url, fast_url], &part, body.len() as u64, &sha, &AtomicBool::new(false), &|_| {}).unwrap();
        assert_eq!(std::fs::read(&part).unwrap(), body);
        let fast_reqs = fast_seen.lock().unwrap().clone();
        assert_eq!(fast_reqs.len(), 1, "卡死后切到备用源一次下完：{fast_reqs:?}");
        assert!(fast_reqs[0].as_deref().is_some_and(|r| r.starts_with("bytes=")), "带着主源已落盘的字节续传：{fast_reqs:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hf_candidate_urls_and_session_progress() {
        PREFER_MIRROR.store(false, Ordering::Relaxed);
        assert_eq!(
            candidate_urls("https://huggingface.co/Xenova/clip/resolve/abc/model.onnx"),
            vec![
                "https://huggingface.co/Xenova/clip/resolve/abc/model.onnx",
                "https://hf-mirror.com/Xenova/clip/resolve/abc/model.onnx",
            ]
        );
        assert_eq!(candidate_urls("https://github.com/aiwren/FleetingGleam/releases/download/v1/x.onnx"), vec!["https://github.com/aiwren/FleetingGleam/releases/download/v1/x.onnx"]);
        // 续传进度：已有 500，总长 600，本次剩 100
        assert_eq!(session_file_done(500, 500, 600, 100), 0);
        assert_eq!(session_file_done(540, 500, 600, 100), 40);
        assert_eq!(session_file_done(600, 500, 600, 100), 100);
        // 服务器回 200 从头重下时按比例映射到 0..100，不倒退也不提前满格
        assert_eq!(session_file_done(300, 500, 600, 100), 50);
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
