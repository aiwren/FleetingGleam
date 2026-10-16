// A native HDR layer over the web view, for the full-size viewer.
//
// The system web view (WKWebView) draws HDR photos as plain SDR, whatever the file or format, so a photo shot in HDR would be judged without the
// brightness it was shot with. This puts the photo in a Core Animation layer that is allowed to go beyond SDR white (extended dynamic range),
// exactly the way the Photos app shows it, on top of the web view and clipped to the viewer's stage. The page tells us where the photo's box is
// and what view (zoom and pan) it has; this layer also takes the wheel, pinch, drag and double-click over the photo itself, so that moving the
// photo never waits for a round trip through the page: the picture follows the hand in the same turn of the run loop, and the page is told
// afterwards (`fg_hdr_set_callback`) so that its own state (the percentage, the sharp crops) follows.
//
// Everything here runs on the main thread except `fg_hdr_prepare`, which decodes and may be called from any thread.
import AppKit
import ImageIO
import QuartzCore
import CoreImage

/// The view that holds the layers. It takes the mouse over the photo (the page's step arrows, menus and bars are outside its frame, and the
/// page makes the frame give way to the Live menu while that is open) and hands it to the gesture functions below.
final class GestureView: NSView {
    override var isOpaque: Bool { false }
    override var acceptsFirstResponder: Bool { false }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { (!isHidden && frame.contains(point)) ? self : nil }
    override func scrollWheel(with e: NSEvent) { gestureScroll(e) }
    override func magnify(with e: NSEvent) { gestureMagnify(e) }
    override func smartMagnify(with e: NSEvent) { gestureToggle(about: pagePoint(e)) }
    override func mouseDown(with e: NSEvent) { gestureDown(e) }
    override func mouseDragged(with e: NSEvent) { gestureDrag(e) }
    override func mouseUp(with e: NSEvent) { gestureUp(e) }
    override func resetCursorRects() {
        let s = State.shared
        let zoomed = s.scale > 1.02
        addCursorRect(bounds, cursor: zoomed ? (s.drag != nil ? .closedHand : .openHand) : .arrow)
        if zoomed, let nr = s.navRect {
            addCursorRect(NSRect(x: nr.minX - s.lastClipR.minX, y: s.lastClipR.maxY - nr.maxY, width: nr.width, height: nr.height), cursor: .crosshair)
        }
    }
}

public typealias ViewCallback = @convention(c) (Double, Double, Double, Int64, Int32) -> Void

final class State {
    static let shared = State()
    let lock = NSLock()
    var images: [String: CGImage] = [:]   // decoded photos, newest last (at most two)
    var order: [String] = []
    var host: GestureView?
    weak var win: NSWindow?
    let layer = CALayer()
    var shown: String?
    // The sharp part: while zoomed, the visible region of the photo decoded at (nearly) its own resolution, drawn over the whole-photo layer.
    let detail = CALayer()
    var detailFrac: CGRect?          // that region as fractions of the photo, top-left origin
    var pendingDetail: CGImage?
    var source: (path: String, image: CIImage)?
    // The navigator (a map of the whole photo with a frame for what is on screen), drawn here because this layer covers the page. The page keeps
    // the mouse handling: it puts an invisible area on the same spot.
    let navBack = CALayer()
    let navDim = CAShapeLayer()
    let navFrame = CAShapeLayer()
    var navRect: NSRect?             // page coordinates
    var navSize: CGSize?             // how big the page wants the map (nil: none)
    var lastImgR = NSRect.zero
    var lastClipR = NSRect.zero
    // The view. `box` is where the photo sits without zoom (page coordinates, top-left origin, points); the photo is that box scaled by `scale`
    // about its centre and moved by (panX, panY). `seq` is the number of the last view the page sent: the page tells ours from its own by it.
    var box = NSRect.zero
    var scale = 1.0
    var panX = 0.0
    var panY = 0.0
    var seq: Int64 = 0
    var emit: ViewCallback?
    var lastEmit: CFTimeInterval = 0
    var finalTimer: Timer?
    var anim: Timer?
    var drag: (x: Double, y: Double, px: Double, py: Double)?
    var dragNav = false
    var cursorKey = ""
    let ciContext = CIContext(options: [.workingFormat: CIFormat.RGBAh])
}

/// The photo as a Core Image (HDR gain map applied, upright), kept for the last photo: decoding a 200 MP JPEG takes a second or so, once; after
/// that any region of it comes out in well under a tenth of a second.
func source(_ path: String) -> CIImage? {
    let s = State.shared
    s.lock.lock()
    if let c = s.source, c.path == path { s.lock.unlock(); return c.image }
    s.lock.unlock()
    guard let ci = CIImage(contentsOf: URL(fileURLWithPath: path), options: [.applyOrientationProperty: true, .expandToHDR: true]) else { return nil }
    s.lock.lock(); s.source = (path, ci); s.lock.unlock()
    return ci
}

func pixels(_ path: String) -> Int {
    guard let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary),
          let p = CGImageSourceCopyPropertiesAtIndex(src, 0, nil) as? [CFString: Any] else { return 0 }
    return ((p[kCGImagePropertyPixelWidth] as? Int) ?? 0) * ((p[kCGImagePropertyPixelHeight] as? Int) ?? 0)
}

/// Photos this large (200 MP and the like) get a modest picture of the whole photo, then regions of it at full detail from Core Image.
private let bigPixels = 60_000_000

/// Reads the whole photo into the Core Image source's cache (a 200 MP JPEG is decoded once, which takes about a second): asking for the very last
/// rows of the photo cannot be answered without decoding all of it. Done in the background while the first, smaller picture is being made, so that
/// by the time someone zooms in, every region comes out almost at once.
func warm(_ path: String) {
    guard let ci = source(path) else { return }
    let r = CGRect(x: ci.extent.minX, y: ci.extent.minY, width: ci.extent.width, height: 8)
    let strip = ci.cropped(to: r).transformed(by: CGAffineTransform(translationX: -r.minX, y: -r.minY)).transformed(by: CGAffineTransform(scaleX: 0.02, y: 0.02))
    _ = State.shared.ciContext.createCGImage(strip, from: CGRect(x: 0, y: 0, width: max(1, r.width * 0.02), height: 1), format: .RGBAh, colorSpace: CGColorSpace(name: CGColorSpace.extendedLinearDisplayP3))
}

func decode(_ path: String) -> CGImage? {
    // Very large photos: a modest picture of the whole photo from the system decoder (what is on screen until the sharp regions arrive), while the
    // photo is read into the Core Image cache in the background.
    let big = pixels(path) >= bigPixels
    if big { DispatchQueue.global(qos: .userInitiated).async { warm(path) } }
    guard let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary) else { return nil }
    let p = CGImageSourceCopyPropertiesAtIndex(src, 0, nil) as? [CFString: Any] ?? [:]
    let side = min(big ? 2048 : 4096, max((p[kCGImagePropertyPixelWidth] as? Int) ?? 4096, (p[kCGImagePropertyPixelHeight] as? Int) ?? 4096))
    let o: [CFString: Any] = [
        kCGImageSourceCreateThumbnailFromImageAlways: true,
        kCGImageSourceCreateThumbnailWithTransform: true,
        kCGImageSourceThumbnailMaxPixelSize: side,
        kCGImageSourceDecodeRequest: kCGImageSourceDecodeToHDR,
    ]
    return CGImageSourceCreateThumbnailAtIndex(src, 0, o as CFDictionary)
}

func window(_ ptr: UnsafeMutableRawPointer) -> NSWindow {
    Unmanaged<NSWindow>.fromOpaque(ptr).takeUnretainedValue()
}

/// 1 when the window is on a display that can show more than SDR white right now.
@_cdecl("fg_hdr_supported")
public func fg_hdr_supported(_ win: UnsafeMutableRawPointer) -> Int32 {
    let w = window(win)
    return (w.screen?.maximumPotentialExtendedDynamicRangeColorComponentValue ?? 1.0) > 1.05 ? 1 : 0
}

/// Decodes the photo in HDR and keeps it ready. Returns 1 when it can be shown. With `requireHDR` that means it really has headroom above SDR
/// (otherwise 0 and nothing is kept); without it any photo will do, for the very large ones that the web view cannot show at full detail.
@_cdecl("fg_hdr_prepare")
public func fg_hdr_prepare(_ cpath: UnsafePointer<CChar>, _ requireHDR: Int32) -> Int32 {
    let path = String(cString: cpath)
    let s = State.shared
    s.lock.lock()
    let have = s.images[path] != nil
    s.lock.unlock()
    if have { return 1 }
    guard let img = decode(path), requireHDR == 0 || img.contentHeadroom > 1.0 || pixels(path) >= bigPixels else { return 0 }
    s.lock.lock()
    s.images[path] = img
    s.order.removeAll { $0 == path }
    s.order.append(path)
    while s.order.count > 2 { s.images[s.order.removeFirst()] = nil }
    s.lock.unlock()
    return 1
}

/// The view's numbers as the page sends them: the photo's box, the clip, the zoom and pan, which view this is, the size of the map, and whether
/// to glide to it.
///   [0..3] box x, y, w, h   [4..7] clip x, y, w, h   [8] scale   [9] panX   [10] panY   [11] seq   [12..13] map w, h (0: none)   [14] glide
func takeFrame(_ w: NSWindow, _ v: UnsafePointer<Double>, adopt: Bool) {
    let s = State.shared
    s.win = w
    s.box = NSRect(x: v[0], y: v[1], width: v[2], height: v[3])
    s.lastClipR = NSRect(x: v[4], y: v[5], width: v[6], height: v[7])
    s.navSize = v[12] > 0 ? CGSize(width: v[12], height: v[13]) : nil
    let sq = Int64(v[11])
    // The page's view counts only when it is a new one: its other messages (the window moved, a menu opened) must not undo what the hand did since.
    if adopt || sq != s.seq {
        s.seq = sq
        stopAnimation()
        if !adopt && v[14] > 0 {
            clampView()
            animateView(to: v[8], panX: v[9], panY: v[10])
        } else {
            s.scale = v[8]; s.panX = v[9]; s.panY = v[10]
        }
    }
    clampView()
}

func clampView() {
    let s = State.shared
    s.scale = min(10, max(1, s.scale))
    if s.scale <= 1 { s.panX = 0; s.panY = 0; return }
    let lx = (s.scale - 1) * s.box.width / 2, ly = (s.scale - 1) * s.box.height / 2
    s.panX = min(lx, max(-lx, s.panX))
    s.panY = min(ly, max(-ly, s.panY))
}

/// Where the photo is now: its box scaled about the centre, then moved.
func imageRect() -> NSRect {
    let s = State.shared
    let w = s.box.width * s.scale, h = s.box.height * s.scale
    return NSRect(x: s.box.midX + s.panX - w / 2, y: s.box.midY + s.panY - h / 2, width: w, height: h)
}

/// Draws everything where the current view says (the layers, the sharp part, the map).
func relayout() {
    guard let w = State.shared.win else { return }
    place(w, imageRect(), State.shared.lastClipR)
}

func place(_ w: NSWindow, _ imgR: NSRect, _ clipR: NSRect) {
    guard let content = w.contentView, let host = State.shared.host else { return }
    let h = content.bounds.height
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    host.frame = NSRect(x: clipR.minX, y: h - clipR.maxY, width: clipR.width, height: clipR.height)
    State.shared.layer.frame = NSRect(x: imgR.minX - clipR.minX, y: clipR.maxY - imgR.maxY, width: imgR.width, height: imgR.height)
    State.shared.lastImgR = imgR
    State.shared.lastClipR = clipR
    if let f = State.shared.detailFrac {
        let top = imgR.minY + f.minY * imgR.height, hh = f.height * imgR.height
        State.shared.detail.frame = NSRect(x: imgR.minX + f.minX * imgR.width - clipR.minX, y: clipR.maxY - (top + hh), width: f.width * imgR.width, height: hh)
    }
    applyNav()
    CATransaction.commit()
    // the cursor depends on whether it is zoomed, on a drag, and on where the map is
    let s = State.shared
    let key = "\(s.scale > 1.02)|\(s.drag != nil)|\(s.navRect.map { "\($0)" } ?? "-")|\(s.lastClipR)"
    if key != s.cursorKey { s.cursorKey = key; w.invalidateCursorRects(for: host) }
}

/// The part of the photo inside its box, as fractions of the photo (top-left origin), from the view alone.
func visibleFraction() -> CGRect {
    let s = State.shared
    let w = max(1, s.box.width), h = max(1, s.box.height)
    let x0 = max(0, 0.5 + (-w / 2 - s.panX) / (w * s.scale)), x1 = min(1, 0.5 + (w / 2 - s.panX) / (w * s.scale))
    let y0 = max(0, 0.5 + (-h / 2 - s.panY) / (h * s.scale)), y1 = min(1, 0.5 + (h / 2 - s.panY) / (h * s.scale))
    return CGRect(x: x0, y: y0, width: max(0, x1 - x0), height: max(0, y1 - y0))
}

/// Lays out the map (a small picture of the whole photo with a frame for what is on screen) in the photo's bottom-right corner, when zoomed in
/// (inside a transaction that is already open).
func applyNav() {
    let s = State.shared
    guard let ns = s.navSize, s.scale > 1.02, s.host != nil, s.box.width > 0 else {
        s.navRect = nil
        s.navBack.isHidden = true
        return
    }
    let r = NSRect(x: s.box.maxX - 14 - ns.width, y: s.box.maxY - 14 - ns.height, width: ns.width, height: ns.height)
    s.navRect = r
    let clipR = s.lastClipR
    let box = NSRect(x: r.minX - clipR.minX, y: clipR.maxY - r.maxY, width: r.width, height: r.height)
    s.navBack.frame = box
    s.navBack.isHidden = false
    let f = visibleFraction()
    // the frame, in the map's own coordinates (bottom-left origin)
    let inner = NSRect(x: f.minX * box.width, y: (1 - f.maxY) * box.height, width: f.width * box.width, height: f.height * box.height)
    for l in [s.navDim, s.navFrame] { l.frame = CGRect(origin: .zero, size: box.size); l.isHidden = false }
    let all = CGMutablePath()
    all.addRect(CGRect(origin: .zero, size: box.size))
    all.addRect(inner)
    s.navDim.path = all
    s.navFrame.path = CGPath(rect: inner, transform: nil)
}

@_cdecl("fg_hdr_show")
public func fg_hdr_show(_ win: UnsafeMutableRawPointer, _ cpath: UnsafePointer<CChar>, _ v: UnsafePointer<Double>) {
    let w = window(win)
    let path = String(cString: cpath)
    let s = State.shared
    guard let content = w.contentView else { return }
    s.lock.lock(); let img = s.images[path]; s.lock.unlock()
    guard let img = img else { return }
    if s.host == nil {
        let host = GestureView(frame: .zero)
        host.wantsLayer = true
        host.layer?.masksToBounds = true
        s.layer.wantsExtendedDynamicRangeContent = true
        s.layer.contentsGravity = .resize
        s.layer.magnificationFilter = .linear
        s.layer.minificationFilter = .trilinear
        host.layer?.addSublayer(s.layer)
        s.detail.wantsExtendedDynamicRangeContent = true
        s.detail.contentsGravity = .resize
        s.detail.magnificationFilter = .linear
        s.detail.minificationFilter = .trilinear
        s.detail.isHidden = true
        host.layer?.addSublayer(s.detail)
        s.navBack.cornerRadius = 8
        s.navBack.masksToBounds = true
        s.navBack.contentsGravity = .resize
        s.navBack.minificationFilter = .trilinear
        s.navBack.borderColor = NSColor(white: 1, alpha: 0.28).cgColor
        s.navBack.borderWidth = 1
        s.navBack.backgroundColor = NSColor(white: 0.1, alpha: 1).cgColor
        s.navDim.fillRule = .evenOdd
        s.navDim.fillColor = NSColor(white: 0, alpha: 0.5).cgColor
        s.navFrame.fillColor = nil
        s.navFrame.strokeColor = NSColor.white.cgColor
        s.navFrame.lineWidth = 1.5
        // the dimming and the frame are inside the map, so its rounded corners clip them too
        s.navBack.isHidden = true
        s.navBack.addSublayer(s.navDim)
        s.navBack.addSublayer(s.navFrame)
        host.layer?.addSublayer(s.navBack)
        s.host = host
    }
    let host = s.host!
    if host.superview !== content { content.addSubview(host, positioned: .above, relativeTo: nil) }
    else if content.subviews.last !== host { content.addSubview(host, positioned: .above, relativeTo: nil) }
    let fresh = s.shown != path
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    s.layer.contents = img
    s.navBack.contents = img
    CATransaction.commit()
    takeFrame(w, v, adopt: true)
    relayout()
    host.isHidden = false
    if fresh {
        let a = CABasicAnimation(keyPath: "opacity")
        a.fromValue = 0.0; a.toValue = 1.0; a.duration = 0.18
        s.layer.add(a, forKey: "fade")
    }
    s.shown = path
}

@_cdecl("fg_hdr_move")
public func fg_hdr_move(_ win: UnsafeMutableRawPointer, _ v: UnsafePointer<Double>) {
    guard State.shared.shown != nil else { return }
    takeFrame(window(win), v, adopt: false)
    relayout()
}

@_cdecl("fg_hdr_hide")
public func fg_hdr_hide(_ win: UnsafeMutableRawPointer) {
    let s = State.shared
    stopAnimation()
    s.finalTimer?.invalidate()
    s.drag = nil
    s.dragNav = false
    s.shown = nil
    s.host?.isHidden = true
    s.host?.removeFromSuperview()
    s.layer.contents = nil
    s.detail.contents = nil
    s.detail.isHidden = true
    s.detailFrac = nil
    s.pendingDetail = nil
    s.navRect = nil
    s.navBack.contents = nil
    s.navBack.isHidden = true
    s.scale = 1; s.panX = 0; s.panY = 0
}

/// The page wants to hear about every view change made here by hand: (scale, panX, panY, seq of the page's last view, 1 for a press).
@_cdecl("fg_hdr_set_callback")
public func fg_hdr_set_callback(_ cb: ViewCallback?) {
    State.shared.emit = cb
}

// MARK: - the hand

/// Where the mouse is, in the page's coordinates (top-left origin).
func pagePoint(_ e: NSEvent) -> CGPoint {
    guard let content = State.shared.host?.window?.contentView else { return .zero }
    let p = content.convert(e.locationInWindow, from: nil)
    return CGPoint(x: p.x, y: content.bounds.height - p.y)
}

/// Tells the page the view changed. At most about once per frame while it keeps changing, and always the last one.
func notify(force: Bool = false) {
    let s = State.shared
    s.finalTimer?.invalidate()
    s.finalTimer = nil
    let now = CACurrentMediaTime()
    if force || now - s.lastEmit >= 0.012 {
        s.lastEmit = now
        s.emit?(s.scale, s.panX, s.panY, s.seq, 0)
    } else {
        s.finalTimer = Timer.scheduledTimer(withTimeInterval: 0.05, repeats: false) { _ in notify(force: true) }
    }
}

func stopAnimation() {
    State.shared.anim?.invalidate()
    State.shared.anim = nil
}

/// Zooms to `n` keeping the point `p` (page coordinates) of the photo where it is.
func zoom(to n: Double, about p: CGPoint?) {
    let s = State.shared
    let sc = min(10, max(1, n))
    let c = p ?? CGPoint(x: s.box.midX, y: s.box.midY)
    let cx = Double(c.x) - Double(s.box.midX), cy = Double(c.y) - Double(s.box.midY)
    let k = sc / s.scale
    s.panX = cx - k * (cx - s.panX)
    s.panY = cy - k * (cy - s.panY)
    s.scale = sc
    clampView()
    relayout()
    notify()
}

/// Glides to a view (the same ease-out the page's own jumps have).
func animateView(to sc: Double, panX px: Double, panY py: Double) {
    let s = State.shared
    stopAnimation()
    let a = (s: s.scale, x: s.panX, y: s.panY)
    let t0 = CACurrentMediaTime()
    let timer = Timer(timeInterval: 1.0 / 120, repeats: true) { t in
        let u = min(1, (CACurrentMediaTime() - t0) / 0.28), e = 1 - pow(1 - u, 3)
        s.scale = a.s + (sc - a.s) * e
        s.panX = a.x + (px - a.x) * e
        s.panY = a.y + (py - a.y) * e
        clampView()
        relayout()
        notify(force: u >= 1)
        if u >= 1 { t.invalidate(); if s.anim === t { s.anim = nil } }
    }
    RunLoop.main.add(timer, forMode: .common)
    s.anim = timer
}

/// Double-click: back to the whole photo when zoomed in, otherwise 3x about the point.
func gestureToggle(about p: CGPoint) {
    let s = State.shared
    if s.scale > 1.02 { animateView(to: 1, panX: 0, panY: 0); return }
    let cx = Double(p.x) - Double(s.box.midX), cy = Double(p.y) - Double(s.box.midY)
    let k = 3.0 / s.scale
    animateView(to: 3, panX: cx - k * (cx - s.panX), panY: cy - k * (cy - s.panY))
}

func gestureScroll(_ e: NSEvent) {
    // the same measure as the page's wheel zoom: exp(-deltaY * 0.0022) with the page's deltaY (down is positive), which is minus what AppKit reports
    scrollZoom(dy: e.hasPreciseScrollingDeltas ? Double(e.scrollingDeltaY) : Double(e.deltaY) * 30, at: pagePoint(e))
}

/// `dy` as AppKit reports a scroll (up is positive): scrolling up zooms in.
func scrollZoom(dy: Double, at p: CGPoint) {
    stopAnimation()
    if dy == 0 { return }
    zoom(to: State.shared.scale * exp(dy * 0.0022), about: p)
}

func gestureMagnify(_ e: NSEvent) {
    stopAnimation()
    zoom(to: State.shared.scale * (1 + Double(e.magnification)), about: pagePoint(e))
}

func centreOnMap(_ p: CGPoint) {
    let s = State.shared
    guard let nr = s.navRect, nr.width > 0, nr.height > 0 else { return }
    let fx = min(1, max(0, (Double(p.x) - nr.minX) / nr.width)), fy = min(1, max(0, (Double(p.y) - nr.minY) / nr.height))
    s.panX = -(fx - 0.5) * s.box.width * s.scale
    s.panY = -(fy - 0.5) * s.box.height * s.scale
    clampView()
    relayout()
    notify()
}

func gestureDown(_ e: NSEvent) {
    let s = State.shared
    stopAnimation()
    // a click here must not take the keyboard away from the page (Esc, the arrow keys and the letter shortcuts are the page's)
    if let w = s.host?.window, let fr = w.firstResponder, fr !== s.host {
        DispatchQueue.main.async { if w.firstResponder !== fr { _ = w.makeFirstResponder(fr) } }
    }
    let p = pagePoint(e)
    s.emit?(s.scale, s.panX, s.panY, s.seq, 1)   // a press: the page closes whatever menu is open
    if let nr = s.navRect, nr.contains(p) {
        s.dragNav = true
        centreOnMap(p)
        return
    }
    if e.clickCount == 2 { gestureToggle(about: p); return }
    if s.scale > 1.02 {
        s.drag = (Double(p.x), Double(p.y), s.panX, s.panY)
        if let h = s.host, let w = h.window { w.invalidateCursorRects(for: h) }
        relayout()
    }
}

func gestureDrag(_ e: NSEvent) {
    let s = State.shared
    let p = pagePoint(e)
    if s.dragNav { centreOnMap(p); return }
    guard let d = s.drag, s.scale > 1.02 else { return }
    s.panX = d.px + Double(p.x) - d.x
    s.panY = d.py + Double(p.y) - d.y
    clampView()
    relayout()
    notify()
}

func gestureUp(_ e: NSEvent) {
    let s = State.shared
    let moved = s.drag != nil || s.dragNav
    s.drag = nil
    s.dragNav = false
    if moved { relayout(); notify(force: true) }
}

/// Forgets the decoded photos (memory) when the viewer closes.
@_cdecl("fg_hdr_release")
public func fg_hdr_release() {
    let s = State.shared
    s.lock.lock(); s.images.removeAll(); s.order.removeAll(); s.source = nil; s.lock.unlock()
}

/// Decodes a region of the photo (fractions of the photo, top-left origin) at up to `outW` pixels wide, HDR gain map applied, and keeps it for
/// `fg_hdr_detail_show`. Core Image decodes only what the region needs, so this is quick even for a 200 MP photo (the first region of a photo
/// takes a moment; the next ones are cheaper). Callable from any thread. Returns 1 on success.
@_cdecl("fg_hdr_detail_render")
public func fg_hdr_detail_render(_ cpath: UnsafePointer<CChar>, _ fx: Double, _ fy: Double, _ fw: Double, _ fh: Double, _ outW: Int32) -> Int32 {
    let path = String(cString: cpath)
    let s = State.shared
    guard let ci = source(path) else { return 0 }
    let W = ci.extent.width, H = ci.extent.height
    let r = CGRect(x: ci.extent.minX + fx * W, y: ci.extent.minY + (1 - fy - fh) * H, width: fw * W, height: fh * H).integral.intersection(ci.extent)
    if r.isEmpty { return 0 }
    let k = min(1.0, CGFloat(outW) / r.width)
    let region = ci.cropped(to: r).transformed(by: CGAffineTransform(translationX: -r.minX, y: -r.minY)).transformed(by: CGAffineTransform(scaleX: k, y: k))
    guard let cg = s.ciContext.createCGImage(region, from: region.extent, format: .RGBAh, colorSpace: CGColorSpace(name: CGColorSpace.extendedLinearDisplayP3)) else { return 0 }
    s.lock.lock(); s.pendingDetail = cg; s.lock.unlock()
    return 1
}

/// Puts the region decoded by `fg_hdr_detail_render` over the photo, where `fx..fh` says it belongs, and keeps it there as the photo moves.
@_cdecl("fg_hdr_detail_show")
public func fg_hdr_detail_show(_ fx: Double, _ fy: Double, _ fw: Double, _ fh: Double) {
    let s = State.shared
    s.lock.lock(); let cg = s.pendingDetail; s.pendingDetail = nil; s.lock.unlock()
    guard let cg = cg, s.shown != nil else { return }
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    s.detail.contents = cg
    s.detailFrac = CGRect(x: fx, y: fy, width: fw, height: fh)
    s.detail.isHidden = false
    CATransaction.commit()
    relayout()
}

@_cdecl("fg_hdr_detail_hide")
public func fg_hdr_detail_hide() {
    let s = State.shared
    s.detail.isHidden = true
    s.detailFrac = nil
}
