//! 网络共享（NAS）：应用自己用纯 Rust 的 SMB2/3 客户端（`smb2` crate）连接，不经过访达，也不依赖系统挂载，
//! 这样 macOS 和以后的 Android 共用同一份实现。
//!
//! 一台 NAS 记为服务器地址 + 账号；它的共享不用事先选，浏览时列出来，作为路径的第一级（`共享/文件夹/…`）。
//! 每台 NAS 在应用数据目录下有一个"镜像"文件夹（`nas/<id>/<共享>/…`）：项目的文件夹就是镜像里的本地路径。扫描前先把远端目录同步进来
//! （只拉新增或变化的原图，分析本来就要完整读一遍，没有额外流量；本地已经没有的远端文件也会从镜像里删掉），之后分析、查看、
//! 导出都照旧走本地文件；淘汰照片则通过客户端在 NAS 上移进回收文件夹，再删掉本地副本。
//!
//! 共享只记地址和用户名（`nas.json`），密码在应用自己的加密仓库里（secrets.rs）。
//! 客户端是异步的，而扫描跑在阻塞线程上，所以所有连接都住在一条专用线程里，用通道收发请求；调用方拿到的是同步接口。

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use smb2::{ClientConfig, ErrorKind, FileTimes, SmbClient, Tree};

#[derive(Clone, Serialize, Deserialize)]
pub struct Share {
    pub id: String,
    pub name: String,
    /// 服务器地址，`host` 或 `host:port`。
    pub host: String,
    /// 空 = 访客
    #[serde(default)]
    pub user: String,
    /// 停用的 NAS 不自动连接、不出现在选文件夹里；它上面的项目打不开，但什么都不删。
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

/// 设置页看到的一台 NAS。
#[derive(Clone, Serialize, Deserialize)]
pub struct ShareStatus {
    pub id: String,
    pub name: String,
    pub host: String,
    pub user: String,
    pub enabled: bool,
    /// 镜像文件夹：从这台 NAS 新建的项目，文件夹都在它下面。
    pub local: String,
    pub connected: bool,
    /// 没连上时最后一次连接的错误（带 `errors` 约定的前缀：`auth:` 密码不对、`net:` 连不上、`denied:` 没权限）；连着时为 null。
    #[serde(default)]
    pub error: Option<String>,
}

/// 浏览共享时的一项：只有文件夹和照片，别的都不列。
#[derive(Clone, Serialize)]
pub struct Entry {
    pub name: String,
    pub dir: bool,
    pub size: u64,
}

/// 从 NAS 拉取原图的进度，发给页面显示。
#[derive(Clone, Serialize)]
pub struct SyncProgress {
    pub share: String,
    pub done: usize,
    pub total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

#[derive(Default, Serialize, Deserialize)]
struct Store {
    list: Vec<Share>,
}

/// 淘汰的照片在共享里去的地方（NAS 上没有废纸篓）。
pub const RECYCLE_DIR: &str = ".FleetingGleam Recycle Bin";

/// 检查服务器地址：`nas.local`、`192.168.1.9` 或带端口的 `host:1445`；顺手容忍写成 `smb://host/` 的。
pub fn check_host(host: &str) -> Result<String, String> {
    let h = host.trim().trim_start_matches("smb://").trim_start_matches("SMB://").trim_end_matches('/');
    if h.is_empty() {
        return Err("the server's address is needed, like nas.local or 192.168.1.9".to_string());
    }
    if h.contains('/') || h.contains('@') || h.contains(' ') {
        return Err("give the server's address only, like nas.local or 192.168.1.9; the share and the folder are picked when a project is started".to_string());
    }
    Ok(h.to_string())
}

/// 连接用的 `host:port`，没写端口就是 445。
fn addr_of(host: &str) -> String {
    if host.rsplit(':').next().map_or(false, |p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) && host.matches(':').count() == 1 {
        host.to_string()
    } else {
        format!("{host}:445")
    }
}

/// 钥匙串条目用的主机名（不带端口）。
fn server_of(host: &str) -> &str {
    match host.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => h,
        _ => host,
    }
}

/// 一条路径拆成共享名和共享内的路径：`photos/2024/trip` → (`photos`, `2024/trip`)。
pub fn split_share(rel: &str) -> (&str, &str) {
    let rel = rel.trim_matches('/');
    match rel.split_once('/') {
        Some((share, rest)) => (share, rest),
        None => (rel, ""),
    }
}

pub fn load(dir: &Path) -> Vec<Share> {
    std::fs::read(dir.join("nas.json")).ok().and_then(|b| serde_json::from_slice::<Store>(&b).ok()).map(|s| s.list).unwrap_or_default()
}

pub fn save(dir: &Path, list: &[Share]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(&Store { list: list.to_vec() }).map_err(|e| e.to_string())?;
    let path = dir.join("nas.json");
    let tmp = path.with_extension("json.tmp");
    crate::write_synced(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))
}

/// 一台 NAS 的镜像文件夹：应用数据目录下自己的一格；下一级是共享名。
pub fn local_dir(data_dir: &Path, id: &str) -> PathBuf {
    data_dir.join("nas").join(id)
}

/// 一台 NAS 的镜像在本机占多少字节。
pub fn mirror_bytes(data_dir: &Path, id: &str) -> u64 {
    walkdir::WalkDir::new(local_dir(data_dir, id)).into_iter().filter_map(Result::ok).filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum()
}

/// 本地路径落在哪台 NAS 的镜像里：那台 NAS，以及从共享名起的路径（`/` 分隔，根是空串）。
pub fn share_of_path<'a>(data_dir: &Path, shares: &'a [Share], path: &Path) -> Option<(&'a Share, String)> {
    shares.iter().find_map(|s| {
        let rel = path.strip_prefix(local_dir(data_dir, &s.id)).ok()?;
        let rel = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
        Some((s, rel))
    })
}

/// 共享里的路径拼出来（`/` 分隔，去掉多余的斜杠）。
fn join_rel(dir: &str, name: &str) -> String {
    if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") }
}

// ---------------------------------------------------------------- 密码：本地加密仓库（见 secrets.rs）

fn secret_id(s: &Share) -> String {
    format!("nas:{}:{}", server_of(&s.host), s.user)
}

/// 记住一台 NAS 的密码。
pub fn remember_password(dir: &Path, s: &Share, password: &str) -> Result<(), String> {
    if s.user.is_empty() {
        return Ok(());
    }
    crate::secrets::set(dir, &secret_id(s), password)
}

/// 记住的密码（访客没有密码，返回空串）。
pub fn saved_password(dir: &Path, s: &Share) -> Option<String> {
    if s.user.is_empty() { Some(String::new()) } else { crate::secrets::get(dir, &secret_id(s)) }
}

pub fn forget_password(dir: &Path, s: &Share) {
    if !s.user.is_empty() {
        crate::secrets::delete(dir, &secret_id(s));
    }
}

// ---------------------------------------------------------------- 客户端线程

/// 一台连着的 NAS：一个会话，用到的共享各开一棵树（第一次用到时打开）。
struct Conn {
    client: SmbClient,
    trees: HashMap<String, Tree>,
}

/// 某个共享的树；`client` 和 `trees` 分开借，调用方还要接着用 `client`。
async fn tree_in<'a>(client: &mut SmbClient, trees: &'a mut HashMap<String, Tree>, share: &str) -> Result<&'a mut Tree, smb2::Error> {
    if !trees.contains_key(share) {
        let t = client.connect_share(share).await?;
        trees.insert(share.to_string(), t);
    }
    Ok(trees.get_mut(share).expect("just inserted"))
}

/// 服务器上的磁盘共享（打印机、IPC$ 和以 $ 结尾的隐藏共享不算），按名字排。
async fn shares_of(client: &mut SmbClient) -> Result<Vec<String>, smb2::Error> {
    let mut names: Vec<String> = client.list_shares().await?.into_iter().filter(|s| s.share_type & 0x3FFF_FFFF == 0 && !s.name.ends_with('$')).map(|s| s.name).collect();
    names.sort_by_key(|n| n.to_lowercase());
    Ok(names)
}

/// 发给客户端线程的一件事；每件事带自己的回信通道。
enum Op {
    /// 连上一台服务器并列出它的共享（添加 NAS 时"测试连接"用；不记任何东西）。
    Probe { host: String, user: String, password: String, reply: mpsc::Sender<Result<Vec<String>, String>> },
    Connect { share: Share, password: String, reply: mpsc::Sender<Result<(), String>> },
    Disconnect { id: String, reply: mpsc::Sender<()> },
    Browse { id: String, rel: String, reply: mpsc::Sender<Result<Vec<Entry>, String>> },
    Sync { id: String, rel: String, local_root: PathBuf, progress: Box<dyn Fn(SyncProgress) + Send>, cancel: Box<dyn Fn() -> bool + Send>, reply: mpsc::Sender<Result<(), String>> },
    Recycle { id: String, rels: Vec<String>, reply: mpsc::Sender<Vec<Result<String, String>>> },
    Upload { id: String, dir: String, files: Vec<PathBuf>, progress: Box<dyn Fn(usize) + Send>, cancel: Box<dyn Fn() -> bool + Send>, reply: mpsc::Sender<Vec<Result<Sent, String>>> },
}

/// 上传一张的结果。
#[derive(Clone, Debug, PartialEq)]
pub enum Sent {
    /// 传上去了，在 NAS 上的位置（`共享/路径`）。
    Uploaded(String),
    /// NAS 上已有同名、同大小、同修改时间的：以前导出过，没再传。
    Same(String),
}

/// 所有共享的连接，住在一条自己的线程里；这里是同步的门面。
pub struct Pool {
    tx: Mutex<mpsc::Sender<Op>>,
    connected: Arc<Mutex<HashSet<String>>>,
}

impl Pool {
    pub fn start() -> Pool {
        let (tx, rx) = mpsc::channel::<Op>();
        let connected = Arc::new(Mutex::new(HashSet::new()));
        let mine = connected.clone();
        std::thread::Builder::new()
            .name("nas".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("tokio runtime for the NAS client");
                let mut conns: HashMap<String, Conn> = HashMap::new();
                for op in rx {
                    rt.block_on(handle(&mut conns, &mine, op));
                }
            })
            .expect("NAS client thread");
        Pool { tx: Mutex::new(tx), connected }
    }

    fn ask<T>(&self, make: impl FnOnce(mpsc::Sender<T>) -> Op) -> Result<T, String> {
        let (tx, rx) = mpsc::channel();
        self.tx.lock().unwrap().send(make(tx)).map_err(|_| "the NAS client has stopped".to_string())?;
        rx.recv().map_err(|_| "the NAS client has stopped".to_string())
    }

    pub fn is_connected(&self, id: &str) -> bool {
        self.connected.lock().unwrap().contains(id)
    }

    /// 试着连一台服务器，返回它的共享名（隐藏共享和 IPC$ 之类不算）。
    pub fn probe(&self, host: &str, user: &str, password: &str) -> Result<Vec<String>, String> {
        self.ask(|reply| Op::Probe { host: host.to_string(), user: user.to_string(), password: password.to_string(), reply })?
    }

    /// 连上（已连着的先断开再连）。
    pub fn connect(&self, share: &Share, password: &str) -> Result<(), String> {
        self.ask(|reply| Op::Connect { share: share.clone(), password: password.to_string(), reply })?
    }

    pub fn disconnect(&self, id: &str) {
        let _ = self.ask(|reply| Op::Disconnect { id: id.to_string(), reply });
    }

    /// NAS 上一个位置的内容：根（空串）是共享列表，往下是子文件夹和照片。
    pub fn browse(&self, id: &str, rel: &str) -> Result<Vec<Entry>, String> {
        self.ask(|reply| Op::Browse { id: id.to_string(), rel: rel.to_string(), reply })?
    }

    /// 把 NAS 上的一个文件夹（`共享/路径`，连同子文件夹）同步到镜像：拉新增或变化的照片，删掉远端已经没有的。
    pub fn sync(&self, id: &str, rel: &str, local_root: &Path, progress: impl Fn(SyncProgress) + Send + 'static, cancel: impl Fn() -> bool + Send + 'static) -> Result<(), String> {
        self.ask(|reply| Op::Sync { id: id.to_string(), rel: rel.to_string(), local_root: local_root.to_path_buf(), progress: Box::new(progress), cancel: Box::new(cancel), reply })?
    }

    /// 把共享里的这些照片移进回收文件夹；每张返回它在共享里的新位置。
    pub fn recycle(&self, id: &str, rels: Vec<String>) -> Vec<Result<String, String>> {
        let n = rels.len();
        self.ask(|reply| Op::Recycle { id: id.to_string(), rels, reply }).unwrap_or_else(|e| vec![Err(e); n])
    }

    /// 把本机的这些文件上传到 NAS 上的文件夹 `dir`（`共享/路径`，没有就建）；重名的加序号，从不覆盖；同名同大小同修改时间的不再传。
    /// 每张返回它在 NAS 上的位置；`progress(n)` 是已经处理完几张。`cancel()` 为真就在当前这张之后停：返回的比 `files` 少。
    pub fn upload(&self, id: &str, dir: &str, files: Vec<PathBuf>, progress: impl Fn(usize) + Send + 'static, cancel: impl Fn() -> bool + Send + 'static) -> Vec<Result<Sent, String>> {
        let n = files.len();
        self.ask(|reply| Op::Upload { id: id.to_string(), dir: dir.to_string(), files, progress: Box::new(progress), cancel: Box::new(cancel), reply }).unwrap_or_else(|e| vec![Err(e); n])
    }
}

/// 把客户端的错误说成人话，前缀按 `errors` 的约定：`auth:` 是账号密码被拒，`denied:` 是登录成功但这个共享或文件夹没权限
/// （以前也说成"密码错误"，用户改了密码还是不行），`net:` 是连不上。连接层面的错误同时让调用方知道该把这个共享当作断开。
fn explain(e: &smb2::Error) -> String {
    match e.kind() {
        ErrorKind::AuthRequired | ErrorKind::GuestSessionRefused | ErrorKind::SignatureInvalid => "auth:wrong user name or password".to_string(),
        ErrorKind::AccessDenied => format!("denied:this account has no access to that share or folder ({e})"),
        ErrorKind::NotFound => "no such share or folder on that server".to_string(),
        ErrorKind::ConnectionLost | ErrorKind::TimedOut | ErrorKind::Io | ErrorKind::SessionExpired => format!("net:the server cannot be reached ({e})"),
        ErrorKind::Cancelled => crate::errors::CANCELLED.to_string(),
        _ => e.to_string(),
    }
}

fn lost(e: &smb2::Error) -> bool {
    matches!(e.kind(), ErrorKind::ConnectionLost | ErrorKind::TimedOut | ErrorKind::Io | ErrorKind::SessionExpired)
}

async fn handle(conns: &mut HashMap<String, Conn>, connected: &Mutex<HashSet<String>>, op: Op) {
    match op {
        Op::Probe { host, user, password, reply } => {
            let _ = reply.send(probe(&host, &user, &password).await);
        }
        Op::Connect { share, password, reply } => {
            conns.remove(&share.id);
            connected.lock().unwrap().remove(&share.id);
            let r = match connect(&share, &password).await {
                Ok(c) => {
                    conns.insert(share.id.clone(), c);
                    connected.lock().unwrap().insert(share.id.clone());
                    Ok(())
                }
                Err(e) => Err(e),
            };
            let _ = reply.send(r);
        }
        Op::Disconnect { id, reply } => {
            conns.remove(&id);
            connected.lock().unwrap().remove(&id);
            let _ = reply.send(());
        }
        Op::Browse { id, rel, reply } => {
            let r = match conns.remove(&id) {
                Some(mut c) => { let r = browse(&mut c, &rel).await; settle(conns, connected, &id, c, r) }
                None => Err("not connected".to_string()),
            };
            let _ = reply.send(r);
        }
        Op::Sync { id, rel, local_root, progress, cancel, reply } => {
            if split_share(&rel).0.is_empty() {
                // 整台 NAS 不能当项目文件夹：至少要选到一个共享
                let _ = reply.send(Err("pick a share (or a folder in one) first".to_string()));
                return;
            }
            let r = match conns.remove(&id) {
                Some(mut c) => { let r = sync(&mut c, &rel, &local_root, &*progress, &*cancel).await; settle(conns, connected, &id, c, r) }
                None => Err("not connected".to_string()),
            };
            let _ = reply.send(r);
        }
        Op::Recycle { id, rels, reply } => {
            let n = rels.len();
            let r = match conns.remove(&id) {
                Some(mut c) => { let r = recycle(&mut c, &rels).await; settle(conns, connected, &id, c, r) }
                None => Err("not connected".to_string()),
            }
            .unwrap_or_else(|e| vec![Err(e); n]);
            let _ = reply.send(r);
        }
        Op::Upload { id, dir, files, progress, cancel, reply } => {
            let n = files.len();
            let r = match conns.remove(&id) {
                Some(mut c) => { let r = upload(&mut c, &dir, &files, &*progress, &*cancel).await; settle(conns, connected, &id, c, r) }
                None => Err("not connected".to_string()),
            }
            .unwrap_or_else(|e| vec![Err(e); n]);
            let _ = reply.send(r);
        }
    }
}

/// 一件事做完之后：连接放回去；连接层面的错误则不放回，这个共享回到"未连接"。
fn settle<T>(conns: &mut HashMap<String, Conn>, connected: &Mutex<HashSet<String>>, id: &str, c: Conn, r: Result<T, smb2::Error>) -> Result<T, String> {
    match r {
        Ok(v) => {
            conns.insert(id.to_string(), c);
            Ok(v)
        }
        Err(e) => {
            if lost(&e) {
                connected.lock().unwrap().remove(id);
            } else {
                conns.insert(id.to_string(), c);
            }
            Err(explain(&e))
        }
    }
}

/// 连接本身（还没挂到哪个共享上）。
async fn client_for(host: &str, user: &str, password: &str) -> Result<SmbClient, String> {
    let cfg = ClientConfig {
        addr: addr_of(host),
        timeout: Duration::from_secs(8),
        username: user.to_string(),
        password: password.to_string(),
        auto_reconnect: true,
        ..ClientConfig::default()
    };
    SmbClient::connect(cfg).await.map_err(|e| explain(&e))
}

async fn probe(host: &str, user: &str, password: &str) -> Result<Vec<String>, String> {
    let host = check_host(host)?;
    let mut client = client_for(&host, user, password).await?;
    shares_of(&mut client).await.map_err(|e| explain(&e))
}

async fn connect(share: &Share, password: &str) -> Result<Conn, String> {
    Ok(Conn { client: client_for(&share.host, &share.user, password).await?, trees: HashMap::new() })
}

/// 一个目录里值得看的条目：子文件夹（不跟随链接）和够大的照片；系统和 NAS 自己的杂项不算。
fn wanted(e: &smb2::DirectoryEntry) -> Option<Entry> {
    if e.name == "." || e.name == ".." || crate::is_junk(&e.name) {
        return None;
    }
    if e.is_directory {
        return (!e.is_link()).then(|| Entry { name: e.name.clone(), dir: true, size: 0 });
    }
    (crate::imageio::is_supported(Path::new(&e.name)) && e.size >= crate::MIN_PHOTO_BYTES).then(|| Entry { name: e.name.clone(), dir: false, size: e.size })
}

async fn browse(c: &mut Conn, rel: &str) -> Result<Vec<Entry>, smb2::Error> {
    let (share, path) = split_share(rel);
    if share.is_empty() {
        // 根：这台 NAS 的共享，当作文件夹列出来
        return Ok(shares_of(&mut c.client).await?.into_iter().map(|name| Entry { name, dir: true, size: 0 }).collect());
    }
    let Conn { client, trees } = c;
    let tree = tree_in(client, trees, share).await?;
    let mut out: Vec<Entry> = client.list_directory(tree, path).await?.iter().filter_map(wanted).collect();
    out.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

/// 远端的一张照片：共享里的路径、大小、修改时间。
struct Remote {
    rel: String,
    size: u64,
    modified: SystemTime,
}

/// 列一个目录：值得看的条目（见 `wanted`）和各自的修改时间。真的是 SMB 的树，测试里是假的。
trait Dirs {
    async fn list(&mut self, dir: &str) -> Result<Vec<(Entry, SystemTime)>, smb2::Error>;
}

/// 一个共享的树，连同用来发请求的客户端。
struct SmbDirs<'a> {
    client: &'a mut SmbClient,
    tree: &'a mut Tree,
}

impl Dirs for SmbDirs<'_> {
    async fn list(&mut self, dir: &str) -> Result<Vec<(Entry, SystemTime)>, smb2::Error> {
        Ok(self.client.list_directory(self.tree, dir).await?.iter().filter_map(|e| wanted(e).map(|w| (w, e.modified.to_system_time().unwrap_or(UNIX_EPOCH)))).collect())
    }
}

/// 远端列出来的样子：照片，和因为没权限而跳过的子文件夹（路径都相对共享根）。
struct Walked {
    photos: Vec<Remote>,
    /// 这些子文件夹里有什么不知道：镜像里它们下面的照片不能当成"远端已经没有"删掉。
    skipped: Vec<String>,
}

/// 共享里一个文件夹下的全部照片，连同子文件夹；路径相对共享根。
async fn walk(dirs: &mut impl Dirs, rel: &str, cancel: &dyn Fn() -> bool) -> Result<Walked, smb2::Error> {
    let mut stack = vec![rel.to_string()];
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    while let Some(dir) = stack.pop() {
        if cancel() {
            return Err(smb2::Error::Cancelled);
        }
        let entries = match dirs.list(&dir).await {
            Ok(v) => v,
            // 里面某个文件夹没权限（NAS 上别人的私人文件夹、系统目录）：跳过它、记一笔，不让整次同步失败；选的那个文件夹本身没权限照旧报错
            Err(e) if e.kind() == ErrorKind::AccessDenied && dir != rel => {
                crate::platform::log(&format!("NAS：没有权限打开 {dir}，跳过（{e}）"));
                skipped.push(dir);
                continue;
            }
            Err(e) => return Err(e),
        };
        for (w, modified) in entries {
            let rel = join_rel(&dir, &w.name);
            if w.dir {
                stack.push(rel);
            } else {
                out.push(Remote { rel, size: w.size, modified });
            }
        }
    }
    Ok(Walked { photos: out, skipped })
}

fn secs(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// 本地副本是否就是远端这张（大小和修改时间都一样）。
fn mirrored(local: &Path, r: &Remote) -> bool {
    std::fs::metadata(local).map_or(false, |m| m.len() == r.size && m.modified().map_or(false, |t| secs(t) == secs(r.modified)))
}

async fn sync(c: &mut Conn, rel: &str, local_root: &Path, progress: &dyn Fn(SyncProgress), cancel: &dyn Fn() -> bool) -> Result<(), smb2::Error> {
    let (share, path) = split_share(rel);
    let share = share.to_string();
    let Conn { client, trees } = c;
    let tree = tree_in(client, trees, &share).await?;
    // 镜像里这个共享自己的一格
    let local_root = local_root.join(&share);
    let local_root = local_root.as_path();
    let Walked { photos: remote, skipped } = walk(&mut SmbDirs { client, tree }, path, cancel).await?;
    let need: Vec<&Remote> = remote.iter().filter(|r| !mirrored(&local_root.join(&r.rel), r)).collect();
    let total = need.len();
    let bytes_total: u64 = need.iter().map(|r| r.size).sum();
    let mut bytes_done = 0u64;
    let report = |done: usize, bytes_done: u64| progress(SyncProgress { share: share.clone(), done, total, bytes_done, bytes_total });
    report(0, 0);
    let mut last = Instant::now();
    for (i, r) in need.iter().enumerate() {
        if cancel() {
            return Err(smb2::Error::Cancelled);
        }
        let local = local_root.join(&r.rel);
        if let Some(parent) = local.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // 先写到旁边的临时文件，整张到齐、时间戳对上之后才换成正式的名字：镜像里永远没有半张照片
        let name = local.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let tmp = local.with_file_name(format!(".{name}.part"));
        let mut download = match client.download(tree, &r.rel).await {
            Ok(d) => d,
            // 单张没权限读：跳过、记一笔，别的照常拉
            Err(e) if e.kind() == ErrorKind::AccessDenied => {
                crate::platform::log(&format!("NAS：没有权限读取 {}，跳过（{e}）", r.rel));
                report(i + 1, bytes_done);
                continue;
            }
            Err(e) => return Err(e),
        };
        let mut file = std::fs::File::create(&tmp)?;
        while let Some(chunk) = download.next_chunk().await {
            let bytes = chunk?;
            file.write_all(&bytes)?;
            bytes_done += bytes.len() as u64;
            if last.elapsed() > Duration::from_millis(250) {
                report(i, bytes_done);
                last = Instant::now();
            }
            if cancel() {
                drop(file);
                let _ = std::fs::remove_file(&tmp);
                return Err(smb2::Error::Cancelled);
            }
        }
        file.flush()?;
        file.set_modified(r.modified)?;
        drop(file);
        std::fs::rename(&tmp, &local)?;
        report(i + 1, bytes_done);
    }
    prune_mirror(local_root, path, &remote, &skipped);
    Ok(())
}

/// 远端已经没有的照片，镜像里也不留（只动照片文件和半截的临时文件；镜像里别的东西不是我们放的，不碰）。
/// 这次没权限打开、跳过了的子文件夹（`skipped`）整个不碰：以前同步过、现在被拒的文件夹，它的照片还在镜像里、项目里照常可看。
fn prune_mirror(local_root: &Path, path: &str, remote: &[Remote], skipped: &[String]) {
    let keep: HashSet<PathBuf> = remote.iter().map(|r| local_root.join(&r.rel)).collect();
    let unknown: Vec<PathBuf> = skipped.iter().map(|d| local_root.join(d)).collect();
    let root = local_root.join(path);
    if !root.is_dir() {
        return;
    }
    let walker = walkdir::WalkDir::new(&root).into_iter().filter_entry(|e| !(e.file_type().is_dir() && unknown.iter().any(|u| e.path() == u.as_path())));
    for e in walker.filter_map(Result::ok).filter(|e| e.file_type().is_file()) {
        let name = e.file_name().to_string_lossy();
        let stale = (crate::imageio::is_supported(e.path()) && !keep.contains(e.path())) || (name.starts_with('.') && name.ends_with(".part"));
        if stale {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// 今天的日期（UTC），回收文件夹按天分开。
pub fn today() -> String {
    let days = (SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) / 86400) as i64;
    // 从 1970 年起的天数算成年月日（Howard Hinnant 的算法），够起一个文件夹名
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}

async fn mkdir_ok(client: &mut SmbClient, tree: &Tree, rel: &str) -> Result<(), smb2::Error> {
    match client.create_directory(tree, rel).await {
        Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(()),
        r => r,
    }
}

/// 把照片（`共享/路径`）移进各自共享根目录的回收文件夹（按天分开，同名的加序号），返回它在 NAS 上的新位置，可以手动放回。
async fn recycle(c: &mut Conn, rels: &[String]) -> Result<Vec<Result<String, String>>, smb2::Error> {
    let day_dir = format!("{RECYCLE_DIR}/{}", today());
    let mut prepared: HashSet<String> = HashSet::new();
    let mut out = Vec::with_capacity(rels.len());
    for full in rels {
        let (share, rel) = split_share(full);
        let Conn { client, trees } = c;
        let tree = tree_in(client, trees, share).await?;
        if prepared.insert(share.to_string()) {
            mkdir_ok(client, tree, RECYCLE_DIR).await?;
            mkdir_ok(client, tree, &day_dir).await?;
        }
        let name = rel.rsplit('/').next().unwrap_or(rel);
        let (stem, ext) = stem_ext(name);
        let mut target = format!("{day_dir}/{name}");
        let mut n = 1;
        while client.stat(tree, &target).await.is_ok() {
            target = format!("{day_dir}/{stem} ({n}){ext}");
            n += 1;
        }
        out.push(match client.rename(tree, rel, &target).await {
            Ok(()) => Ok(format!("{share}/{target}")),
            Err(e) if lost(&e) => return Err(e),
            Err(e) => Err(explain(&e)),
        });
    }
    Ok(out)
}

/// `名字.扩展名` → (`名字`, `.扩展名`)；没有扩展名的扩展名是空串。
fn stem_ext(name: &str) -> (String, String) {
    match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    }
}

/// 上传：先写到目标文件夹里的 `.名字.part`，整张写完、修改时间改成原文件的之后才改成正式名字（同名的加序号），
/// 所以 NAS 上永远没有半张照片，别的设备看到的是完整的文件。原文件不动。
async fn upload(c: &mut Conn, dir: &str, files: &[PathBuf], progress: &dyn Fn(usize), cancel: &dyn Fn() -> bool) -> Result<Vec<Result<Sent, String>>, smb2::Error> {
    let (share, path) = split_share(dir);
    let share = share.to_string();
    let Conn { client, trees } = c;
    let tree = tree_in(client, trees, &share).await?;
    // 目标文件夹（一级一级建，已经有的不管）
    let mut acc = String::new();
    for part in path.split('/').filter(|p| !p.is_empty()) {
        acc = join_rel(&acc, part);
        mkdir_ok(client, tree, &acc).await?;
    }
    let mut out = Vec::with_capacity(files.len());
    for (i, f) in files.iter().enumerate() {
        if cancel() {
            break;
        }
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let (stem, ext) = stem_ext(&name);
        let mut target = join_rel(path, &name);
        // 同名、同大小、同修改时间的已经在 NAS 上（以前导出过；上传时修改时间设成了原图的）：不再传一份 "(1)"
        if let (Ok(there), Ok(m)) = (client.stat(tree, &target).await, std::fs::metadata(f)) {
            let here = m.modified().ok().map(secs);
            if !there.is_directory && there.size == m.len() && here.is_some() && there.modified.to_system_time().map(secs) == here {
                out.push(Ok(Sent::Same(format!("{share}/{target}"))));
                progress(i + 1);
                continue;
            }
        }
        let mut n = 1;
        while client.stat(tree, &target).await.is_ok() {
            target = join_rel(path, &format!("{stem} ({n}){ext}"));
            n += 1;
        }
        let tmp = join_rel(path, &format!(".{name}.part"));
        let r: Result<(), smb2::Error> = async {
            let mut src = std::fs::File::open(f)?;
            let modified = src.metadata().and_then(|m| m.modified()).ok();
            let mut w = client.create_file_writer(tree, &tmp).await?;
            let mut buf = vec![0u8; 1 << 20];
            loop {
                let k = src.read(&mut buf)?;
                if k == 0 {
                    break;
                }
                w.write_chunk(&buf[..k]).await?;
            }
            if let Some(t) = modified {
                w.set_times(FileTimes::new().set_modified(t)).await?;
            }
            w.finish().await?;
            client.rename(tree, &tmp, &target).await
        }
        .await;
        match r {
            Ok(()) => out.push(Ok(Sent::Uploaded(format!("{share}/{target}")))),
            Err(e) if lost(&e) => return Err(e),
            Err(e) => {
                let _ = client.delete_file(tree, &tmp).await;
                out.push(Err(explain(&e)));
            }
        }
        progress(i + 1);
    }
    Ok(out)
}

/// 共享里一个位置的地址（给页面说"导出到了哪里"）。
pub fn share_url(s: &Share, rel: &str) -> String {
    format!("smb://{}/{}", s.host, rel.trim_matches('/'))
}

/// 某个共享的回收文件夹地址，给页面说"移到了哪里"。
pub fn recycle_url(s: &Share, share: &str) -> String {
    format!("smb://{}/{share}/{RECYCLE_DIR}", s.host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_checked_before_anything_is_tried() {
        assert_eq!(check_host(" nas.local ").unwrap(), "nas.local");
        assert_eq!(check_host("smb://192.168.1.9:1445/").unwrap(), "192.168.1.9:1445");
        assert!(check_host("").is_err());
        assert!(check_host("nas.local/photos").is_err(), "the share is picked later");
        assert!(check_host("me@nas.local").is_err(), "the user name has its own field");
        assert_eq!(addr_of("nas.local"), "nas.local:445");
        assert_eq!(addr_of("10.0.0.2:1445"), "10.0.0.2:1445");
        assert_eq!(server_of("10.0.0.2:1445"), "10.0.0.2");
        assert_eq!(split_share("photos/2024/trip"), ("photos", "2024/trip"));
        assert_eq!(split_share("photos"), ("photos", ""));
        assert_eq!(split_share(""), ("", ""));
    }

    #[test]
    fn the_shares_file_round_trips_without_passwords() {
        let dir = std::env::temp_dir().join(format!("fg_nas_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let list = vec![Share { id: "n1".into(), name: "Home NAS".into(), host: "nas.local".into(), user: "me".into(), enabled: true }];
        save(&dir, &list).unwrap();
        let back = load(&dir);
        assert_eq!(back.len(), 1);
        assert_eq!((back[0].id.as_str(), back[0].user.as_str()), ("n1", "me"));
        assert!(!std::fs::read_to_string(dir.join("nas.json")).unwrap().contains("password"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_mirror_path_maps_back_to_its_share() {
        let data = Path::new("/tmp/fgdata");
        let shares = vec![Share { id: "n1".into(), name: "A".into(), host: "nas".into(), user: String::new(), enabled: true }];
        let (s, rel) = share_of_path(data, &shares, &data.join("nas/n1/photos/2024/trip/IMG_1.jpg")).unwrap();
        assert_eq!((s.id.as_str(), rel.as_str()), ("n1", "photos/2024/trip/IMG_1.jpg"));
        assert_eq!(share_of_path(data, &shares, &data.join("nas/n1")).unwrap().1, "");
        assert!(share_of_path(data, &shares, Path::new("/Users/me/Pictures/IMG_1.jpg")).is_none());
        assert_eq!(join_rel("", "a"), "a");
        assert_eq!(join_rel("a/b", "c.jpg"), "a/b/c.jpg");
        assert_eq!(recycle_url(&shares[0], "photos"), "smb://nas/photos/.FleetingGleam Recycle Bin");
    }

    /// 假的 NAS 目录：没列出的目录没权限。
    struct FakeDirs(HashMap<String, Vec<(Entry, SystemTime)>>);

    impl Dirs for FakeDirs {
        async fn list(&mut self, dir: &str) -> Result<Vec<(Entry, SystemTime)>, smb2::Error> {
            self.0.get(dir).cloned().ok_or(smb2::Error::Protocol { status: smb2::types::status::NtStatus::ACCESS_DENIED, command: smb2::types::Command::QueryDirectory })
        }
    }

    #[test]
    fn a_folder_without_permission_is_skipped_not_fatal() {
        let file = |n: &str| (Entry { name: n.into(), dir: false, size: 20_000 }, UNIX_EPOCH);
        let dir = |n: &str| (Entry { name: n.into(), dir: true, size: 0 }, UNIX_EPOCH);
        let mut fake = FakeDirs(HashMap::from([
            ("photos".to_string(), vec![file("a.jpg"), dir("trip"), dir("private")]),
            ("photos/trip".to_string(), vec![file("b.jpg")]),
        ]));
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let found = rt.block_on(walk(&mut fake, "photos", &|| false)).expect("the private folder is skipped");
        assert_eq!(found.skipped, vec!["photos/private"]);
        let mut names: Vec<String> = found.photos.into_iter().map(|r| r.rel).collect();
        names.sort();
        assert_eq!(names, vec!["photos/a.jpg", "photos/trip/b.jpg"]);
        // 选的那个文件夹本身没权限：照旧报错，说成没权限，不说密码错
        let e = rt.block_on(walk(&mut fake, "photos/private", &|| false)).err().expect("the chosen folder itself is refused");
        assert_eq!(e.kind(), ErrorKind::AccessDenied);
        assert!(explain(&e).starts_with("denied:"), "{}", explain(&e));
    }

    /// 删"远端已经没有的"时绕开这次没权限、跳过了的子文件夹：以前同步下来的照片留着；真没了的照样删。
    #[test]
    fn a_skipped_folder_keeps_its_mirrored_photos() {
        let root = std::env::temp_dir().join(format!("fg_nas_prune_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for f in ["photos/a.jpg", "photos/gone.jpg", "photos/private/p.jpg", "photos/private/deeper/q.jpg", "photos/privateer/x.jpg", "photos/trip/.b.jpg.part", "photos/notes.txt"] {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"x").unwrap();
        }
        let remote = vec![Remote { rel: "photos/a.jpg".into(), size: 1, modified: UNIX_EPOCH }];
        prune_mirror(&root, "photos", &remote, &["photos/private".to_string()]);
        let mut left: Vec<String> = walkdir::WalkDir::new(&root).into_iter().filter_map(Result::ok).filter(|e| e.file_type().is_file())
            .map(|e| e.path().strip_prefix(&root).unwrap().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, vec!["photos/a.jpg", "photos/notes.txt", "photos/private/deeper/q.jpg", "photos/private/p.jpg"], "被跳过的子树不动；名字只是开头一样的文件夹照删");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 排错用：直接连一次，把错误的完整样子打出来。
    #[test]
    #[ignore]
    fn debug_connect() {
        let host = std::env::var("FG_NAS_HOST").unwrap();
        let (user, pass) = (std::env::var("FG_NAS_USER").unwrap_or_default(), std::env::var("FG_NAS_PASSWORD").unwrap_or_default());
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            for auto in [false, true] {
                let cfg = ClientConfig { addr: addr_of(&host), timeout: Duration::from_secs(8), username: user.clone(), password: pass.clone(), auto_reconnect: auto, ..ClientConfig::default() };
                match SmbClient::connect(cfg).await {
                    Ok(mut c) => println!("auto_reconnect={auto}: connected; shares: {:?}", c.list_shares().await.map(|v| v.len()).map_err(|e| format!("{e:?}"))),
                    Err(e) => println!("auto_reconnect={auto}: {e:?}"),
                }
            }
        });
    }

    /// 对着一台真的 NAS（密码已在钥匙串里，或由 FG_NAS_PASSWORD 给出）：
    /// `FG_NAS_HOST=192.168.1.9 FG_NAS_USER=me [FG_NAS_DIR=共享/文件夹] [FG_NAS_SYNC=1] [FG_NAS_RECYCLE=1] [FG_NAS_UPLOAD=1] cargo test --lib nas::tests::a_real_nas -- --ignored --nocapture`
    /// FG_NAS_UPLOAD=1：往 FG_NAS_DIR 下的 `.fg-upload-test` 里传两张小图（第二张同名，应得到"(1)"），列出来核对，再移进回收文件夹。
    #[test]
    #[ignore]
    fn a_real_nas_is_browsed_synced_and_recycled_into() {
        let Ok(host) = std::env::var("FG_NAS_HOST") else { return };
        let s = Share { id: "t1".into(), name: "test".into(), host: check_host(&host).unwrap(), user: std::env::var("FG_NAS_USER").unwrap_or_default(), enabled: true };
        let pw = std::env::var("FG_NAS_PASSWORD").expect("FG_NAS_PASSWORD");
        let pool = Pool::start();
        pool.connect(&s, &pw).expect("connect");
        assert!(pool.is_connected("t1"));
        let root = pool.browse("t1", "").expect("list the shares");
        println!("{} shares:", root.len());
        for e in &root {
            println!("  {}/", e.name);
        }
        let Ok(dir) = std::env::var("FG_NAS_DIR") else {
            println!("set FG_NAS_DIR to 共享/文件夹 to browse and sync it");
            pool.disconnect("t1");
            return;
        };
        if std::env::var("FG_NAS_UPLOAD").is_ok() {
            let tmp = std::env::temp_dir().join(format!("fg_nas_up_{}", std::process::id()));
            std::fs::create_dir_all(&tmp).unwrap();
            let img = tmp.join("fg-upload.jpg");
            image::RgbImage::from_fn(64, 48, |x, y| image::Rgb([x as u8 * 4, y as u8 * 5, 90])).save(&img).unwrap();
            let target = format!("{dir}/.fg-upload-test");
            let done = Arc::new(Mutex::new(0usize));
            let seen = done.clone();
            let r = pool.upload("t1", &target, vec![img.clone(), img.clone()], move |n| *seen.lock().unwrap() = n, || false);
            println!("uploaded: {r:?}");
            let r: Vec<Sent> = r.into_iter().map(|x| x.expect("uploaded")).collect();
            assert_eq!(*done.lock().unwrap(), 2);
            // 第二张同名同大小同时间：认出是导出过的，不再传一份 "(1)"
            let up: Vec<String> = r.iter().map(|s| match s { Sent::Uploaded(p) | Sent::Same(p) => p.clone() }).collect();
            assert!(matches!(&r[0], Sent::Uploaded(p) if p.ends_with("/fg-upload.jpg")) && matches!(&r[1], Sent::Same(_)), "{r:?}");
            let up = vec![up[0].clone()];
            let listed: Vec<(String, u64)> = pool.browse("t1", &target).expect("list the upload folder").into_iter().map(|e| (e.name, e.size)).collect();
            let size = std::fs::metadata(&img).unwrap().len();
            assert!(listed.iter().any(|(n, s)| n == "fg-upload.jpg" && *s == size), "{listed:?}");
            assert!(!listed.iter().any(|(n, _)| n.ends_with(".part")), "no half-written files left: {listed:?}");
            let back = pool.recycle("t1", up);
            println!("cleaned up: {back:?}");
            let _ = std::fs::remove_dir_all(&tmp);
        }
        let listing = pool.browse("t1", &dir).expect("list the folder");
        println!("{} entries in {dir}:", listing.len());
        for e in listing.iter().take(20) {
            println!("  {}{}", e.name, if e.dir { "/" } else { "" });
        }
        if std::env::var("FG_NAS_SYNC").is_err() {
            println!("set FG_NAS_SYNC=1 to mirror {dir} (every photo in it is downloaded)");
            pool.disconnect("t1");
            return;
        }
        let data = std::env::temp_dir().join(format!("fg_nas_real_{}", std::process::id()));
        let local = local_dir(&data, "t1");
        pool.sync("t1", &dir, &local, |p| println!("{}/{} ({} / {} bytes)", p.done, p.total, p.bytes_done, p.bytes_total), || false).expect("sync");
        let mirrored: Vec<PathBuf> = walkdir::WalkDir::new(&local).into_iter().filter_map(Result::ok).filter(|e| e.file_type().is_file()).map(|e| e.into_path()).collect();
        println!("{} photos mirrored under {}", mirrored.len(), local.display());
        // 再同步一次：什么都不用拉
        pool.sync("t1", &dir, &local, |p| assert_eq!(p.total, 0, "nothing to fetch the second time"), || false).expect("sync again");
        // 回收一张（FG_NAS_RECYCLE=1 时才动 NAS 上的东西）
        if std::env::var("FG_NAS_RECYCLE").is_ok() {
            if let Some(first) = mirrored.first() {
                let (_, rel) = share_of_path(&data, std::slice::from_ref(&s), first).unwrap();
                let r = pool.recycle("t1", vec![rel.clone()]);
                let target = r[0].clone().expect("recycled");
                println!("{rel} -> {target}");
                let (share, inside) = split_share(&target);
                assert!(inside.starts_with(RECYCLE_DIR));
                let day = format!("{share}/{}", inside.rsplit_once('/').unwrap().0);
                let names: Vec<String> = pool.browse("t1", &day).expect("list the recycle folder").into_iter().map(|e| e.name).collect();
                assert!(names.iter().any(|n| target.ends_with(n.as_str())), "{names:?}");
                // 同步后镜像里也没有它了
                pool.sync("t1", &dir, &local, |_| {}, || false).expect("sync after recycling");
                assert!(!first.exists(), "the mirror copy is gone after the photo left the folder on the share");
            }
        }
        pool.disconnect("t1");
        assert!(!pool.is_connected("t1"));
        let _ = std::fs::remove_dir_all(&data);
    }
}
