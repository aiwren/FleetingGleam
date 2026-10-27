//! 联网用的代理：启动时读 macOS 的系统 HTTPS 代理（`scutil --proxy`，和"系统设置 › 网络 › 代理"是同一份），设置里手填的优先，
//! 交给核心的 `platform::set_proxy`（模型下载、云备份都走它）。系统代理变了，下次启动生效。
//! 只认 HTTP 类代理（`http://主机:端口`）：核心的 HTTP 客户端没编 SOCKS；自动代理配置（PAC）也不解析。
//! 系统的"不使用代理的主机"（`ExceptionsList`）与"排除简单主机名"（`ExcludeSimpleHostnames`）交给核心的
//! `platform::set_proxy_bypass`：这些地址直连。局域网与本机（私有网段、`.local`、`localhost`）核心本来就直连。
//! 手填代理时也照样按系统的排除列表绕开。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;
use tauri::AppHandle;

/// 启动时读到的系统代理（读一次，整个进程用它）。
static SYSTEM: OnceLock<SystemProxy> = OnceLock::new();

/// 系统设置里的代理：HTTPS 代理地址，和不走代理的那些地址。
#[derive(Default, Clone, Debug, PartialEq)]
struct SystemProxy {
    url: Option<String>,
    /// "不使用代理的主机与域"（`*.local`、`169.254/16`……）
    exceptions: Vec<String>,
    /// "排除简单主机名"（不带点的主机名直连）
    exclude_simple: bool,
}

#[derive(Serialize)]
pub struct ProxyInfo {
    /// 系统设置里的 HTTPS 代理（没开就是 null）
    system: Option<String>,
    /// 设置里手填的（没填就是 null）
    manual: Option<String>,
    /// 现在实际走的
    active: Option<String>,
    /// 系统设置里不走代理的地址（跟随系统代理时才有意义）；惊鸿眼下照样走代理，设置页据此提示
    exceptions: Vec<String>,
    /// 系统设置里勾了"排除简单主机名"
    exclude_simple: bool,
}

fn file(data: &Path) -> PathBuf {
    data.join("proxy.txt")
}

fn read_manual(data: &Path) -> Option<String> {
    std::fs::read_to_string(file(data)).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// `scutil --proxy` 的输出：HTTPS 代理（`HTTPSEnable : 1`、`HTTPSProxy : 主机`、`HTTPSPort : 端口`），
/// 加上排除列表（`ExceptionsList : <array> { 0 : *.local … }`）与 `ExcludeSimpleHostnames : 1`。没开 HTTPS 代理时排除列表也不要。
fn parse_scutil(text: &str) -> SystemProxy {
    let Some(url) = parse_url(text) else { return SystemProxy::default() };
    let mut exceptions = Vec::new();
    let mut lines = text.lines();
    while let Some(l) = lines.next() {
        if l.split_once(" : ").is_some_and(|(k, v)| k.trim() == "ExceptionsList" && v.trim_start().starts_with("<array>")) {
            for item in lines.by_ref() {
                let item = item.trim();
                if item.starts_with('}') {
                    break;
                }
                if let Some((_, v)) = item.split_once(" : ") {
                    let v = v.trim();
                    if !v.is_empty() {
                        exceptions.push(v.to_string());
                    }
                }
            }
            break;
        }
    }
    let exclude_simple = text.lines().any(|l| l.split_once(" : ").is_some_and(|(k, v)| k.trim() == "ExcludeSimpleHostnames" && v.trim() == "1"));
    SystemProxy { url: Some(url), exceptions, exclude_simple }
}

fn parse_url(text: &str) -> Option<String> {
    let get = |key: &str| {
        text.lines().find_map(|l| {
            let (k, v) = l.split_once(" : ")?;
            (k.trim() == key).then(|| v.trim().to_string())
        })
    };
    if get("HTTPSEnable").as_deref() != Some("1") {
        return None;
    }
    let host = get("HTTPSProxy").filter(|h| !h.is_empty())?;
    let port = get("HTTPSPort").and_then(|p| p.parse::<u16>().ok()).unwrap_or(8080);
    Some(format!("http://{host}:{port}"))
}

fn read_system() -> SystemProxy {
    std::process::Command::new("/usr/sbin/scutil")
        .arg("--proxy")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|out| parse_scutil(&String::from_utf8_lossy(&out.stdout)))
        .unwrap_or_default()
}

fn system() -> &'static SystemProxy {
    SYSTEM.get_or_init(read_system)
}

/// 手填的代理整理成 `http://主机:端口`；`主机:端口` 补上 `http://`。认不出来就说原因。
fn normalize(input: &str) -> Result<String, String> {
    let s = input.trim();
    let rest = match s.split_once("://") {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") => rest,
        Some((scheme, _)) => return Err(format!("unsupported:{scheme}")),
        None => s,
    };
    let rest = rest.trim_end_matches('/');
    let (host, port) = rest.rsplit_once(':').ok_or_else(|| "port".to_string())?;
    let port: u16 = port.parse().map_err(|_| "port".to_string())?;
    if host.is_empty() || host.contains('/') || host.contains(' ') || port == 0 {
        return Err("host".to_string());
    }
    Ok(format!("http://{host}:{port}"))
}

fn info(data: &Path) -> ProxyInfo {
    let s = system();
    ProxyInfo { system: s.url.clone(), manual: read_manual(data), active: fg_core::platform::proxy(), exceptions: s.exceptions.clone(), exclude_simple: s.exclude_simple }
}

/// 启动时调用（放在后台线程里，`scutil` 不拖慢开窗口）：读系统代理，手填的优先，交给核心。
pub fn init(data: PathBuf) {
    std::thread::spawn(move || {
        let s = system();
        fg_core::platform::set_proxy_bypass(s.exceptions.clone(), s.exclude_simple);
        let system = s.url.clone();
        let manual = read_manual(&data);
        // 先手填、后系统；两样都没有就直连
        fg_core::platform::set_proxy(manual.or(system));
    });
}

#[tauri::command]
pub fn proxy_info(app: AppHandle) -> ProxyInfo {
    info(&crate::data_dir(&app))
}

/// 手填代理：`manual` 为空或 null 就清掉（回到系统代理）；马上生效，并记到数据目录里下次启动还在。
#[tauri::command]
pub fn proxy_set(app: AppHandle, manual: Option<String>) -> Result<ProxyInfo, String> {
    let data = crate::data_dir(&app);
    let manual = match manual.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(normalize(s)?),
        None => None,
    };
    let _ = std::fs::create_dir_all(&data);
    match &manual {
        Some(m) => std::fs::write(file(&data), m).map_err(|e| e.to_string())?,
        None => {
            let _ = std::fs::remove_file(file(&data));
        }
    }
    fg_core::platform::set_proxy(manual.or_else(|| system().url.clone()));
    Ok(info(&data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_https_proxy_from_scutil() {
        let on = "<dictionary> {\n  HTTPEnable : 1\n  HTTPPort : 7890\n  HTTPProxy : 127.0.0.1\n  HTTPSEnable : 1\n  HTTPSPort : 7890\n  HTTPSProxy : 127.0.0.1\n  ProxyAutoConfigEnable : 0\n}\n";
        assert_eq!(parse_scutil(on).url.as_deref(), Some("http://127.0.0.1:7890"));
        assert!(parse_scutil(on).exceptions.is_empty());
        let off = "<dictionary> {\n  HTTPEnable : 0\n  HTTPSEnable : 0\n  ProxyAutoConfigEnable : 0\n  SOCKSEnable : 0\n}\n";
        assert_eq!(parse_scutil(off), SystemProxy::default());
        // 只开了 HTTP 代理：HTTPS 照系统的意思直连
        let http_only = "<dictionary> {\n  HTTPEnable : 1\n  HTTPPort : 8080\n  HTTPProxy : proxy.lan\n  HTTPSEnable : 0\n}\n";
        assert_eq!(parse_scutil(http_only).url, None);
    }

    #[test]
    fn reads_exceptions_from_scutil() {
        let text = "<dictionary> {\n  ExceptionsList : <array> {\n    0 : *.local\n    1 : 169.254/16\n    2 : 192.168.0.0/16\n  }\n  ExcludeSimpleHostnames : 1\n  HTTPSEnable : 1\n  HTTPSPort : 7890\n  HTTPSProxy : 127.0.0.1\n}\n";
        let s = parse_scutil(text);
        assert_eq!(s.url.as_deref(), Some("http://127.0.0.1:7890"));
        assert_eq!(s.exceptions, vec!["*.local", "169.254/16", "192.168.0.0/16"]);
        assert!(s.exclude_simple);
        // 没开 HTTPS 代理：排除列表没有意义
        let off = "<dictionary> {\n  ExceptionsList : <array> {\n    0 : *.local\n  }\n  HTTPSEnable : 0\n}\n";
        assert_eq!(parse_scutil(off), SystemProxy::default());
    }

    #[test]
    fn normalizes_manual_proxy() {
        assert_eq!(normalize("127.0.0.1:7890").unwrap(), "http://127.0.0.1:7890");
        assert_eq!(normalize(" http://proxy.lan:8080/ ").unwrap(), "http://proxy.lan:8080");
        assert_eq!(normalize("https://proxy.lan:443").unwrap(), "http://proxy.lan:443");
        assert!(normalize("socks5://127.0.0.1:1080").unwrap_err().starts_with("unsupported:"));
        assert_eq!(normalize("proxy.lan").unwrap_err(), "port");
        assert_eq!(normalize("proxy.lan:abc").unwrap_err(), "port");
        assert_eq!(normalize(":8080").unwrap_err(), "host");
    }
}
