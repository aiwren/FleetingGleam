//! Persistent per-photo analysis cache. Re-opening a project, adding a folder or re-scoring never
//! re-analyses a file whose size and modification time are unchanged and whose thumbnail still exists.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::analyze::Photo;

/// The float vectors of an entry (CLIP, search, faces) are stored as text of raw bytes (base64) instead of a JSON array of numbers: a third of
/// the size and several times faster to write and read, which matters at tens of thousands of photos. Files written the old way still load.
pub(crate) mod packed {
    use serde::de::{self, Deserializer, SeqAccess, Visitor};
    use serde::ser::{SerializeSeq, Serializer};
    use std::fmt;

    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    fn encode(v: &[f32]) -> String {
        let mut bytes = Vec::with_capacity(v.len() * 4);
        for x in v {
            bytes.extend_from_slice(&x.to_le_bytes());
        }
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for c in bytes.chunks(3) {
            let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
            out.push(ALPHA[(n >> 18) as usize & 63] as char);
            out.push(ALPHA[(n >> 12) as usize & 63] as char);
            out.push(if c.len() > 1 { ALPHA[(n >> 6) as usize & 63] as char } else { '=' });
            out.push(if c.len() > 2 { ALPHA[n as usize & 63] as char } else { '=' });
        }
        out
    }

    fn decode(s: &str) -> Option<Vec<f32>> {
        let mut val = [255u8; 256];
        for (i, &a) in ALPHA.iter().enumerate() {
            val[a as usize] = i as u8;
        }
        let b = s.as_bytes();
        if b.len() % 4 != 0 {
            return None;
        }
        let mut bytes = Vec::with_capacity(b.len() / 4 * 3);
        for q in b.chunks(4) {
            let pad = q.iter().rev().take_while(|&&c| c == b'=').count();
            let mut n = 0u32;
            for (i, &c) in q.iter().enumerate() {
                let v = if c == b'=' { 0 } else { val[c as usize] };
                if v == 255 {
                    return None;
                }
                n |= (v as u32) << (18 - 6 * i as u32);
            }
            bytes.push((n >> 16) as u8);
            if pad < 2 { bytes.push((n >> 8) as u8); }
            if pad < 1 { bytes.push(n as u8); }
        }
        if bytes.len() % 4 != 0 {
            return None;
        }
        Some(bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
    }

    /// A vector: the packed text, or (old files) the plain array of numbers.
    struct One;
    impl<'de> Visitor<'de> for One {
        type Value = Vec<f32>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result { f.write_str("packed floats or a list of numbers") }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<Vec<f32>, E> { decode(v).ok_or_else(|| E::custom("bad packed vector")) }
        fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Vec<f32>, A::Error> {
            let mut out = Vec::with_capacity(a.size_hint().unwrap_or(0));
            while let Some(x) = a.next_element::<f32>()? { out.push(x); }
            Ok(out)
        }
    }
    struct Elem(Vec<f32>);
    impl<'de> serde::Deserialize<'de> for Elem {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> { d.deserialize_any(One).map(Elem) }
    }
    pub fn serialize<S: Serializer>(v: &Vec<f32>, s: S) -> Result<S::Ok, S::Error> { s.serialize_str(&encode(v)) }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<f32>, D::Error> { d.deserialize_any(One) }

    /// A list of vectors.
    pub mod many {
        use super::*;
        pub fn serialize<S: Serializer>(v: &Vec<Vec<f32>>, s: S) -> Result<S::Ok, S::Error> {
            let mut seq = s.serialize_seq(Some(v.len()))?;
            for x in v { seq.serialize_element(&encode(x))?; }
            seq.end()
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Vec<f32>>, D::Error> {
            struct Many;
            impl<'de> Visitor<'de> for Many {
                type Value = Vec<Vec<f32>>;
                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result { f.write_str("a list of vectors") }
                fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                    let mut out = Vec::with_capacity(a.size_hint().unwrap_or(0));
                    while let Some(Elem(x)) = a.next_element::<Elem>()? { out.push(x); }
                    Ok(out)
                }
            }
            d.deserialize_seq(Many)
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    /// Analysis version + model; entries made by a different one are ignored and redone.
    #[serde(default)]
    tag: String,
    size: u64,
    mtime: i64,
    photo: Photo,
    hash: u64,
    #[serde(with = "packed")]
    emb: Vec<f32>,
    /// CLIP ViT-B/32 embedding used for search (512 numbers). Filled by the search index for photos analysed with the larger model.
    #[serde(default, with = "packed")]
    sem: Vec<f32>,
    /// Colour signature (see colorsig.rs), made together with the search vector.
    #[serde(default)]
    color: Vec<u8>,
    /// Face-recognition vectors, one per entry of `photo.face_boxes`.
    #[serde(default, with = "packed::many")]
    fe: Vec<Vec<f32>>,
}

// ---------------------------------------------------------------- 落盘
//
// 以前每存一次就把整个缓存（万张照片几百 MB 的 JSON）重新序列化、写盘、fsync，而且是在引擎的 `cache` 锁里：分析中每分钟一次，
// 期间所有要查缓存的调用都排队；转一张照片也是整文件重写。现在分两层：
//
// - 底稿 `analysis-cache.json`：和以前同一个格式（`{"entries": {...}}`），多一个 `seq`（底稿包含到第几条日志），旧文件没有它就是 0；
// - 日志 `analysis-cache.log`：每改一条记录追加一行 `{"seq":N,"key":"...","e":{记录}}`（`e` 为 null 是删掉），每批 fsync 一次。
//
// 读的时候先读底稿，再按顺序重放日志里 `seq` 比底稿新的行（同一张照片以 `seq` 最大的为准）；最后一行写到一半（断电）就丢掉那一行。
// 日志长到和底稿差不多大时整理一次：锁里只做快照（记录是 `Arc`，快照只拷指针）并把日志改名成 `analysis-cache.log.old`，
// 锁外写新底稿、改名、再删旧日志；中途断电，下次读时旧底稿 + 旧日志 + 新日志照样拼得回来。
//
// 锁里只决定要写什么（`save` 返回一个 `Pending`），序列化与写盘都在锁外由调用方 `write()`。

/// 日志长过这么多、而且超过底稿的一半，下一次存就顺带整理。
const COMPACT_AT: u64 = 8 << 20;

/// 几个 `Pending` 共用的落盘状态：日志文件的大小（写坏一半时截回去）、是不是正在整理、上次整理有没有失败。
#[derive(Default)]
struct Disk {
    /// 当前日志文件的长度；追加、改名都在这把锁里做。
    log: std::sync::Mutex<u64>,
    /// 同一个长度的副本：`save`（在引擎的锁里）读它，不用等别人追加时的 fsync。
    log_len: AtomicU64,
    /// 一次只整理一个。
    compacting: AtomicBool,
    /// 上次整理没写成（它的快照里有还没进日志的改动）：下次存再整理。
    retry: AtomicBool,
    /// 底稿多大（决定什么时候整理）。
    base: AtomicU64,
}

#[derive(Default)]
pub struct Cache {
    entries: HashMap<String, Arc<Entry>>,
    path: PathBuf,
    /// 最后一条记录的序号（底稿与日志共用一个计数）。
    seq: u64,
    /// 改过、还没交给日志的照片。
    dirty: HashSet<String>,
    disk: Arc<Disk>,
}

pub fn file_key(path: &Path) -> Option<(u64, i64)> {
    let m = std::fs::metadata(path).ok()?;
    let mtime = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
    Some((m.len(), mtime))
}

/// 底稿读进来的样子；旧文件没有 `seq`。
#[derive(Deserialize)]
struct BaseIn {
    #[serde(default)]
    entries: HashMap<String, Entry>,
    #[serde(default)]
    seq: u64,
}

/// 日志的一行。
#[derive(Deserialize)]
struct LineIn {
    seq: u64,
    key: String,
    e: Option<Entry>,
}

#[derive(Serialize)]
struct LineOut<'a> {
    seq: u64,
    key: &'a str,
    e: Option<&'a Entry>,
}

/// 底稿写出去的样子：记录在 `Arc` 里，按引用写，不再拷一份。
struct BaseOut<'a> {
    entries: &'a HashMap<String, Arc<Entry>>,
    seq: u64,
}

impl Serialize for BaseOut<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        struct Entries<'a>(&'a HashMap<String, Arc<Entry>>);
        impl Serialize for Entries<'_> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_map(self.0.iter().map(|(k, v)| (k, &**v)))
            }
        }
        let mut st = s.serialize_struct("Cache", 2)?;
        st.serialize_field("entries", &Entries(self.entries))?;
        st.serialize_field("seq", &self.seq)?;
        st.end()
    }
}

fn log_path(base: &Path) -> PathBuf {
    base.with_extension("log")
}

fn old_log_path(base: &Path) -> PathBuf {
    base.with_extension("log.old")
}

/// 在锁里定好、锁外再写的一次落盘。什么都不用写时 `write` 什么也不做。
#[must_use = "缓存的改动要 `write()` 才落盘"]
pub struct Pending {
    path: PathBuf,
    disk: Arc<Disk>,
    job: Job,
}

enum Job {
    Nothing,
    /// 追加到日志的几行。
    Append(Vec<(u64, String, Option<Arc<Entry>>)>),
    /// 整个快照写成新底稿（日志已经改名成旧日志）。
    Compact(HashMap<String, Arc<Entry>>, u64),
}

impl Pending {
    pub fn write(self) {
        match self.job {
            Job::Nothing => {}
            Job::Append(lines) => append_lines(&self.path, &self.disk, &lines),
            Job::Compact(entries, seq) => {
                let ok = write_base(&self.path, &entries, seq);
                if ok {
                    let _ = std::fs::remove_file(old_log_path(&self.path));
                    self.disk.base.store(std::fs::metadata(&self.path).map_or(0, |m| m.len()), Ordering::Relaxed);
                } else {
                    // 快照里有没进日志的改动：旧日志留着（下次读还要它），下次存再整理一次
                    crate::platform::log(&format!("{}: 分析缓存整理没写成，下次再试", self.path.display()));
                    self.disk.retry.store(true, Ordering::Relaxed);
                }
                self.disk.compacting.store(false, Ordering::Relaxed);
            }
        }
    }
}

/// 几行日志序列化成要追加的字节。
fn encode_lines(lines: &[(u64, String, Option<Arc<Entry>>)]) -> Result<Vec<u8>, String> {
    let mut buf = Vec::with_capacity(lines.len() * 8 * 1024);
    for (seq, key, e) in lines {
        serde_json::to_writer(&mut buf, &LineOut { seq: *seq, key, e: e.as_deref() }).map_err(|e| format!("{key}: {e}"))?;
        buf.push(b'\n');
    }
    Ok(buf)
}

/// 在日志锁里（`len` 是锁着的日志长度）把 `buf` 追加到当前日志、fsync；写坏一半就截回原来的长度，不留半行。
fn append_locked(path: &Path, len: &mut u64, buf: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(log_path(path)).map_err(|e| e.to_string())?;
    let start = f.metadata().map_or(*len, |m| m.len());
    match std::io::Write::write_all(&mut f, buf).and_then(|_| f.sync_data()) {
        Ok(()) => {
            *len = start + buf.len() as u64;
            Ok(())
        }
        Err(e) => {
            let _ = f.set_len(start);
            *len = start;
            Err(e.to_string())
        }
    }
}

/// 一批日志行：先在锁外序列化好，再在日志锁里一次追加、fsync。
/// 没写成（序列化不了、打不开、写不进、fsync 失败）时这批行就丢了，但它们还在内存的记录里：置 `retry`，下一次存整理一次、
/// 写整份快照，把它们补进底稿（以前整文件保存会自愈，改成日志后这里不补就再也不写了）。
fn append_lines(path: &Path, disk: &Disk, lines: &[(u64, String, Option<Arc<Entry>>)]) {
    let r = encode_lines(lines).and_then(|buf| {
        let mut len = disk.log.lock().unwrap_or_else(|e| e.into_inner());
        let r = append_locked(path, &mut len, &buf);
        disk.log_len.store(*len, Ordering::Relaxed);
        r
    });
    if let Err(why) = r {
        crate::platform::log(&format!("{}: 分析缓存写不进去，下次存时整份重写：{why}", log_path(path).display()));
        disk.retry.store(true, Ordering::Relaxed);
    }
}

/// 新底稿：先写临时文件、flush、fsync，再改名；断电只会留下旧的或新的，不会是半个。
fn write_base(path: &Path, entries: &HashMap<String, Arc<Entry>>, seq: u64) -> bool {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("tmp");
    let Ok(f) = std::fs::File::create(&tmp) else { return false };
    // 边产出边写（几百 MB 的文件在内存里不再多拷一份）
    let mut w = std::io::BufWriter::with_capacity(1 << 20, f);
    serde_json::to_writer(&mut w, &BaseOut { entries, seq }).is_ok()
        && std::io::Write::flush(&mut w).is_ok()
        && w.get_ref().sync_all().is_ok()
        && std::fs::rename(&tmp, path).is_ok()
}

impl Cache {
    pub fn load(path: PathBuf) -> Self {
        let base: Option<BaseIn> = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let base_len = std::fs::metadata(&path).map_or(0, |m| m.len());
        let (entries, base_seq) = base.map(|b| (b.entries, b.seq)).unwrap_or_default();
        let mut c = Cache { entries: entries.into_iter().map(|(k, v)| (k, Arc::new(v))).collect(), seq: base_seq, ..Default::default() };
        // 日志：旧的（上次整理没做完）在前，当前的在后；同一张照片以序号最大的一行为准，底稿已经包含的跳过
        let mut at: HashMap<String, u64> = HashMap::new();
        let old = old_log_path(&path);
        let had_old = old.exists();
        let mut log_len = 0;
        for (file, current) in [(old, false), (log_path(&path), true)] {
            let Ok(bytes) = std::fs::read(&file) else { continue };
            // 最后一行写到一半（断电）：丢掉，并把文件截到最后一个完整行，免得下一次追加接在半行后面
            let whole = bytes.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
            if whole < bytes.len() {
                if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&file) {
                    let _ = f.set_len(whole as u64);
                }
            }
            if current {
                log_len = whole as u64;
            }
            for line in bytes[..whole].split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
                let Ok(r) = serde_json::from_slice::<LineIn>(line) else { continue };
                c.seq = c.seq.max(r.seq);
                if r.seq <= base_seq || at.get(&r.key).map_or(false, |&s| s >= r.seq) {
                    continue;
                }
                at.insert(r.key.clone(), r.seq);
                match r.e {
                    Some(e) => c.entries.insert(r.key, Arc::new(e)),
                    None => c.entries.remove(&r.key),
                };
            }
        }
        *c.disk.log.lock().unwrap_or_else(|e| e.into_inner()) = log_len;
        c.disk.log_len.store(log_len, Ordering::Relaxed);
        c.disk.base.store(base_len, Ordering::Relaxed);
        // 上次整理没做完：下次存时整理（旧日志里的东西要进新底稿）
        c.disk.retry.store(had_old, Ordering::Relaxed);
        c.path = path;
        c
    }

    /// Whether `get` would answer, without copying the photo (and its vectors) out: for the many places that only need to know.
    pub fn has(&self, key: &str, size: u64, mtime: i64, tag: &str) -> bool {
        self.entries.get(key).map_or(false, |e| e.tag == tag && e.size == size && e.mtime == mtime)
    }

    /// 已知这张照片（文件没变）时它的尺寸：不管是哪一版模型分析的，尺寸都一样，所以不看 tag。
    /// 探测和扫描用它跳过读文件头：读 395 个 20 MB 的 JPEG 头要一两秒，查表只要微秒。
    pub fn dims(&self, key: &str, size: u64, mtime: i64) -> Option<(u32, u32)> {
        let e = self.entries.get(key)?;
        (e.size == size && e.mtime == mtime && e.photo.width > 0 && e.photo.height > 0).then_some((e.photo.width, e.photo.height))
    }

    /// A cached photo, if the file is unchanged. 缩略图不在了（手机的"清理加速"会清应用缓存）也算：分析结果没丢，
    /// 调用方只要重做缩略图（`scan_project` 会做），不用整张重新分析。
    pub fn get(&self, key: &str, size: u64, mtime: i64, tag: &str) -> Option<Photo> {
        let e = self.entries.get(key)?;
        if e.tag != tag || e.size != size || e.mtime != mtime {
            return None;
        }
        let mut p = e.photo.clone();
        p.hash = e.hash;
        p.embedding = e.emb.clone();
        p.face_embs = e.fe.clone();
        Some(p)
    }

    pub fn put(&mut self, size: u64, mtime: i64, tag: &str, p: &Photo) {
        // a re-analysis of an unchanged file keeps the search embedding it already has
        let (sem, color) = self.entries.get(&p.path).filter(|e| e.size == size && e.mtime == mtime).map(|e| (e.sem.clone(), e.color.clone())).unwrap_or_default();
        // 文件没变而记录重做了（换了模型、强制重新分析）：用户转过的角度留着
        let rot = self.entries.get(&p.path).filter(|e| e.size == size && e.mtime == mtime).map(|e| e.photo.rotation).unwrap_or(0);
        let turned;
        let p = if rot != 0 && p.rotation == 0 {
            let mut q = p.clone();
            if let Some(dir) = Path::new(&p.thumb).parent().map(Path::to_path_buf) {
                let _ = crate::analyze::apply_rotation(&mut q, rot, &dir);
            }
            turned = q;
            &turned
        } else {
            p
        };
        self.entries.insert(
            p.path.clone(),
            Arc::new(Entry { tag: tag.to_string(), size, mtime, photo: p.clone(), hash: p.hash, emb: p.embedding.clone(), sem, color, fe: p.face_embs.clone() }),
        );
        self.dirty.insert(p.path.clone());
    }

    /// The vector to search with: the dedicated one, or the analysis embedding itself when that already is ViT-B/32 (512 numbers).
    pub fn semantic(&self, key: &str) -> Option<&[f32]> {
        let e = self.entries.get(key)?;
        if !e.sem.is_empty() { Some(&e.sem) } else if e.emb.len() == 512 { Some(&e.emb) } else { None }
    }

    pub fn set_index(&mut self, key: &str, size: u64, mtime: i64, sem: Option<Vec<f32>>, color: Vec<u8>) {
        if let Some(e) = self.entries.get_mut(key) {
            if e.size == size && e.mtime == mtime {
                // 快照（整理中的底稿）还拿着这条时先拷一份再改
                let e = Arc::make_mut(e);
                if let Some(v) = sem { e.sem = v; }
                e.color = color;
                self.dirty.insert(key.to_string());
            }
        }
    }

    pub fn color(&self, key: &str) -> Option<&[u8]> {
        self.entries.get(key).map(|e| e.color.as_slice()).filter(|c| !c.is_empty())
    }

    /// 忘掉这些文件夹（含子文件夹）里照片的分析结果，`keep` 里的文件夹除外（别的项目还用着），返回忘掉的照片路径。
    pub fn forget_under(&mut self, dirs: &[String], keep: &[String]) -> Vec<String> {
        let under = |k: &str, d: &String| k.starts_with(&format!("{}/", d.trim_end_matches('/')));
        let gone: Vec<String> = self.entries.keys().filter(|k| dirs.iter().any(|d| under(k, d)) && !keep.iter().any(|d| under(k, d))).cloned().collect();
        for k in &gone {
            self.entries.remove(k);
            self.dirty.insert(k.clone());
        }
        gone
    }

    /// The photo record of an entry (any analysis version), for showing search results.
    pub fn photo(&self, key: &str) -> Option<&Photo> {
        self.entries.get(key).map(|e| &e.photo)
    }

    /// 可改的记录（转照片用）。
    pub fn photo_mut(&mut self, key: &str) -> Option<&mut Photo> {
        let e = self.entries.get_mut(key)?;
        self.dirty.insert(key.to_string());
        Some(&mut Arc::make_mut(e).photo)
    }

    /// Entries the search index still has to look at (no search vector or no colour signature), with their thumbnail (a 640 px JPEG:
    /// plenty for CLIP and very cheap to decode) and whether a search vector is needed as well.
    pub fn without_index(&self, keys: &[String]) -> Vec<(String, String, u64, i64, bool)> {
        keys.iter()
            .filter_map(|k| self.entries.get(k).map(|e| (k, e)))
            .filter(|(_, e)| Path::new(&e.photo.thumb).exists())
            .filter_map(|(k, e)| {
                let need_vec = e.sem.is_empty() && e.emb.len() != 512;
                (need_vec || e.color.is_empty()).then(|| (k.clone(), e.photo.thumb.clone(), e.size, e.mtime, need_vec))
            })
            .collect()
    }

    /// A sample of analysed photos (file stem and embedding): the "typical photo" the learned taste is compared against.
    /// At most a couple of thousand, spread over the cache: that is plenty to stand for "typical", and cloning every
    /// embedding of a large cache (tens of MB) on each keypress was what made marking a photo stall.
    pub fn pool(&self) -> Vec<(String, Vec<f32>)> {
        const MAX: usize = 2000;
        let with: Vec<_> = self.entries.values().filter(|e| !e.emb.is_empty()).collect();
        let step = (with.len() as f32 / MAX as f32).max(1.0);
        let mut out = Vec::with_capacity(with.len().min(MAX));
        let mut i = 0f32;
        while (i as usize) < with.len() && out.len() < MAX {
            let e = with[i as usize];
            out.push((Path::new(&e.photo.name).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default(), e.emb.clone()));
            i += step;
        }
        out
    }

    pub fn embedding(&self, key: &str) -> Option<Vec<f32>> {
        self.entries.get(key).map(|e| e.emb.clone()).filter(|v| !v.is_empty())
    }

    /// 定下要落盘的东西（在锁里调，便宜）：改过的记录交给日志，日志太长时改成整理一次。真正的序列化与写盘在返回的 `Pending::write`，
    /// 调用方出了锁再调。
    pub fn save(&mut self) -> Pending {
        let disk = self.disk.clone();
        let path = self.path.clone();
        let log_len = disk.log_len.load(Ordering::Relaxed);
        let base = disk.base.load(Ordering::Relaxed);
        let due = disk.retry.load(Ordering::Relaxed) || (log_len >= COMPACT_AT && log_len > base / 2);
        // 正在整理（另一个 `Pending` 还没写完）时照常追加
        if due && !disk.compacting.swap(true, Ordering::Relaxed) {
            disk.retry.store(false, Ordering::Relaxed);
            self.dirty.clear();
            // 日志改名成旧日志：从这里起的新行进新日志，旧日志里的都在快照里了（整理写成之后删）
            {
                let mut len = disk.log.lock().unwrap_or_else(|e| e.into_inner());
                let (cur, old) = (log_path(&path), old_log_path(&path));
                if cur.exists() {
                    if old.exists() {
                        // 上次整理没写成，旧日志还在：接到它后面（它也还没进底稿）
                        let ok = std::fs::read(&cur).ok().and_then(|b| {
                            let mut f = std::fs::OpenOptions::new().append(true).open(&old).ok()?;
                            std::io::Write::write_all(&mut f, &b).ok()?;
                            f.sync_data().ok()
                        });
                        if ok.is_some() {
                            let _ = std::fs::remove_file(&cur);
                        }
                        // 没接上就留着当前日志（它的行都不比快照新，读的时候会跳过）
                        *len = std::fs::metadata(&cur).map_or(0, |m| m.len());
                    } else {
                        let _ = std::fs::rename(&cur, &old);
                        *len = 0;
                    }
                }
                disk.log_len.store(*len, Ordering::Relaxed);
            }
            return Pending { path, disk, job: Job::Compact(self.entries.clone(), self.seq) };
        }
        if self.dirty.is_empty() {
            return Pending { path, disk, job: Job::Nothing };
        }
        let mut keys: Vec<String> = self.dirty.drain().collect();
        keys.sort();
        let lines = keys
            .into_iter()
            .map(|k| {
                self.seq += 1;
                let e = self.entries.get(&k).cloned();
                (self.seq, k, e)
            })
            .collect();
        Pending { path, disk, job: Job::Append(lines) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big(n: usize) -> Cache {
        let mut c = Cache::default();
        let mut seed = 5u64;
        let mut rnd = move || { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); ((seed >> 40) as f32) / ((1u64 << 24) as f32) - 0.5 };
        for i in 0..n {
            let emb: Vec<f32> = (0..768).map(|_| rnd()).collect();
            let sem: Vec<f32> = (0..512).map(|_| rnd()).collect();
            let p = Photo { path: format!("/p/{i}.jpg"), name: format!("{i}.jpg"), thumb: "/tmp".into(), embedding: emb, face_embs: vec![(0..128).map(|_| rnd()).collect()], ..Default::default() };
            c.put(1, 1, "t", &p);
            c.set_index(&p.path, 1, 1, Some(sem), vec![1; 20]);
        }
        c
    }

    /// 每个测试自己的文件夹，底稿放在里面。
    fn place(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fg-cache-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("analysis-cache.json")
    }

    fn photo(i: usize) -> Photo {
        Photo { path: format!("/p/{i}.jpg"), name: format!("{i}.jpg"), thumb: "/nowhere/t.jpg".into(), embedding: vec![i as f32, 0.5], score: i as f32 / 10.0, ..Default::default() }
    }

    /// 底稿用旧的写法（没有 `seq`）写出来。
    fn as_old_file(c: &Cache) -> serde_json::Value {
        let mut v = serde_json::to_value(BaseOut { entries: &c.entries, seq: 0 }).unwrap();
        v.as_object_mut().unwrap().remove("seq");
        v
    }

    #[test]
    fn old_plain_array_files_still_load() {
        let mut c = Cache::default();
        let p = Photo { path: "/a.jpg".into(), name: "a.jpg".into(), thumb: "/t.jpg".into(), embedding: vec![0.5, -1.25, 3.0], face_embs: vec![vec![0.25, 0.5]], ..Default::default() };
        c.put(1, 2, "t", &p);
        c.set_index("/a.jpg", 1, 2, Some(vec![1.0, 2.0]), vec![1]);
        // what the files looked like before: the vectors as plain JSON arrays, and no `seq`
        let mut v = as_old_file(&c);
        let e = v["entries"]["/a.jpg"].as_object_mut().unwrap();
        e.insert("emb".into(), serde_json::json!([0.5, -1.25, 3.0]));
        e.insert("sem".into(), serde_json::json!([1.0, 2.0]));
        e.insert("fe".into(), serde_json::json!([[0.25, 0.5]]));
        let path = place("plain");
        std::fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
        let old = Cache::load(path.clone());
        let o = old.entries.get("/a.jpg").unwrap();
        assert_eq!((o.emb.clone(), o.sem.clone(), o.fe.clone()), (vec![0.5, -1.25, 3.0], vec![1.0, 2.0], vec![vec![0.25, 0.5]]));
        // and what is written now reads back to the same numbers, whatever the length
        let again: BaseIn = serde_json::from_str(&serde_json::to_string(&BaseOut { entries: &old.entries, seq: 0 }).unwrap()).unwrap();
        let a = again.entries.get("/a.jpg").unwrap();
        assert_eq!((a.emb.clone(), a.sem.clone(), a.fe.clone()), (vec![0.5, -1.25, 3.0], vec![1.0, 2.0], vec![vec![0.25, 0.5]]));
        for len in 0..9usize {
            let v: Vec<f32> = (0..len).map(|i| i as f32 * 0.37 - 1.0).collect();
            let mut c = Cache::default();
            c.put(1, 2, "t", &Photo { path: "/b.jpg".into(), embedding: v.clone(), ..Default::default() });
            let back: BaseIn = serde_json::from_str(&serde_json::to_string(&BaseOut { entries: &c.entries, seq: 0 }).unwrap()).unwrap();
            assert_eq!(back.entries.get("/b.jpg").unwrap().emb, v, "length {len}");
        }
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// 改动进日志，读回来一样：新增、转照片、搜索向量、忘掉文件夹；底稿只在整理时写。
    #[test]
    fn changes_go_to_the_log_and_read_back() {
        let path = place("log");
        let mut c = Cache::load(path.clone());
        for i in 0..5 {
            c.put(1, 1, "t", &photo(i));
        }
        c.save().write();
        assert!(!path.exists(), "a small change does not rewrite the whole file");
        assert!(log_path(&path).exists());
        c.photo_mut("/p/2.jpg").unwrap().rotation = 90;
        c.set_index("/p/3.jpg", 1, 1, Some(vec![9.0; 4]), vec![7]);
        c.save().write();
        assert!(matches!(c.save().job, Job::Nothing), "nothing changed, nothing to write");
        let back = Cache::load(path.clone());
        assert_eq!(back.entries.len(), 5);
        assert_eq!(back.photo("/p/2.jpg").unwrap().rotation, 90);
        assert_eq!(back.semantic("/p/3.jpg").unwrap(), &[9.0; 4]);
        assert_eq!(back.get("/p/4.jpg", 1, 1, "t").unwrap().embedding, vec![4.0, 0.5]);
        // 忘掉的记下"删掉"，读回来就没有
        let mut c = back;
        assert_eq!(c.forget_under(&["/p".into()], &[]).len(), 5);
        c.put(1, 1, "t", &photo(9));
        c.save().write();
        let back = Cache::load(path.clone());
        assert_eq!(back.entries.keys().collect::<Vec<_>>(), vec!["/p/9.jpg"]);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// 旧版本写的底稿（没有 `seq`）照样读，之后的改动接着进日志。
    #[test]
    fn an_old_cache_file_keeps_working() {
        let path = place("old");
        let mut c = Cache::default();
        for i in 0..3 {
            c.put(1, 1, "t", &photo(i));
        }
        std::fs::write(&path, serde_json::to_vec(&as_old_file(&c)).unwrap()).unwrap();
        let mut c = Cache::load(path.clone());
        assert_eq!(c.entries.len(), 3);
        c.put(1, 1, "t", &photo(3));
        c.photo_mut("/p/0.jpg").unwrap().rotation = 180;
        c.save().write();
        let back = Cache::load(path.clone());
        assert_eq!(back.entries.len(), 4);
        assert_eq!(back.photo("/p/0.jpg").unwrap().rotation, 180);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// 断电时最后一行只写了一半：那一行丢掉，前面的都在；文件截齐，后面的追加不会接在半行后面。
    #[test]
    fn a_torn_last_line_is_dropped() {
        let path = place("torn");
        let mut c = Cache::load(path.clone());
        c.put(1, 1, "t", &photo(0));
        c.save().write();
        let half = br#"{"seq":99,"key":"/p/1.jpg","e":{"tag":"t","si"#;
        let mut f = std::fs::OpenOptions::new().append(true).open(log_path(&path)).unwrap();
        std::io::Write::write_all(&mut f, half).unwrap();
        drop(f);
        let mut c = Cache::load(path.clone());
        assert_eq!(c.entries.keys().collect::<Vec<_>>(), vec!["/p/0.jpg"]);
        c.put(1, 1, "t", &photo(2));
        c.save().write();
        let back = Cache::load(path.clone());
        let mut keys: Vec<_> = back.entries.keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, vec!["/p/0.jpg", "/p/2.jpg"]);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// 整理：快照写成新底稿、旧日志删掉；整理期间（快照之后、写完之前）的改动进新日志，不丢。
    #[test]
    fn compacting_keeps_what_changed_meanwhile() {
        let path = place("compact");
        let mut c = Cache::load(path.clone());
        for i in 0..4 {
            c.put(1, 1, "t", &photo(i));
        }
        c.save().write();
        c.disk.retry.store(true, Ordering::Relaxed);
        c.put(1, 1, "t", &photo(4));
        let compact = c.save();
        assert!(matches!(compact.job, Job::Compact(..)));
        assert!(old_log_path(&path).exists() && !log_path(&path).exists());
        // 快照之后、整理写完之前又改了两张：正在整理，所以只追加
        c.photo_mut("/p/1.jpg").unwrap().rotation = 270;
        c.put(1, 1, "t", &photo(5));
        let append = c.save();
        assert!(matches!(append.job, Job::Append(..)));
        append.write();
        // 这时断电：旧底稿（没有）+ 旧日志 + 新日志
        let mid = Cache::load(path.clone());
        assert_eq!(mid.entries.len(), 5, "photo 4 was only in the snapshot, the rest are in the logs");
        assert_eq!(mid.photo("/p/1.jpg").unwrap().rotation, 270);
        compact.write();
        assert!(path.exists() && !old_log_path(&path).exists());
        let back = Cache::load(path.clone());
        assert_eq!(back.entries.len(), 6);
        assert_eq!(back.photo("/p/1.jpg").unwrap().rotation, 270);
        let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["entries"].as_object().unwrap().len(), 5, "the new base is the snapshot");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// 上次整理没做完（旧日志还在）：读回来不缺东西，下一次存就整理掉。
    #[test]
    fn an_unfinished_compaction_is_finished_later() {
        let path = place("unfinished");
        let mut c = Cache::load(path.clone());
        for i in 0..3 {
            c.put(1, 1, "t", &photo(i));
        }
        c.save().write();
        std::fs::rename(log_path(&path), old_log_path(&path)).unwrap();
        let mut c = Cache::load(path.clone());
        assert_eq!(c.entries.len(), 3);
        c.put(1, 1, "t", &photo(3));
        let p = c.save();
        assert!(matches!(p.job, Job::Compact(..)), "the leftover old log is folded in");
        p.write();
        assert!(!old_log_path(&path).exists());
        assert_eq!(Cache::load(path.clone()).entries.len(), 4);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// 追加没写成（这里让日志路径是个文件夹，打不开）：那批行不丢，下一次存改成整理、整份快照写进底稿。
    #[test]
    fn a_failed_append_is_made_good_by_the_next_compaction() {
        let path = place("append-fail");
        let mut c = Cache::load(path.clone());
        std::fs::create_dir_all(log_path(&path)).unwrap();
        for i in 0..3 {
            c.put(1, 1, "t", &photo(i));
        }
        let p = c.save();
        assert!(matches!(p.job, Job::Append(..)));
        p.write();
        assert!(c.disk.retry.load(Ordering::Relaxed), "写不进去就记下要重来");
        std::fs::remove_dir_all(log_path(&path)).unwrap();
        c.put(1, 1, "t", &photo(3));
        let p = c.save();
        assert!(matches!(p.job, Job::Compact(..)), "下一次存整理");
        p.write();
        assert_eq!(Cache::load(path.clone()).entries.len(), 4, "没写进日志的三张也在");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// 两万张的缓存：锁里要做的（定下写什么）和锁外的写盘各多久。以前整文件序列化 + fsync 全在锁里。
    #[test]
    fn what_a_large_cache_costs() {
        let n = 20_000;
        let mut c = big(n);
        c.path = place("large");
        // 整理（第一次存、或日志长了）：锁里只是快照
        c.disk.retry.store(true, Ordering::Relaxed);
        let t = std::time::Instant::now();
        let p = c.save();
        let locked = t.elapsed();
        let t = std::time::Instant::now();
        p.write();
        let written = t.elapsed();
        let size = std::fs::metadata(&c.path).map(|m| m.len()).unwrap_or(0);
        // 分析中的一批（100 张）和转一张照片：只追加
        for i in 0..100 {
            c.photo_mut(&format!("/p/{i}.jpg")).unwrap().score = 0.5;
        }
        let t = std::time::Instant::now();
        let p = c.save();
        let batch_locked = t.elapsed();
        let t = std::time::Instant::now();
        p.write();
        let batch_written = t.elapsed();
        c.photo_mut("/p/7.jpg").unwrap().rotation = 90;
        let t = std::time::Instant::now();
        c.save().write();
        let turn = t.elapsed();
        let t = std::time::Instant::now();
        let back = Cache::load(c.path.clone());
        let loaded = t.elapsed();
        println!(
            "{n} photos: file {} MB; compaction {locked:?} in the lock + {written:?} writing; a batch of 100 {batch_locked:?} in the lock + {batch_written:?} writing; \
             turning one photo {turn:?}; load {loaded:?}",
            size / 1_000_000
        );
        assert_eq!(back.entries.len(), n);
        let a = c.entries.get("/p/777.jpg").unwrap();
        let b = back.entries.get("/p/777.jpg").unwrap();
        assert_eq!(a.emb, b.emb);
        assert_eq!(a.sem, b.sem);
        assert_eq!(a.fe, b.fe);
        assert_eq!(back.photo("/p/7.jpg").unwrap().rotation, 90);
        assert_eq!(back.photo("/p/50.jpg").unwrap().score, 0.5);
        let _ = std::fs::remove_dir_all(c.path.parent().unwrap());
    }
}
