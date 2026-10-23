// fg-image: one-shot image helper for FleetingGleam, on top of the system's ImageIO (macOS 15+).
// It is how the app sees what the camera really recorded: HDR gain maps, wide colour, HEIC / AVIF / JPEG XL / TIFF / camera RAW.
//
//   fg-image info   <in> [fast]             -> one JSON line: {"w":..,"h":..,"hdr":bool}   (size as displayed, EXIF orientation applied; "fast" skips the HDR check)
//   fg-image crop   <in> <out.jpg> <fx> <fy> <fw> <fh> <outW> -> the region (fractions of the upright photo, top-left origin) at up to <outW> pixels wide, as a JPEG
//   fg-image render <in> <out.jpg> <max> [sdr] -> a JPEG whose longest side is at most <max>, upright; keeps the HDR gain map unless "sdr" is given
//   fg-image serve [--par N]                   -> 常驻：stdin 每行一个请求 {"id":n,"path":"...","max":1024}，stdout 回一行 {"id":n,"w":..,"h":..,"ow":..,"oh":..}
//                                                 后面紧跟 w*h*3 字节的 8 位 RGB 像素（摆正、长边不超过 max、SDR；默认按照片自己的色彩空间原样给数值，
//                                                 "managed":true 时转成 sRGB）；失败回 {"id":n,"error":"..."}。
//                                                 分析流水线用它代替进程内整张解码：JPEG 在 DCT 域缩放，快好几倍；一个进程同时解 N 张，回复可乱序。
import Foundation
import ImageIO
import CoreGraphics
import UniformTypeIdentifiers
import CoreImage

func fail(_ msg: String) -> Never {
    FileHandle.standardError.write(Data((msg + "\n").utf8))
    exit(1)
}

func open(_ path: String) -> CGImageSource {
    guard let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary),
          CGImageSourceGetCount(src) > 0 else { fail("cannot open \(path)") }
    return src
}

/// Displayed size: the stored size, swapped when the EXIF orientation is a quarter turn.
func displayedSize(_ src: CGImageSource) -> (Int, Int) {
    let p = CGImageSourceCopyPropertiesAtIndex(src, 0, nil) as? [CFString: Any] ?? [:]
    let w = (p[kCGImagePropertyPixelWidth] as? Int) ?? 0, h = (p[kCGImagePropertyPixelHeight] as? Int) ?? 0
    let o = (p[kCGImagePropertyOrientation] as? Int) ?? 1
    return (5...8).contains(o) ? (h, w) : (w, h)
}

/// HDR when the file carries a gain map, or its pixels decode with headroom (HEIC/AVIF in PQ or HLG, for instance).
func isHDR(_ src: CGImageSource) -> Bool {
    for t in [kCGImageAuxiliaryDataTypeISOGainMap, kCGImageAuxiliaryDataTypeHDRGainMap] {
        if CGImageSourceCopyAuxiliaryDataInfoAtIndex(src, 0, t) != nil { return true }
    }
    // Camera RAW is scene-referred: it has no HDR rendering of its own to show beyond a gain map.
    if let ty = CGImageSourceGetType(src) as String?, let ut = UTType(ty), ut.conforms(to: .rawImage) { return false }
    let o: [CFString: Any] = [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceThumbnailMaxPixelSize: 64, kCGImageSourceDecodeRequest: kCGImageSourceDecodeToHDR]
    if let t = CGImageSourceCreateThumbnailAtIndex(src, 0, o as CFDictionary) { return t.contentHeadroom > 1.0 }
    return false
}

// ---------------------------------------------------------------- serve：常驻解码

/// stdin 上自己做缓冲的按行读（请求都是一行 JSON）。
final class Input {
    private var buf = Data()
    private var pos = 0
    private func fill() -> Bool {
        if pos > 0 { buf.removeSubrange(0..<pos); pos = 0 }
        var tmp = [UInt8](repeating: 0, count: 1 << 16)
        let n = tmp.withUnsafeMutableBytes { read(0, $0.baseAddress, 1 << 16) }
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
}

/// 一张照片摆正、缩到长边 `maxSide`、SDR 的紧凑 8 位 RGB 像素。给出 (像素, 宽, 高, 原宽, 原高)。
/// `managed` 为真时把颜色转到 sRGB；为假（分析用的默认）时按照片自己的色彩空间原样给出数值——和 `image` crate 整张解码（不理会色彩配置）
/// 一致，老缓存里的分数才接得上。
func decodeScaled(_ path: String, _ maxSide: Int, managed: Bool) -> (Data, Int, Int, Int, Int)? {
    guard let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary),
          CGImageSourceGetCount(src) > 0 else { return nil }
    let (ow, oh) = displayedSize(src)
    let side = min(maxSide, max(ow, oh))
    let o: [CFString: Any] = [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceCreateThumbnailWithTransform: true, kCGImageSourceThumbnailMaxPixelSize: side]
    guard let img = CGImageSourceCreateThumbnailAtIndex(src, 0, o as CFDictionary) else { return nil }
    // 目标尺寸按核心库 `fast_resize` 的规则四舍五入（ImageIO 自己的缩略图短边是向下取整的，会差一个像素），画进画布时顺手对齐
    let scale = Double(side) / Double(max(ow, oh, 1))
    let (w, h) = ow > side || oh > side ? (max(1, Int((Double(ow) * scale).rounded())), max(1, Int((Double(oh) * scale).rounded()))) : (ow, oh)
    guard w > 0, h > 0, let srgb = CGColorSpace(name: CGColorSpace.sRGB) else { return nil }
    // 不做色彩管理时用照片自己的 RGB 空间当画布（灰度等别的模型画不进 RGB 画布，退回 sRGB）
    let own = img.colorSpace.flatMap { $0.model == .rgb ? $0 : nil }
    let cs = managed ? srgb : (own ?? srgb)
    // CGBitmapContext 不支持 24 位，先画成 RGBX 再压紧
    var rgba = [UInt8](repeating: 0, count: w * h * 4)
    guard let ctx = CGContext(data: &rgba, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: cs, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue) else { return nil }
    ctx.interpolationQuality = .high
    ctx.draw(img, in: CGRect(x: 0, y: 0, width: w, height: h))
    var rgb = Data(count: w * h * 3)
    rgb.withUnsafeMutableBytes { (dst: UnsafeMutableRawBufferPointer) in
        let d = dst.baseAddress!.assumingMemoryBound(to: UInt8.self)
        rgba.withUnsafeBufferPointer { s in
            for i in 0..<(w * h) {
                d[i * 3] = s[i * 4]; d[i * 3 + 1] = s[i * 4 + 1]; d[i * 3 + 2] = s[i * 4 + 2]
            }
        }
    }
    return (rgb, w, h, ow, oh)
}

func serve(parallel: Int) {
    let trace = ProcessInfo.processInfo.environment["FG_IMAGE_TRACE"] == "1"
    let input = Input()
    let outLock = NSLock()
    // 显式要用户级 QoS：边车是没有界面的子进程，不指定时队列继承的 QoS 可能让它落到能效核上（量过没有明显差别，但不留这个变数）
    let queue = DispatchQueue(label: "fg-image", qos: .userInitiated, attributes: .concurrent)
    let slots = DispatchSemaphore(value: max(1, parallel))
    let inflight = DispatchGroup()
    func reply(_ head: [String: Any], _ body: Data?) {
        guard var data = try? JSONSerialization.data(withJSONObject: head) else { return }
        data.append(0x0A)
        outLock.lock()
        FileHandle.standardOutput.write(data)
        if let body = body { FileHandle.standardOutput.write(body) }
        outLock.unlock()
    }
    while let line = input.line() {
        guard let data = line.data(using: .utf8), let req = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { continue }
        let id = req["id"] as? Int ?? 0
        let managed = req["managed"] as? Bool ?? false
        guard let path = req["path"] as? String, let maxSide = req["max"] as? Int, maxSide > 0 else {
            reply(["id": id, "error": "bad request"], nil)
            continue
        }
        slots.wait()
        inflight.enter()
        queue.async {
            let t0 = DispatchTime.now()
            if let (rgb, w, h, ow, oh) = decodeScaled(path, maxSide, managed: managed) {
                let t1 = DispatchTime.now()
                reply(["id": id, "w": w, "h": h, "ow": ow, "oh": oh], rgb)
                if trace {
                    let ms = { (a: DispatchTime, b: DispatchTime) in Double(b.uptimeNanoseconds - a.uptimeNanoseconds) / 1e6 }
                    FileHandle.standardError.write(Data(String(format: "fg-image: decode %.1f ms, reply %.1f ms\n", ms(t0, t1), ms(t1, DispatchTime.now())).utf8))
                }
            } else {
                reply(["id": id, "error": "cannot decode \(path)"], nil)
            }
            slots.signal()
            inflight.leave()
        }
    }
    inflight.wait()
}

let args = CommandLine.arguments
guard args.count >= 2 else { fail("usage: fg-image info <in> | render <in> <out.jpg> <max> [sdr] | crop ... | serve [--par N]") }
if args[1] == "serve" {
    let par = args.firstIndex(of: "--par").flatMap { i in i + 1 < args.count ? Int(args[i + 1]) : nil } ?? 4
    serve(parallel: par)
    exit(0)
}
guard args.count >= 3 else { fail("usage: fg-image info <in> | render <in> <out.jpg> <max> [sdr]") }

switch args[1] {
case "info":
    let src = open(args[2])
    let (w, h) = displayedSize(src)
    let hdr = args.count > 3 && args[3] == "fast" ? false : isHDR(src)
    print("{\"w\":\(w),\"h\":\(h),\"hdr\":\(hdr)}")
case "render":
    guard args.count >= 5, let maxSide = Int(args[4]) else { fail("render needs <in> <out.jpg> <max>") }
    let sdr = args.count > 5 && args[5] == "sdr"
    let src = open(args[2])
    let (w, h) = displayedSize(src)
    let side = min(maxSide, max(w, h))
    let hdr = !sdr && isHDR(src)
    var o: [CFString: Any] = [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceCreateThumbnailWithTransform: true, kCGImageSourceThumbnailMaxPixelSize: side]
    if hdr { o[kCGImageSourceDecodeRequest] = kCGImageSourceDecodeToHDR }
    guard let img = CGImageSourceCreateThumbnailAtIndex(src, 0, o as CFDictionary) else { fail("cannot decode \(args[2])") }
    guard let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: args[3]) as CFURL, UTType.jpeg.identifier as CFString, 1, nil) else { fail("cannot write \(args[3])") }
    var d: [CFString: Any] = [kCGImageDestinationLossyCompressionQuality: 0.86]
    if hdr { d[kCGImageDestinationEncodeRequest] = kCGImageDestinationEncodeToISOHDR }
    CGImageDestinationAddImage(dest, img, d as CFDictionary)
    guard CGImageDestinationFinalize(dest) else { fail("cannot encode \(args[3])") }
case "crop":
    guard args.count >= 9, let fx = Double(args[4]), let fy = Double(args[5]), let fw = Double(args[6]), let fh = Double(args[7]), let outW = Double(args[8]) else { fail("crop needs <in> <out.jpg> <fx> <fy> <fw> <fh> <outW>") }
    guard let ci = CIImage(contentsOf: URL(fileURLWithPath: args[2]), options: [.applyOrientationProperty: true]) else { fail("cannot decode \(args[2])") }
    let W = ci.extent.width, H = ci.extent.height
    let r = CGRect(x: ci.extent.minX + fx * W, y: ci.extent.minY + (1 - fy - fh) * H, width: fw * W, height: fh * H).integral.intersection(ci.extent)
    if r.isEmpty { fail("empty region") }
    let k = min(1.0, CGFloat(outW) / r.width)
    let region = ci.cropped(to: r).transformed(by: CGAffineTransform(translationX: -r.minX, y: -r.minY)).transformed(by: CGAffineTransform(scaleX: k, y: k))
    let ctx = CIContext()
    let space = CGColorSpace(name: CGColorSpace.displayP3) ?? CGColorSpaceCreateDeviceRGB()
    guard let data = ctx.jpegRepresentation(of: region, colorSpace: space, options: [kCGImageDestinationLossyCompressionQuality as CIImageRepresentationOption: 0.9]) else { fail("cannot encode") }
    do { try data.write(to: URL(fileURLWithPath: args[3])) } catch { fail("cannot write \(args[3])") }
default:
    fail("unknown command \(args[1])")
}
