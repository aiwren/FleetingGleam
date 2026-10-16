//! The native HDR layer of the full-size viewer. The system web view draws HDR photos as plain SDR, so the viewer puts the photo in a
//! Core Animation layer that may go beyond SDR white (see native/overlay.swift) over the web view, where the page says the photo is.
//! Only macOS has it; elsewhere these commands report "not supported" and the viewer shows the photo as the web view draws it.

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, Copy)]
pub struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Where the photo is and what is done with it: `frame` is the box it sits in without zoom, `clip` what may be seen of it (page coordinates, points),
/// `scale`, `x` and `y` the zoom and the pan (from the box's centre). `seq` numbers the views the page makes: the native layer, which also moves
/// the photo by hand (wheel, pinch, drag, double-click), says in its events which of the page's views it has seen, so that the page can tell
/// an answer to its latest view from a stale one. `nav` is the size of the map of the whole photo, shown in the corner when zoomed in.
#[derive(Deserialize)]
pub struct View {
    #[serde(rename = "box")]
    frame: Rect,
    clip: Rect,
    scale: f64,
    x: f64,
    y: f64,
    seq: f64,
    nav: Option<Size>,
    /// Glide to this view instead of jumping (the page's own discrete jumps: keys, reset).
    #[serde(default)]
    glide: bool,
}

#[derive(Deserialize, Clone, Copy)]
pub struct Size {
    w: f64,
    h: f64,
}

impl View {
    /// The numbers in the order `fg_hdr_show` / `fg_hdr_move` read them (see `takeFrame` in overlay.swift).
    fn pack(&self) -> [f64; 15] {
        let (f, c) = (self.frame, self.clip);
        let (nw, nh) = self.nav.map_or((0.0, 0.0), |n| (n.w, n.h));
        [f.x, f.y, f.w, f.h, c.x, c.y, c.w, c.h, self.scale, self.x, self.y, self.seq, nw, nh, if self.glide { 1.0 } else { 0.0 }]
    }
}

/// What the native layer reports when the view was changed by hand.
#[derive(Serialize, Clone)]
struct ViewEvent {
    scale: f64,
    x: f64,
    y: f64,
    /// The `seq` of the last view of the page that the native layer had seen when this happened.
    ack: f64,
    /// A mouse press over the photo (the page closes its open menus).
    press: bool,
}

static APP: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

#[cfg(target_os = "macos")]
mod native {
    use std::ffi::{c_char, c_void, CString};

    extern "C" {
        fn fg_hdr_supported(win: *mut c_void) -> i32;
        fn fg_hdr_prepare(path: *const c_char, require_hdr: i32) -> i32;
        fn fg_hdr_detail_render(path: *const c_char, fx: f64, fy: f64, fw: f64, fh: f64, out_w: i32) -> i32;
        fn fg_hdr_detail_show(fx: f64, fy: f64, fw: f64, fh: f64);
        fn fg_hdr_detail_hide();
        fn fg_hdr_show(win: *mut c_void, path: *const c_char, v: *const f64);
        fn fg_hdr_move(win: *mut c_void, v: *const f64);
        fn fg_hdr_hide(win: *mut c_void);
        fn fg_hdr_release();
        fn fg_hdr_set_callback(cb: Option<extern "C" fn(f64, f64, f64, i64, i32)>);
    }

    use super::Rect;
    use tauri::{Emitter, WebviewWindow};

    /// Called on the main thread by the native layer when the photo was moved by hand.
    extern "C" fn on_view(scale: f64, x: f64, y: f64, ack: i64, press: i32) {
        if let Some(app) = super::APP.get() {
            let _ = app.emit("hdr-view", super::ViewEvent { scale, x, y, ack: ack as f64, press: press != 0 });
        }
    }

    /// Runs `f` with the window's NSWindow on the main thread and waits for its answer.
    fn on_main<T: Send + 'static>(window: &WebviewWindow, f: impl FnOnce(*mut c_void) -> T + Send + 'static) -> Option<T> {
        let ptr = window.ns_window().ok()? as usize;
        let (tx, rx) = std::sync::mpsc::channel();
        window.run_on_main_thread(move || { let _ = tx.send(f(ptr as *mut c_void)); }).ok()?;
        rx.recv_timeout(std::time::Duration::from_secs(5)).ok()
    }

    /// Same, without waiting (position updates while zooming must never block).
    fn on_main_nowait(window: &WebviewWindow, f: impl FnOnce(*mut c_void) + Send + 'static) {
        if let Ok(ptr) = window.ns_window() {
            let ptr = ptr as usize;
            let _ = window.run_on_main_thread(move || f(ptr as *mut c_void));
        }
    }

    pub fn supported(window: &WebviewWindow) -> bool {
        on_main(window, |w| unsafe { fg_hdr_supported(w) } == 1).unwrap_or(false)
    }
    pub fn prepare(path: &str, require_hdr: bool) -> bool {
        CString::new(path).map_or(false, |c| unsafe { fg_hdr_prepare(c.as_ptr(), require_hdr as i32) } == 1)
    }
    /// Decodes a region (fractions of the photo) at up to `out_w` pixels wide, ready for `detail_show`.
    pub fn detail_render(path: &str, r: Rect, out_w: i32) -> bool {
        CString::new(path).map_or(false, |c| unsafe { fg_hdr_detail_render(c.as_ptr(), r.x, r.y, r.w, r.h, out_w) } == 1)
    }
    pub fn detail_show(window: &WebviewWindow, r: Rect) {
        let _ = on_main(window, move |_| unsafe { fg_hdr_detail_show(r.x, r.y, r.w, r.h) });
    }
    pub fn detail_hide(window: &WebviewWindow) {
        on_main_nowait(window, |_| unsafe { fg_hdr_detail_hide() });
    }
    pub fn show(window: &WebviewWindow, path: &str, v: [f64; 15]) {
        let Ok(p) = CString::new(path) else { return };
        let _ = super::APP.set(tauri::Manager::app_handle(window).clone());
        unsafe { fg_hdr_set_callback(Some(on_view)) };
        let _ = on_main(window, move |w| unsafe { fg_hdr_show(w, p.as_ptr(), v.as_ptr()) });
    }
    /// Moves the layer (and with it the map and the sharp part); the view in it counts only when it is a new one.
    pub fn mv(window: &WebviewWindow, v: [f64; 15]) {
        on_main_nowait(window, move |w| unsafe { fg_hdr_move(w, v.as_ptr()) });
    }
    pub fn hide(window: &WebviewWindow, release: bool) {
        on_main_nowait(window, move |w| unsafe { fg_hdr_hide(w); if release { fg_hdr_release(); } });
    }
}

#[cfg(not(target_os = "macos"))]
mod native {
    use super::Rect;
    use tauri::WebviewWindow;
    pub fn supported(_: &WebviewWindow) -> bool { false }
    pub fn prepare(_: &str, _: bool) -> bool { false }
    pub fn detail_render(_: &str, _: Rect, _: i32) -> bool { false }
    pub fn detail_show(_: &WebviewWindow, _: Rect) {}
    pub fn detail_hide(_: &WebviewWindow) {}
    pub fn show(_: &WebviewWindow, _: &str, _: [f64; 15]) {}
    pub fn mv(_: &WebviewWindow, _: [f64; 15]) {}
    pub fn hide(_: &WebviewWindow, _: bool) {}
}

/// Whether the window is on a display that can show HDR now.
#[tauri::command]
pub async fn hdr_overlay_supported(window: tauri::WebviewWindow) -> bool {
    native::supported(&window)
}

/// Shows the photo in HDR over the page, where `view` says (page coordinates).
/// With `require_hdr`, answers false when the photo has no HDR to show (the page keeps drawing it); without, any photo is shown.
#[tauri::command]
pub async fn hdr_overlay_show(window: tauri::WebviewWindow, path: String, require_hdr: bool, view: View) -> Result<bool, String> {
    let p = path.clone();
    let ready = tauri::async_runtime::spawn_blocking(move || native::prepare(&p, require_hdr)).await.map_err(|e| e.to_string())?;
    if !ready {
        return Ok(false);
    }
    native::show(&window, &path, view.pack());
    Ok(true)
}

#[tauri::command]
pub fn hdr_overlay_move(window: tauri::WebviewWindow, view: View) {
    native::mv(&window, view.pack());
}

/// Takes the layer away; `release` also frees the decoded photos (when the viewer closes).
#[tauri::command]
pub fn hdr_overlay_hide(window: tauri::WebviewWindow, release: Option<bool>) {
    DETAIL_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    native::hide(&window, release.unwrap_or(false));
}

/// The newest request for the sharp part wins: while the photo is being zoomed and panned requests pile up, and an older one is dropped as soon
/// as a newer one exists (before it starts, and again before it is shown). Decoding and showing happen one request at a time.
static DETAIL_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static DETAIL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Draws the sharp, full-resolution part of the photo over the layer: `region` is what is visible (fractions of the photo, top-left origin),
/// `out_w` how many pixels wide to decode it. The layer keeps it in place as the photo moves.
#[tauri::command]
pub async fn hdr_overlay_detail(window: tauri::WebviewWindow, path: String, region: Rect, out_w: i32) -> Result<bool, String> {
    use std::sync::atomic::Ordering;
    let me = DETAIL_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
    tauri::async_runtime::spawn_blocking(move || {
        let _one_at_a_time = DETAIL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if DETAIL_SEQ.load(Ordering::SeqCst) != me {
            return false;
        }
        if !native::detail_render(&path, region, out_w) || DETAIL_SEQ.load(Ordering::SeqCst) != me {
            return false;
        }
        native::detail_show(&window, region);
        true
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn hdr_overlay_detail_hide(window: tauri::WebviewWindow) {
    DETAIL_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);   // whatever is still being decoded is no longer wanted
    native::detail_hide(&window);
}
