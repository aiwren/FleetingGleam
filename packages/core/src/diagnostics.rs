//! 本机的诊断记录：`platform::log` 的每一行同时追加到 `<数据目录>/logs/app.log`（满 2 MB 换到 `app.log.1`，只留这两份）；
//! 核心崩溃（panic）写 `logs/crash-<时间>.txt`（最多留 5 份）。Android 的 Kotlin 未捕获异常与 `ApplicationExitInfo` 也写进这个目录
//! （`crash-kotlin-<时间>.txt` / `exit-<时间>.txt`，由 Kotlin 自己写）。
//!
//! "导出诊断信息"把版本、系统、设置、最近的日志和全部崩溃记录写成一个文本文件，用户自己决定发不发。纯本地，不联网。
//! 写出去之前把一切密码、令牌、NAS 用户名抹掉：加密仓库里存的每一个秘密、备份目标的 Access Key、地址里的 `用户:密码@`。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// `app.log` 满这么大就换下一份。
const LOG_MAX: u64 = 2 * 1024 * 1024;
/// 核心的崩溃记录最多留几份。
const CRASH_KEEP: usize = 5;

static LOG_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
static HOOK: std::sync::Once = std::sync::Once::new();

/// 引擎初始化时调：日志写到 `<data>/logs`，装上 panic hook（一个进程只装一次，之前的 hook 照样调用）。
pub fn init(data: &Path) {
    let dir = logs_dir(data);
    let _ = std::fs::create_dir_all(&dir);
    *LOG_DIR.lock().unwrap_or_else(|e| e.into_inner()) = Some(dir);
    HOOK.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            // 不等锁：panic 可能就发生在拿着日志锁写 app.log 的时候，这里再等就卡死了
            let dir = match LOG_DIR.try_lock() {
                Ok(g) => g.clone(),
                Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner().clone(),
                Err(std::sync::TryLockError::WouldBlock) => None,
            };
            if let Some(dir) = dir {
                let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
                let bt = std::backtrace::Backtrace::force_capture();
                let _ = write_crash(&dir, &format!("FleetingGleam {} 崩溃（panic）\n时间：{} UTC\n线程：{thread}\n{info}\n\n调用栈：\n{bt}\n", crate::VERSION, stamp(now()).0));
            }
            prev(info);
        }));
    });
}

/// 日志目录：`<data>/logs`。
pub fn logs_dir(data: &Path) -> PathBuf {
    data.join("logs")
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Unix 秒 → (`2026-10-26 12:34:56`, `20261026-123456`)，UTC。
fn stamp(secs: u64) -> (String, String) {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    // 从 1970 年起的天数算成年月日（Howard Hinnant 的算法）
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    let (hh, mm, ss) = (rem / 3600, rem / 60 % 60, rem % 60);
    (format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02}"), format!("{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}"))
}

/// `platform::log` 的每一行：带时间追加到 `app.log`，满了换一份。还没初始化（没有引擎）就不写。
pub(crate) fn append(msg: &str) {
    let guard = LOG_DIR.lock().unwrap_or_else(|e| e.into_inner());
    let Some(dir) = guard.as_ref() else { return };
    let path = dir.join("app.log");
    if std::fs::metadata(&path).map_or(false, |m| m.len() >= LOG_MAX) {
        let _ = std::fs::rename(&path, dir.join("app.log.1"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{} {msg}", stamp(now()).0);
    }
}

/// 写一份崩溃记录，只留最近的 `CRASH_KEEP` 份（Kotlin 写的 `crash-kotlin-*` 不算在内，也不删）。
fn write_crash(dir: &Path, text: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let mut path = dir.join(format!("crash-{}.txt", stamp(now()).1));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("crash-{}-{n}.txt", stamp(now()).1));
        n += 1;
    }
    std::fs::write(&path, text)?;
    let mut mine: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.starts_with("crash-") && !n.starts_with("crash-kotlin-")
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    mine.sort();
    let extra = mine.len().saturating_sub(CRASH_KEEP);
    for (_, p) in mine.into_iter().take(extra) {
        let _ = std::fs::remove_file(p);
    }
    Ok(path)
}

/// 抹掉地址里的 `用户:密码@`（代理、smb://、https://）。
fn strip_userinfo(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("://") {
        out.push_str(&rest[..i + 3]);
        rest = &rest[i + 3..];
        // 地址到第一个空白、/、引号为止；里面有 @ 就是带了账号
        let end = rest.find(|c: char| c.is_whitespace() || c == '/' || c == '"' || c == '\'').unwrap_or(rest.len());
        if let Some(at) = rest[..end].rfind('@') {
            out.push_str("***@");
            rest = &rest[at + 1..];
        }
    }
    out.push_str(rest);
    out
}

/// 把要写出去的文字里的敏感内容抹掉：`secrets` 里每一项（密码、令牌）、`also`（NAS 用户名、Access Key）、地址里的账号。
pub fn redact(text: &str, secrets: &[String]) -> String {
    let mut t = strip_userinfo(text);
    let mut words: Vec<&String> = secrets.iter().filter(|s| s.chars().count() >= 3).collect();
    // 长的先换：一个秘密包含另一个时不留半截
    words.sort_by_key(|s| std::cmp::Reverse(s.len()));
    for w in words {
        t = t.replace(w.as_str(), "***");
    }
    t
}

/// 诊断文件的内容：`header`（版本、系统、设置，调用方已去掉敏感字段）+ 最近的 app.log + 全部崩溃 / 退出记录，再整体抹一遍 `secrets`。
pub fn report(logs: &Path, header: &str, secrets: &[String]) -> String {
    let mut text = String::new();
    text.push_str(header);
    for name in ["app.log.1", "app.log"] {
        if let Ok(b) = std::fs::read(logs.join(name)) {
            text.push_str(&format!("\n\n===== {name} =====\n"));
            text.push_str(&String::from_utf8_lossy(&b));
        }
    }
    let mut records: Vec<PathBuf> = std::fs::read_dir(logs)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.file_name().map_or(false, |n| { let n = n.to_string_lossy(); (n.starts_with("crash-") || n.starts_with("exit-")) && n.ends_with(".txt") })).collect())
        .unwrap_or_default();
    records.sort();
    for p in records {
        if let Ok(b) = std::fs::read(&p) {
            text.push_str(&format!("\n\n===== {} =====\n", p.file_name().unwrap_or_default().to_string_lossy()));
            text.push_str(&String::from_utf8_lossy(&b));
        }
    }
    redact(&text, secrets)
}

/// 这台机器的系统：内核名、版本、硬件（`uname`）。
pub fn os_info() -> String {
    #[cfg(unix)]
    unsafe {
        let mut u: libc::utsname = std::mem::zeroed();
        if libc::uname(&mut u) == 0 {
            let f = |b: &[libc::c_char]| std::ffi::CStr::from_ptr(b.as_ptr()).to_string_lossy().into_owned();
            return format!("{} {} {}", f(&u.sysname), f(&u.release), f(&u.machine));
        }
    }
    std::env::consts::OS.to_string()
}

/// 生成时间（给诊断文件的抬头）。
pub fn generated_at() -> String {
    format!("{} UTC", stamp(now()).0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_like_dates() {
        assert_eq!(stamp(0).0, "1970-01-01 00:00:00");
        // 2026-10-26 12:34:56 UTC
        assert_eq!(stamp(1_793_018_096), ("2026-10-26 12:34:56".to_string(), "20261026-123456".to_string()));
    }

    #[test]
    fn accounts_in_addresses_and_known_secrets_are_wiped() {
        let t = "proxy http://bob:pa55@proxy.lan:8080 ignored; smb://alice@nas.local/photos; pw=hunter22 ok; https://example.com/x";
        let r = redact(t, &["hunter22".to_string(), "x".to_string()]);
        assert!(!r.contains("bob") && !r.contains("pa55") && !r.contains("alice@") && !r.contains("hunter22"), "{r}");
        assert!(r.contains("http://***@proxy.lan:8080") && r.contains("smb://***@nas.local/photos") && r.contains("https://example.com/x"), "{r}");
    }

    #[test]
    fn only_the_latest_crashes_are_kept() {
        let dir = std::env::temp_dir().join(format!("fg-crash-keep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("crash-kotlin-20261001-000000.txt"), "kotlin").unwrap();
        for i in 0..8 {
            write_crash(&dir, &format!("crash {i}")).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        let names: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names.iter().filter(|n| n.starts_with("crash-") && !n.starts_with("crash-kotlin-")).count(), CRASH_KEEP);
        assert!(names.iter().any(|n| n.starts_with("crash-kotlin-")), "Kotlin 的不删");
        let all: String = names.iter().map(|n| std::fs::read_to_string(dir.join(n)).unwrap()).collect();
        assert!(all.contains("crash 7") && !all.contains("crash 0"), "留的是最近的");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
