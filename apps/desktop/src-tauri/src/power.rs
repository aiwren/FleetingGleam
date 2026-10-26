//! 分析期间别让 Mac 睡着、别被 App Nap 降速：`NSProcessInfo.beginActivity(.userInitiated)`（其中含 `.idleSystemSleepDisabled`），
//! 结束时 `endActivity`。用 `Awake` 守着：分析正常结束、被取消、出错，守卫一离开作用域就释放。
//! 系统那边是一个电源断言，`pmset -g assertions` 里能看到（PreventUserIdleSystemSleep，名字是 `reason`）。

/// 活动令牌（`NSObject *`，已 retain）；0 = 没拿到（不是 macOS，或系统没给）。
pub struct Awake(usize);

#[cfg(target_os = "macos")]
mod ffi {
    use std::ffi::{c_char, c_void};

    pub type Id = *mut c_void;
    pub type Sel = *const c_void;

    #[link(name = "objc")]
    extern "C" {
        pub fn objc_getClass(name: *const c_char) -> Id;
        pub fn sel_registerName(name: *const c_char) -> Sel;
        pub fn objc_msgSend();
        pub fn objc_autoreleasePoolPush() -> *mut c_void;
        pub fn objc_autoreleasePoolPop(pool: *mut c_void);
    }

    // Foundation 已经由 build.rs 链接进来
    /// NSActivityUserInitiated：0x00FFFFFF，含 NSActivityIdleSystemSleepDisabled（1 << 20）
    pub const USER_INITIATED: u64 = 0x00FF_FFFF;

    pub unsafe fn sel(name: &str) -> Sel {
        let c = std::ffi::CString::new(name).unwrap();
        sel_registerName(c.as_ptr())
    }

    pub unsafe fn class(name: &str) -> Id {
        let c = std::ffi::CString::new(name).unwrap();
        objc_getClass(c.as_ptr())
    }

    /// `objc_msgSend` 按每种签名单独转换（arm64 上必须这样调用）。
    pub unsafe fn send0(obj: Id, s: Sel) -> Id {
        let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(obj, s)
    }

    pub unsafe fn send_ptr(obj: Id, s: Sel, a: *const c_void) -> Id {
        let f: unsafe extern "C" fn(Id, Sel, *const c_void) -> Id = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(obj, s, a)
    }

    pub unsafe fn send_u64_ptr(obj: Id, s: Sel, a: u64, b: *const c_void) -> Id {
        let f: unsafe extern "C" fn(Id, Sel, u64, *const c_void) -> Id = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(obj, s, a, b)
    }
}

impl Awake {
    /// 开始一段"用户在等结果"的活动：系统空闲时不睡眠，应用被遮挡时不进 App Nap。
    pub fn begin(reason: &str) -> Awake {
        #[cfg(target_os = "macos")]
        unsafe {
            use ffi::*;
            let pool = objc_autoreleasePoolPush();
            let info = send0(class("NSProcessInfo"), sel("processInfo"));
            let text = std::ffi::CString::new(reason).unwrap_or_default();
            let ns = send_ptr(class("NSString"), sel("stringWithUTF8String:"), text.as_ptr() as *const _);
            let token = if info.is_null() || ns.is_null() { std::ptr::null_mut() } else { send_u64_ptr(info, sel("beginActivityWithOptions:reason:"), USER_INITIATED, ns) };
            // 令牌是自动释放的：留着它直到 end
            if !token.is_null() {
                send0(token, sel("retain"));
            }
            objc_autoreleasePoolPop(pool);
            return Awake(token as usize);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = reason;
            Awake(0)
        }
    }
}

impl Drop for Awake {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        if self.0 != 0 {
            unsafe {
                use ffi::*;
                let pool = objc_autoreleasePoolPush();
                let token = self.0 as Id;
                let info = send0(class("NSProcessInfo"), sel("processInfo"));
                if !info.is_null() {
                    send_ptr(info, sel("endActivity:"), token as *const _);
                }
                send0(token, sel("release"));
                objc_autoreleasePoolPop(pool);
            }
        }
    }
}

// 令牌只是一个对象指针，begin / end 在哪个线程调都行（NSProcessInfo 是线程安全的）
unsafe impl Send for Awake {}
unsafe impl Sync for Awake {}

#[cfg(test)]
mod tests {
    use super::*;

    /// 拿得到令牌，释放不崩；拿着的时候 `pmset -g assertions` 里有这一条。
    #[test]
    fn begin_and_end() {
        let a = Awake::begin("FleetingGleam test");
        #[cfg(target_os = "macos")]
        {
            assert_ne!(a.0, 0);
            let out = std::process::Command::new("pmset").args(["-g", "assertions"]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
            assert!(out.contains("FleetingGleam test"), "{out}");
        }
        drop(a);
        #[cfg(target_os = "macos")]
        {
            let out = std::process::Command::new("pmset").args(["-g", "assertions"]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
            assert!(!out.contains("FleetingGleam test"), "{out}");
        }
    }
}
