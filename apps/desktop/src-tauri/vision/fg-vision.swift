// fg-vision: on-device Apple Vision signals for FleetingGleam (macOS 15+).
// Protocol: one image path per stdin line -> one JSON line on stdout.
import Foundation
import Vision
import CoreImage
import CoreML

setvbuf(stdout, nil, _IOLBF, 0)
// `--cpu` pins Vision to the CPU; otherwise the default (GPU / Neural Engine) is tried first.
var useCPU = CommandLine.arguments.contains("--cpu")

func forceCPU(_ r: VNRequest) {
    guard let cpu = MLComputeDevice.allComputeDevices.first(where: { if case .cpu = $0 { return true }; return false }),
          let stages = try? r.supportedComputeStageDevices else { return }
    for stage in stages.keys { r.setComputeDevice(cpu, for: stage) }
}

func analyze(_ url: URL) throws -> [String: Any] {
    guard let ci = CIImage(contentsOf: url, options: [.applyOrientationProperty: true]) else {
        throw NSError(domain: "fg", code: 1, userInfo: [NSLocalizedDescriptionKey: "cannot decode image"])
    }
    let aes = VNCalculateImageAestheticsScoresRequest()
    let face = VNDetectFaceCaptureQualityRequest()
    let marks = VNDetectFaceLandmarksRequest()
    if useCPU { forceCPU(aes); forceCPU(face); forceCPU(marks) }
    try VNImageRequestHandler(ciImage: ci).perform([aes, face, marks])
    let a = aes.results?.first
    // Eye openness per face: height / width of each eye's outline, in pixels, averaged over both eyes.
    // About 0.25-0.4 for open eyes, below ~0.15 when they are closed.
    let size = ci.extent.size
    // Same face, same order, as both `eyes` (the geometric ratio) and `eyeBoxes` (its crop, top-left-origin
    // normalized, for a learned model to look at the same region): only faces big enough to measure eyes on.
    var eyes: [Double] = []
    var eyeBoxes: [[Double]] = []
    for obs in marks.results ?? [] {
        guard let l = obs.landmarks?.leftEye?.normalizedPoints, let r = obs.landmarks?.rightEye?.normalizedPoints, l.count > 3, r.count > 3 else { continue }
        let bw = Double(obs.boundingBox.width * size.width), bh = Double(obs.boundingBox.height * size.height)
        // The eyes of a face only a few dozen pixels tall cannot be measured: leave those faces out.
        guard bh >= 0.11 * Double(max(size.width, size.height)) else { continue }
        func ratio(_ pts: [CGPoint]) -> Double {
            let xs = pts.map { Double($0.x) }, ys = pts.map { Double($0.y) }
            let w = (xs.max()! - xs.min()!) * bw, h = (ys.max()! - ys.min()!) * bh
            return w > 0 ? h / w : 0
        }
        eyes.append((ratio(Array(l)) + ratio(Array(r))) / 2)
        // Vision's boundingBox has a bottom-left origin; flip to top-left, matching the pixel buffer Rust crops.
        let b = obs.boundingBox
        eyeBoxes.append([Double(b.origin.x), Double(1 - b.origin.y - b.height), Double(b.width), Double(b.height)])
    }
    // Every face with a full set of landmarks, for identity matching: its box plus five points (left eye, right eye,
    // nose, left mouth corner, right mouth corner, "left" meaning left *in the picture*), all top-left-origin and
    // normalised to the image. 14 numbers per face: x, y, w, h, then the five (x, y) pairs.
    func centre(_ pts: [CGPoint]) -> CGPoint {
        CGPoint(x: pts.map { $0.x }.reduce(0, +) / CGFloat(pts.count), y: pts.map { $0.y }.reduce(0, +) / CGFloat(pts.count))
    }
    var faceMarks: [[Double]] = []
    for obs in marks.results ?? [] {
        guard let lm = obs.landmarks, let le = lm.leftEye, let re = lm.rightEye, let nose = lm.nose, let lips = lm.outerLips,
              le.pointCount > 0, re.pointCount > 0, nose.pointCount > 0, lips.pointCount > 1 else { continue }
        let leP = le.pointsInImage(imageSize: size), reP = re.pointsInImage(imageSize: size)
        let a = centre(leP), b = centre(reP)
        let (eyeL, eyeR) = a.x <= b.x ? (a, b) : (b, a)
        let n = centre(nose.pointsInImage(imageSize: size))
        let lp = lips.pointsInImage(imageSize: size)
        guard let mL = lp.min(by: { $0.x < $1.x }), let mR = lp.max(by: { $0.x < $1.x }) else { continue }
        func p(_ c: CGPoint) -> [Double] { [Double(c.x / size.width), Double(1 - c.y / size.height)] }
        let bb = obs.boundingBox
        var row: [Double] = [Double(bb.origin.x), Double(1 - bb.origin.y - bb.height), Double(bb.width), Double(bb.height)]
        for c in [eyeL, eyeR, n, mL, mR] { row.append(contentsOf: p(c)) }
        faceMarks.append(row)
    }
    return [
        "ok": true,
        "aesthetic": Double(a?.overallScore ?? 0),
        "utility": a?.isUtility ?? false,
        "faces": (face.results ?? []).compactMap { $0.faceCaptureQuality }.map { Double($0) },
        "eyes": eyes,
        "eyeBoxes": eyeBoxes,
        "faceMarks": faceMarks,
    ]
}

func emit(_ d: [String: Any]) {
    if let data = try? JSONSerialization.data(withJSONObject: d), let s = String(data: data, encoding: .utf8) { print(s) }
}

while let line = readLine() {
    let url = URL(fileURLWithPath: line)
    do {
        emit(try analyze(url))
    } catch {
        // The default (Neural Engine) path fails in some sessions; fall back to CPU once and stick with it.
        if !useCPU {
            useCPU = true
            if let r = try? analyze(url) { emit(r); continue }
        }
        emit(["ok": false, "error": "\(error.localizedDescription)"])
    }
}
