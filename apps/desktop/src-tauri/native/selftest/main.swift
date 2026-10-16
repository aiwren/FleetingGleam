// Self-test of the native photo layer (overlay.swift) without the web view: a real window, the real layers, events delivered to the real view.
//
//   swiftc -o /tmp/fg-selftest native/overlay.swift native/selftest/main.swift && /tmp/fg-selftest /path/to/photo.jpg
//
// Not part of the app's build (build.rs compiles overlay.swift alone).
import AppKit
import QuartzCore

var failures = 0
func check(_ ok: Bool, _ what: String) { print(ok ? "ok    " : "FAIL  ", what); if !ok { failures += 1 } }
func near(_ a: Double, _ b: Double, _ eps: Double = 1e-3) -> Bool { abs(a - b) < eps }
func near(_ a: CGFloat, _ b: Double, _ eps: Double = 1e-3) -> Bool { abs(Double(a) - b) < eps }

guard CommandLine.arguments.count > 1 else { print("usage: fg-selftest photo.jpg"); exit(2) }
let photo = CommandLine.arguments[1]

_ = NSApplication.shared
let win = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1000, height: 700), styleMask: [.titled], backing: .buffered, defer: false)
let content = NSView(frame: NSRect(x: 0, y: 0, width: 1000, height: 700))
win.contentView = content
let ptr = Unmanaged.passUnretained(win).toOpaque()

var events: [(scale: Double, x: Double, y: Double, ack: Int64, press: Int32)] = []
let callback: ViewCallback = { s, x, y, ack, press in events.append((s, x, y, ack, press)) }
fg_hdr_set_callback(callback)

/// [box x y w h, clip x y w h, scale, panX, panY, seq, map w h, glide]
func vals(box: [Double] = [100, 50, 800, 500], clip: [Double]? = nil, scale: Double = 1, x: Double = 0, y: Double = 0, seq: Double, glide: Double = 0) -> [Double] {
    box + (clip ?? box) + [scale, x, y, seq, 150, 100, glide]
}
func show(_ v: [Double]) { v.withUnsafeBufferPointer { fg_hdr_show(ptr, photo, $0.baseAddress!) } }
func move(_ v: [Double]) { v.withUnsafeBufferPointer { fg_hdr_move(ptr, $0.baseAddress!) } }
func spin(_ s: Double) { RunLoop.main.run(until: Date(timeIntervalSinceNow: s)) }
let S = State.shared

/// Mouse events at a page point (top-left origin) as the window sees them (bottom-left).
func mouse(_ type: NSEvent.EventType, _ x: Double, _ y: Double, clicks: Int = 1) -> NSEvent {
    NSEvent.mouseEvent(with: type, location: NSPoint(x: x, y: 700 - y), modifierFlags: [], timestamp: 0, windowNumber: win.windowNumber, context: nil, eventNumber: 0, clickCount: clicks, pressure: 1)!
}
/// The photo point (fraction of the photo) under a page point, with the photo where it is now.
func photoPoint(_ x: Double, _ y: Double) -> (Double, Double) {
    let r = imageRect()
    return ((x - Double(r.minX)) / Double(r.width), (y - Double(r.minY)) / Double(r.height))
}

check(fg_hdr_prepare(photo, 0) == 1, "the photo decodes")
show(vals(seq: 3))
guard let host = content.subviews.last else { print("FAIL  no host view"); exit(1) }
check(host.frame == NSRect(x: 100, y: 150, width: 800, height: 500), "the host sits on the clip rectangle (flipped to bottom-left)")
check(host.hitTest(NSPoint(x: 500, y: 400)) === host, "the host takes the mouse over the photo")
check(host.hitTest(NSPoint(x: 50, y: 400)) == nil, "...and not outside it (the step arrows stay the page's)")
check(near(S.scale, 1) && S.seq == 3, "the page's first view is adopted (scale 1, seq 3)")

// --- wheel zoom keeps the point under the cursor where it is
let before = photoPoint(700, 300)
scrollZoom(dy: 100, at: CGPoint(x: 700, y: 300))
let k = exp(100 * 0.0022)
check(near(S.scale, k), "scrolling up by 100 zooms in by exp(0.22) = \(String(format: "%.4f", k))")
let after = photoPoint(700, 300)
check(near(before.0, after.0) && near(before.1, after.1), "the photo point under the cursor did not move")
check(near(S.layer.frame.width, 800 * k) && near(S.layer.frame.minX, Double(imageRect().minX) - 100), "the photo layer follows (size and position inside the host)")
check(events.last.map { near($0.scale, k) && $0.ack == 3 && $0.press == 0 } ?? false, "the page is told the view, with the seq it last sent")

// --- the map in the corner, with the frame for what is on screen
check(S.navRect == NSRect(x: 736, y: 436, width: 150, height: 100), "the map is in the photo's bottom-right corner (14 pt in)")
let vf = visibleFraction()
check(near(vf.width, 1 / k) && near(vf.minX, 0.5 + (-400 - S.panX) / (800 * k)), "the frame in the map is the visible part of the photo")
check(!S.navBack.isHidden && near(S.navBack.frame.minX, 636) && near(S.navBack.frame.minY, 14), "the map layer is placed (host coordinates)")

// --- drag, clamped to the photo
let px0 = S.panX
host.mouseDown(with: mouse(.leftMouseDown, 500, 300))
check(events.last?.press == 1, "a press is reported to the page (it closes its menus)")
host.mouseDragged(with: mouse(.leftMouseDragged, 450, 320))
check(near(S.panX, max(-98.4, px0 - 50), 0.5) && near(S.panY, 20, 0.5), "dragging moves the photo with the hand (and stops at the edge)")
host.mouseDragged(with: mouse(.leftMouseDragged, 0, 700))
check(near(S.panX, -(k - 1) * 400, 0.5) && near(S.panY, (k - 1) * 250, 0.5), "...and cannot go beyond the photo's edge (left and down here)")
host.mouseUp(with: mouse(.leftMouseUp, 0, 700))
check(events.last.map { near($0.x, S.panX) && near($0.y, S.panY) } ?? false, "the last state is reported at the end of the drag")

// --- clicking the map jumps there
host.mouseDown(with: mouse(.leftMouseDown, 740, 440))
host.mouseUp(with: mouse(.leftMouseUp, 740, 440))
let vf2 = visibleFraction()
check(vf2.minX < 0.001 && vf2.minY < 0.001, "a click on the map's top-left shows the photo's top-left")

// --- the page's own views: a new one counts, a repeat of an old one does not undo the hand
move(vals(scale: 1, seq: 4))
check(near(S.scale, 1) && S.seq == 4, "a new view from the page (seq 4) is adopted")
scrollZoom(dy: 50, at: CGPoint(x: 500, y: 300))
let handScale = S.scale
move(vals(scale: 1, seq: 4))
check(near(S.scale, handScale), "a repeat of the page's old view (same seq) does not undo what the hand did")
move(vals(scale: 1, seq: 5))
check(near(S.scale, 1), "...but a newer one does")

// --- double-click: glide to 3x about the point, and back
let p0 = photoPoint(300, 300)
host.mouseDown(with: mouse(.leftMouseDown, 300, 300, clicks: 2))
host.mouseUp(with: mouse(.leftMouseUp, 300, 300, clicks: 2))
check(near(S.scale, 1, 0.5) && S.anim != nil, "double-click starts a glide (not a jump)")
spin(0.45)
check(near(S.scale, 3), "double-click zooms to 3x")
let p1 = photoPoint(300, 300)
check(near(p0.0, p1.0, 0.002) && near(p0.1, p1.1, 0.002), "...about the cursor")
host.mouseDown(with: mouse(.leftMouseDown, 300, 300, clicks: 2))
spin(0.45)
check(near(S.scale, 1) && near(S.panX, 0) && near(S.panY, 0), "a second double-click goes back to the whole photo")
check(S.navRect == nil && S.navBack.isHidden, "the map is gone at 1x")

// --- the page's discrete jumps glide too
move(vals(scale: 2, seq: 6, glide: 1))
check(near(S.scale, 1, 0.5), "a page jump with glide does not jump")
spin(0.45)
check(near(S.scale, 2), "...it arrives")

// --- the sharp part follows the photo
check(fg_hdr_detail_render(photo, 0.25, 0.25, 0.5, 0.5, 400) == 1, "a region decodes")
fg_hdr_detail_show(0.25, 0.25, 0.5, 0.5)
check(!S.detail.isHidden && near(S.detail.frame.width, 0.5 * 800 * 2), "the sharp region is drawn over the photo at the right size")
scrollZoom(dy: 40, at: CGPoint(x: 400, y: 250))
let r = imageRect(), clip = S.lastClipR
check(near(S.detail.frame.minX, Double(r.minX) + 0.25 * Double(r.width) - Double(clip.minX)), "...and moves with it")

// --- the Live menu takes the top of the photo's area: the host gives way
move(vals(clip: [100, 200, 800, 350], scale: S.scale, seq: 6))
check(host.hitTest(NSPoint(x: 500, y: 600)) == nil && host.hitTest(NSPoint(x: 500, y: 300)) === host, "with the menu open the host's frame is smaller (the menu stays clickable)")

fg_hdr_hide(ptr)
check(host.superview == nil && S.shown == nil && near(S.scale, 1), "hiding removes the view and forgets the view")
print(failures == 0 ? "\nall passed" : "\n\(failures) FAILED")
exit(failures == 0 ? 0 : 1)
