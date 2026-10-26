//! 用户可能碰到的错误按前缀分类（三端约定，见审查第 26、33、35 条）：界面按前缀给中文说明，冒号后面是细节，可以放进"详情"。
//!
//! | 前缀 | 含义 |
//! |---|---|
//! | `space:<需要字节>:<可用字节>` | 磁盘空间不够 |
//! | `verify:<细节>` | 下载完校验失败 |
//! | `net:<细节>` | 网络问题（超时、DNS、TLS、代理） |
//! | `auth:<细节>` | NAS 用户名或密码不对 |
//! | `denied:<细节>` | 没权限（NAS 共享、目录） |
//! | `cancelled` | 用户取消 |

use std::path::Path;

pub const PREFIXES: [&str; 5] = ["space:", "verify:", "net:", "auth:", "denied:"];
pub const CANCELLED: &str = "cancelled";

/// 这条错误已经带了约定的前缀（或者就是 `cancelled`）。
pub fn classified(e: &str) -> bool {
    e == CANCELLED || PREFIXES.iter().any(|p| e.starts_with(p))
}

/// 磁盘空间不够：还需要多少、还有多少。
pub fn space(need: u64, avail: u64) -> String {
    format!("space:{need}:{avail}")
}

/// 给错误加上下文（哪台 NAS、哪个文件），前缀仍在最前面：`auth:` + `家里的 NAS: wrong password`。没有前缀的照旧 `上下文: 错误`。
pub fn context(ctx: &str, e: &str) -> String {
    if e == CANCELLED {
        return e.to_string();
    }
    match PREFIXES.iter().find(|p| e.starts_with(*p)) {
        Some(p) => format!("{p}{ctx}: {}", &e[p.len()..]),
        None => format!("{ctx}: {e}"),
    }
}

/// 写文件出的错：磁盘满说成 `space:`（需要 `need` 字节，可用的现查），没权限说成 `denied:`，别的原样。
pub fn io(e: &std::io::Error, at: &Path, need: u64) -> String {
    if e.kind() == std::io::ErrorKind::StorageFull || e.raw_os_error() == Some(28) {
        return space(need, free_space(at).unwrap_or(0));
    }
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        return format!("denied:{}: {e}", at.display());
    }
    format!("{}: {e}", at.display())
}

/// `path` 所在的卷还有多少可用字节（普通用户能用的那部分）。`path` 还不存在时看它最近的已存在的上级目录；查不出来是 None。
pub fn free_space(path: &Path) -> Option<u64> {
    let mut p = path;
    while !p.exists() {
        p = p.parent()?;
    }
    free_space_of(p)
}

#[cfg(unix)]
fn free_space_of(p: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(p.as_os_str().as_bytes()).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return None;
    }
    #[allow(clippy::unnecessary_cast)]
    Some(st.f_bavail as u64 * st.f_frsize as u64)
}

#[cfg(not(unix))]
fn free_space_of(_: &Path) -> Option<u64> {
    None
}

/// 要写 `need` 字节到 `dir`：可用空间（查得出来时）至少要 `need`，否则返回 `space:` 错误。
pub fn check_space(dir: &Path, need: u64) -> Result<(), String> {
    match free_space(dir) {
        Some(avail) if avail < need => Err(space(need, avail)),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prefix_stays_in_front_of_the_context() {
        assert_eq!(context("家里的 NAS", "auth:wrong user name or password"), "auth:家里的 NAS: wrong user name or password");
        assert_eq!(context("家里的 NAS", "something odd"), "家里的 NAS: something odd");
        assert_eq!(context("x", "cancelled"), "cancelled");
        assert!(classified("space:10:2") && classified("cancelled") && !classified("oops"));
    }

    #[test]
    fn free_space_is_known_here_and_checked() {
        let tmp = std::env::temp_dir();
        let free = free_space(&tmp.join("does/not/exist/yet")).expect("the temp folder's volume");
        assert!(free > 0);
        assert!(check_space(&tmp, 1).is_ok());
        let e = check_space(&tmp, u64::MAX).unwrap_err();
        assert!(e.starts_with(&format!("space:{}:", u64::MAX)), "{e}");
    }

    #[test]
    fn a_full_disk_and_a_refusal_are_told_apart() {
        let full = std::io::Error::from_raw_os_error(28);
        assert!(io(&full, Path::new("/tmp/x.jpg"), 100).starts_with("space:100:"));
        let no = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert!(io(&no, Path::new("/tmp/x.jpg"), 100).starts_with("denied:"));
    }
}
