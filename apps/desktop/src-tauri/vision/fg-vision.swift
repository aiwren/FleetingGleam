// fg-vision：FleetingGleam 在 macOS 15+ 上的 Apple Vision 边车（美学分、实用图片、人脸拍摄质量、眼睛与五官点）。
//
// 协议（stdin 进、stdout 出，请求可以不等回复连着发，回复可能乱序，靠 id 对上）：
//   {"id":n,"path":"/一张照片"}\n                    从文件读（旧路径；重新量眼睛 / 人脸时用）
//   {"id":n,"imgs":[[w,h],[w,h],...]}\n + 像素        分析线程内存里的像素直接送来：每张 w*h*3 字节的 8 位 RGB（摆正后、长边 1024），按 imgs 的顺序紧跟在这一行后面
//   -> {"id":n,"results":[{"ok":true,...},...]}\n   每张一个结果，顺序同请求
// 一个进程内同时处理 `--par N` 个请求（Vision 在一个进程里开几条线程的吞吐是一条的三四倍），所以主程序一般只起一个边车。
// `--cpu` 把 Vision 钉在 CPU 上；默认先试 GPU / 神经引擎，失败一次后这个进程改走 CPU。
// FG_VISION_TRACE=1 时把每张图的耗时打到 stderr（调性能用）。
import Foundation
import Vision
import CoreImage
import CoreGraphics
import CoreML

let args = CommandLine.arguments
let parallel = max(1, args.firstIndex(of: "--par").flatMap { i in i + 1 < args.count ? Int(args[i + 1]) : nil } ?? 1)
let trace = ProcessInfo.processInfo.environment["FG_VISION_TRACE"] == "1"
// 神经引擎的路径在有些会话里会失败：失败一次后整个进程改走 CPU（几条线程共用这个开关，所以加锁）
let cpuLock = NSLock()
nonisolated(unsafe) var cpuOnly = args.contains("--cpu")
func useCPU() -> Bool { cpuLock.lock(); defer { cpuLock.unlock() }; return cpuOnly }
func switchToCPU() -> Bool { cpuLock.lock(); defer { cpuLock.unlock() }; let was = cpuOnly; cpuOnly = true; return !was }

func forceCPU(_ r: VNRequest) {
    guard let cpu = MLComputeDevice.allComputeDevices.first(where: { if case .cpu = $0 { return true }; return false }),
          let stages = try? r.supportedComputeStageDevices else { return }
    for stage in stages.keys { r.setComputeDevice(cpu, for: stage) }
}

// ---------------------------------------------------------------- 分析一张图

func analyze(_ handler: VNImageRequestHandler, size: CGSize) throws -> [String: Any] {
    let aes = VNCalculateImageAestheticsScoresRequest()
    let face = VNDetectFaceCaptureQualityRequest()
    let marks = VNDetectFaceLandmarksRequest()
    if useCPU() { forceCPU(aes); forceCPU(face); forceCPU(marks) }
    try handler.perform([aes, face, marks])
    let a = aes.results?.first
    // 每张脸的睁眼程度：两只眼轮廓的高 / 宽（像素），两眼取平均。睁着约 0.25–0.4，闭着低于 0.15 左右。
    // `eyes` 与 `eyeBoxes` 同序同脸（后者是这张脸的框，左上为原点、相对整图归一化，给学习模型看同一块区域）：只算大到能量眼睛的脸。
    var eyes: [Double] = []
    var eyeBoxes: [[Double]] = []
    for obs in marks.results ?? [] {
        guard let l = obs.landmarks?.leftEye?.normalizedPoints, let r = obs.landmarks?.rightEye?.normalizedPoints, l.count > 3, r.count > 3 else { continue }
        let bw = Double(obs.boundingBox.width * size.width), bh = Double(obs.boundingBox.height * size.height)
        // 只有几十像素高的脸量不出眼睛，跳过
        guard bh >= 0.11 * Double(max(size.width, size.height)) else { continue }
        func ratio(_ pts: [CGPoint]) -> Double {
            let xs = pts.map { Double($0.x) }, ys = pts.map { Double($0.y) }
            let w = (xs.max()! - xs.min()!) * bw, h = (ys.max()! - ys.min()!) * bh
            return w > 0 ? h / w : 0
        }
        eyes.append((ratio(Array(l)) + ratio(Array(r))) / 2)
        // Vision 的 boundingBox 原点在左下；翻成左上，和 Rust 那边裁剪用的像素坐标一致
        let b = obs.boundingBox
        eyeBoxes.append([Double(b.origin.x), Double(1 - b.origin.y - b.height), Double(b.width), Double(b.height)])
    }
    // 每张有完整关键点的脸，给人物识别用：框加五个点（左眼、右眼、鼻子、左嘴角、右嘴角，"左"指画面里的左），
    // 全部左上为原点、相对整图归一化。每张脸 14 个数：x, y, w, h，然后五对 (x, y)。
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

/// 一张图的来源：文件，或内存里的像素。
enum Source {
    case file(URL)
    case pixels(Data, Int, Int)

    /// 文件按 EXIF 方向摆正；像素已经是摆正的 8 位 RGB，用 sRGB 建成 CGImage（不拷贝）。
    func handler() throws -> (VNImageRequestHandler, CGSize) {
        switch self {
        case .file(let url):
            guard let ci = CIImage(contentsOf: url, options: [.applyOrientationProperty: true]) else {
                throw NSError(domain: "fg", code: 1, userInfo: [NSLocalizedDescriptionKey: "cannot decode image"])
            }
            return (VNImageRequestHandler(ciImage: ci), ci.extent.size)
        case .pixels(let data, let w, let h):
            guard w > 0, h > 0, data.count == w * h * 3,
                  let provider = CGDataProvider(data: data as CFData),
                  let cs = CGColorSpace(name: CGColorSpace.sRGB),
                  let cg = CGImage(width: w, height: h, bitsPerComponent: 8, bitsPerPixel: 24, bytesPerRow: w * 3, space: cs,
                                   bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.none.rawValue), provider: provider,
                                   decode: nil, shouldInterpolate: false, intent: .defaultIntent) else {
                throw NSError(domain: "fg", code: 2, userInfo: [NSLocalizedDescriptionKey: "bad pixel buffer \(w)x\(h) (\(data.count) bytes)"])
            }
            return (VNImageRequestHandler(cgImage: cg), CGSize(width: w, height: h))
        }
    }
}

func analyze(_ src: Source) -> [String: Any] {
    let t0 = DispatchTime.now()
    func once() throws -> [String: Any] {
        let (h, size) = try src.handler()
        return try analyze(h, size: size)
    }
    var out: [String: Any]
    do {
        out = try once()
    } catch {
        // 默认（神经引擎）路径失败：这个进程改走 CPU 再试一次
        out = ["ok": false, "error": "\(error.localizedDescription)"]
        if !useCPU() {
            _ = switchToCPU()
            if let r = try? once() { out = r }
        }
    }
    if trace {
        let ms = Double(DispatchTime.now().uptimeNanoseconds - t0.uptimeNanoseconds) / 1e6
        FileHandle.standardError.write(Data(String(format: "fg-vision: %.1f ms faces=%d\n", ms, (out["faces"] as? [Double])?.count ?? 0).utf8))
    }
    return out
}

// ---------------------------------------------------------------- 读请求、写回复

/// stdin 上自己做缓冲的读法：一行 JSON 头后面紧跟二进制像素，标准库按行读的那套不能混用。
final class Input {
    private var buf = Data()
    private var pos = 0
    private let chunk = 1 << 20

    /// 再从 stdin 读一块；到了文件尾返回 false。
    private func fill() -> Bool {
        if pos > 0 { buf.removeSubrange(0..<pos); pos = 0 }
        var tmp = [UInt8](repeating: 0, count: chunk)
        let n = tmp.withUnsafeMutableBytes { read(0, $0.baseAddress, chunk) }
        if n <= 0 { return false }
        buf.append(contentsOf: tmp[0..<n])
        return true
    }

    func line() -> String? {
        while true {
            if let nl = buf[pos...].firstIndex(of: 0x0A) {
                let s = String(decoding: buf[pos..<nl], as: UTF8.self)
                pos = nl + 1
                return s
            }
            if !fill() { return nil }
        }
    }

    /// 正好 n 字节；不够就先用缓冲里的，剩下直接读进目标，少一次拷贝。
    func bytes(_ n: Int) -> Data? {
        var out = Data(count: n)
        var got = 0
        let have = min(n, buf.count - pos)
        if have > 0 {
            out.replaceSubrange(0..<have, with: buf[pos..<(pos + have)])
            pos += have
            got = have
        }
        while got < n {
            let r = out.withUnsafeMutableBytes { read(0, $0.baseAddress! + got, n - got) }
            if r <= 0 { return nil }
            got += r
        }
        return out
    }
}

let outLock = NSLock()
func emit(_ d: [String: Any]) {
    guard var data = try? JSONSerialization.data(withJSONObject: d) else { return }
    data.append(0x0A)
    outLock.lock()
    FileHandle.standardOutput.write(data)
    outLock.unlock()
}

let input = Input()
// 显式要用户级 QoS：没有界面的子进程里不指定时队列继承的 QoS 可能让它落到能效核上（量过没有明显差别，但不留这个变数）
let queue = DispatchQueue(label: "fg-vision", qos: .userInitiated, attributes: .concurrent)
// 同时在做的请求不超过 `parallel` 个，另外最多再预读两个在内存里等着（每张 1024px 的像素约 2.4 MB）
let slots = DispatchSemaphore(value: parallel + 2)
let inflight = DispatchGroup()

while let head = input.line() {
    guard let data = head.data(using: .utf8), let req = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else {
        // 读不懂请求头就无法再对齐后面的二进制：退出，主程序会重新起一个
        FileHandle.standardError.write(Data("fg-vision: bad request header, exiting\n".utf8))
        exit(1)
    }
    let id = req["id"] as? Int ?? 0
    var sources: [Source] = []
    if let path = req["path"] as? String {
        sources = [.file(URL(fileURLWithPath: path))]
    } else if let imgs = req["imgs"] as? [[Int]] {
        for wh in imgs {
            guard wh.count == 2, wh[0] >= 0, wh[1] >= 0, wh[0] * wh[1] <= 64_000_000, let px = input.bytes(wh[0] * wh[1] * 3) else {
                FileHandle.standardError.write(Data("fg-vision: truncated pixel request, exiting\n".utf8))
                exit(1)
            }
            sources.append(.pixels(px, wh[0], wh[1]))
        }
    }
    slots.wait()
    inflight.enter()
    queue.async {
        // 一次请求里的几张也并行做
        var results = [[String: Any]](repeating: [:], count: sources.count)
        let lock = NSLock()
        DispatchQueue.concurrentPerform(iterations: sources.count) { i in
            let r = analyze(sources[i])
            lock.lock(); results[i] = r; lock.unlock()
        }
        emit(["id": id, "results": results])
        slots.signal()
        inflight.leave()
    }
}
inflight.wait()
