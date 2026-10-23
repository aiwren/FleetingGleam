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
//                                                 JPEG 还有一条硬件路径（VideoToolbox 整张解码 + vImage 缩放，几乎不占 CPU）和软件路径并行分担：请求带 "hw":true 强走硬件
//                                                 （不行再回落软件）、"hw":false 只走软件、不带由边车自己按代价调度（硬件排队的估计时间不超过软件的估计时间）；
//                                                 硬件解的回复头多一个 "hw":true。FG_IMAGE_HW_SCALE 选硬件路径的缩放滤波（lanczos / lanczos5 / cg）。
import Foundation
import ImageIO
import CoreGraphics
import UniformTypeIdentifiers
import CoreImage
import VideoToolbox
import CoreMedia
import CoreVideo
import Accelerate
import Metal

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

// ---------------------------------------------------------------- serve：硬件 JPEG 解码（VideoToolbox）

/// JPEG 文件头里和硬件解码有关的几件事：帧头类型、精度、分量数、存储尺寸。
struct JPEGFrame {
    var sof: UInt8
    var precision: Int
    var components: Int
    var width: Int
    var height: Int
    /// EXIF 方向（1–8；没有就是 1）
    var orientation: Int
    /// 硬件解码器认得的：SOF0 基线、SOF1 扩展顺序式，8 位，三分量（YCbCr）。渐进式（SOF2）、无损、算术编码、灰度、CMYK 都走软件。
    var hardwareFriendly: Bool { (sof == 0xC0 || sof == 0xC1) && precision == 8 && components == 3 }
}

/// APP1 段里的 EXIF：TIFF 头（字节序、42、第 0 个 IFD 的偏移），IFD0 的条目里找方向标签 0x0112。读不出来就当 1。
func exifOrientation(_ p: UnsafePointer<UInt8>, _ start: Int, _ end: Int) -> Int? {
    // start 指向段长度之后的内容："Exif\0\0" + TIFF
    guard start + 14 <= end, p[start] == 0x45, p[start + 1] == 0x78, p[start + 2] == 0x69, p[start + 3] == 0x66 else { return nil }
    let t = start + 6
    let little: Bool
    if p[t] == 0x49 && p[t + 1] == 0x49 { little = true } else if p[t] == 0x4D && p[t + 1] == 0x4D { little = false } else { return nil }
    func u16(_ i: Int) -> Int { little ? Int(p[i]) | Int(p[i + 1]) << 8 : Int(p[i]) << 8 | Int(p[i + 1]) }
    func u32(_ i: Int) -> Int { little ? u16(i) | u16(i + 2) << 16 : u16(i) << 16 | u16(i + 2) }
    guard u16(t + 2) == 42 else { return nil }
    let ifd = t + u32(t + 4)
    guard ifd + 2 <= end else { return nil }
    let count = u16(ifd)
    for k in 0..<count {
        let e = ifd + 2 + k * 12
        guard e + 12 <= end else { return nil }
        if u16(e) == 0x0112 {
            let v = u16(e + 8)
            return (1...8).contains(v) ? v : nil
        }
    }
    return nil
}

/// 顺着标记段扫到帧头（SOF）为止，顺手记下 APP1 里的 EXIF 方向；不是 JPEG、或帧头之前就到了扫描数据，给 nil。只看文件头，不碰压缩数据。
func scanJPEG(_ data: Data) -> JPEGFrame? {
    return data.withUnsafeBytes { (raw: UnsafeRawBufferPointer) -> JPEGFrame? in
        guard let p = raw.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return nil }
        let n = raw.count
        guard n > 4, p[0] == 0xFF, p[1] == 0xD8 else { return nil }
        var i = 2
        var orientation = 1
        while i + 4 <= n {
            guard p[i] == 0xFF else { return nil }
            var m = p[i + 1]
            while m == 0xFF && i + 2 < n { i += 1; m = p[i + 1] }   // 标记前可以有填充的 0xFF
            guard i + 4 <= n else { return nil }
            if m == 0xD8 || m == 0x01 || (0xD0...0xD7).contains(m) { i += 2; continue }   // 没有长度的独立标记
            if m == 0xD9 || m == 0xDA { return nil }   // 帧头之前就结束 / 进了扫描数据
            let len = Int(p[i + 2]) << 8 | Int(p[i + 3])
            if m == 0xE1, let o = exifOrientation(p, i + 4, min(n, i + 2 + len)) { orientation = o }
            if (0xC0...0xCF).contains(m) && m != 0xC4 && m != 0xC8 && m != 0xCC {
                guard i + 10 <= n else { return nil }
                return JPEGFrame(sof: m, precision: Int(p[i + 4]), components: Int(p[i + 9]), width: Int(p[i + 7]) << 8 | Int(p[i + 8]), height: Int(p[i + 5]) << 8 | Int(p[i + 6]), orientation: orientation)
            }
            i += 2 + len
        }
        return nil
    }
}

/// GPU 上的缩小：两趟（先横后竖）可分离的三角滤波，权重和核心库 `fast_resize`（fast_image_resize 的 Bilinear 卷积）一个算法——
/// 输出像素中心 (x+0.5)·scale 两侧 scale 以内的源像素按 1−|d|/scale 加权、归一。这样硬件解出来的图缩小后和老路几乎一样，清晰度分不漂，
/// 而且不花 CPU（12 MP 到 1024 一张约 1–2 ms GPU）。
final class GPUShrinker {
    private let device: MTLDevice
    private let queue: MTLCommandQueue
    private let horizontal: MTLComputePipelineState
    private let vertical: MTLComputePipelineState
    private var cache: CVMetalTextureCache?
    private var mid: MTLTexture?
    private var out: MTLTexture?

    static let source = """
    #include <metal_stdlib>
    using namespace metal;
    // 一维三角滤波：沿 dir（0 横 1 竖）把 src 缩到 dst 的那一维，另一维原样
    static float4 tent(texture2d<float, access::read> src, uint2 gid, float scale, uint dir, uint srcLen) {
        float center = (float(dir == 0 ? gid.x : gid.y) + 0.5) * scale;
        float support = max(scale, 1.0);
        int lo = max(0, int(floor(center - support)));
        int hi = min(int(srcLen) - 1, int(ceil(center + support)));
        float4 acc = 0; float wsum = 0;
        for (int i = lo; i <= hi; i++) {
            float d = fabs(float(i) + 0.5 - center) / support;
            if (d >= 1.0) continue;
            float w = 1.0 - d;
            acc += w * src.read(dir == 0 ? uint2(i, gid.y) : uint2(gid.x, i));
            wsum += w;
        }
        return wsum > 0 ? acc / wsum : acc;
    }
    kernel void shrinkH(texture2d<float, access::read> src [[texture(0)]], texture2d<float, access::write> dst [[texture(1)]], constant float& scale [[buffer(0)]], uint2 gid [[thread_position_in_grid]]) {
        if (gid.x >= dst.get_width() || gid.y >= dst.get_height()) return;
        dst.write(tent(src, gid, scale, 0, src.get_width()), gid);
    }
    kernel void shrinkV(texture2d<float, access::read> src [[texture(0)]], texture2d<float, access::write> dst [[texture(1)]], constant float& scale [[buffer(0)]], uint2 gid [[thread_position_in_grid]]) {
        if (gid.x >= dst.get_width() || gid.y >= dst.get_height()) return;
        dst.write(tent(src, gid, scale, 1, src.get_height()), gid);
    }
    """

    init?() {
        guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue(),
              let lib = try? device.makeLibrary(source: GPUShrinker.source, options: nil),
              let h = lib.makeFunction(name: "shrinkH"), let v = lib.makeFunction(name: "shrinkV"),
              let hp = try? device.makeComputePipelineState(function: h), let vp = try? device.makeComputePipelineState(function: v) else { return nil }
        self.device = device
        self.queue = queue
        horizontal = hp
        vertical = vp
        guard CVMetalTextureCacheCreate(nil, nil, device, nil, &cache) == kCVReturnSuccess else { return nil }
    }

    private func texture(_ w: Int, _ h: Int, _ format: MTLPixelFormat, shared: Bool, reuse: MTLTexture?) -> MTLTexture? {
        if let t = reuse, t.width == w, t.height == h { return t }
        let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: format, width: w, height: h, mipmapped: false)
        d.usage = [.shaderRead, .shaderWrite]
        d.storageMode = shared ? .shared : .private
        return device.makeTexture(descriptor: d)
    }

    /// 把 BGRA 的像素缓冲缩到 (tw, th)，写进 `dst`（tw*4 字节一行的 BGRA）。
    func shrink(_ pb: CVPixelBuffer, to tw: Int, _ th: Int, into dst: UnsafeMutableRawPointer) -> Bool {
        guard let cache = cache else { return false }
        let sw = CVPixelBufferGetWidth(pb), sh = CVPixelBufferGetHeight(pb)
        var cvtex: CVMetalTexture?
        guard CVMetalTextureCacheCreateTextureFromImage(nil, cache, pb, nil, .bgra8Unorm, sw, sh, 0, &cvtex) == kCVReturnSuccess, let cvtex = cvtex, let src = CVMetalTextureGetTexture(cvtex) else { return false }
        guard let mid = texture(tw, sh, .rgba16Float, shared: false, reuse: mid), let out = texture(tw, th, .bgra8Unorm, shared: true, reuse: out) else { return false }
        self.mid = mid
        self.out = out
        guard let cmd = queue.makeCommandBuffer(), let enc = cmd.makeComputeCommandEncoder() else { return false }
        var sx = Float(sw) / Float(tw), sy = Float(sh) / Float(th)
        let groups = { (p: MTLComputePipelineState, w: Int, h: Int) -> (MTLSize, MTLSize) in
            let tg = MTLSize(width: 16, height: 16, depth: 1)
            return (MTLSize(width: (w + 15) / 16, height: (h + 15) / 16, depth: 1), tg)
        }
        enc.setComputePipelineState(horizontal)
        enc.setTexture(src, index: 0)
        enc.setTexture(mid, index: 1)
        enc.setBytes(&sx, length: 4, index: 0)
        let (g1, t1) = groups(horizontal, tw, sh)
        enc.dispatchThreadgroups(g1, threadsPerThreadgroup: t1)
        enc.setComputePipelineState(vertical)
        enc.setTexture(mid, index: 0)
        enc.setTexture(out, index: 1)
        enc.setBytes(&sy, length: 4, index: 0)
        let (g2, t2) = groups(vertical, tw, th)
        enc.dispatchThreadgroups(g2, threadsPerThreadgroup: t2)
        enc.endEncoding()
        cmd.commit()
        cmd.waitUntilCompleted()
        guard cmd.status == .completed else { return false }
        out.getBytes(dst, bytesPerRow: tw * 4, from: MTLRegionMake2D(0, 0, tw, th), mipmapLevel: 0)
        CVMetalTextureCacheFlush(cache, 0)
        return true
    }
}

/// 固定几条自己起的线程按先来后到做活。不用 GCD 的并发队列：GCD 给每个 QoS 档的线程数有上限，软件解码把它占满时，
/// VideoToolbox 自己内部的派发队列就拿不到线程，一张硬件解码会从 30 ms 拖到 100 ms 上下（量过）；专用线程不占 GCD 的额度。
final class WorkerPool {
    private let lock = NSLock()
    private var jobs: [() -> Void] = []
    private let pending = DispatchSemaphore(value: 0)

    init(threads: Int, name: String, qos: QualityOfService) {
        for i in 0..<max(1, threads) {
            let t = Thread { [unowned self] in
                while true {
                    self.pending.wait()
                    self.lock.lock()
                    let job = self.jobs.removeFirst()
                    self.lock.unlock()
                    job()
                }
            }
            t.name = "\(name).\(i)"
            t.qualityOfService = qos
            t.start()
        }
    }

    func async(_ job: @escaping () -> Void) {
        lock.lock()
        jobs.append(job)
        lock.unlock()
        pending.signal()
    }
}

/// VideoToolbox 的硬件 JPEG 解码器：整张解成 32BGRA。硬件是串行的（M1 上一张 4096×3072 约 30 ms，几乎不占 CPU），所以只有一条线程按先来后到做；
/// 会话按尺寸复用（同一台相机的照片尺寸都一样，最多留四个尺寸）。这台机器要是没有硬件解码器（会话建不起来）就记下来，以后全走软件——
/// 和没有这条路时的行为一样。
///
/// 这条线程是自己起的（`WorkerPool`，一条、QoS 更高），不是 GCD 串行队列：软件路径把核数那么多的块都占着时，GCD 不再给同级的队列发线程，
/// 硬件那一路就得等某张软件解码结束才轮得到（量过：十线程下 60 张只有 9 张走了硬件）。
final class HardwareJPEG {
    private let lock = NSLock()
    private let worker = WorkerPool(threads: 1, name: "fg-image.hw", qos: .userInteractive)
    private var sessions: [[Int]: (VTDecompressionSession, CMVideoFormatDescription)] = [:]
    private var recent: [[Int]] = []
    private var failures = 0
    private var noDecoder = false
    /// 这台机器没有硬件 JPEG 解码器（或它一直建不起会话）：以后全走软件
    var unavailable: Bool {
        lock.lock()
        defer { lock.unlock() }
        return noDecoder
    }
    /// 排在硬件队列里（含正在解的那张）的工作量估计，毫秒；调度用
    private var backlogMs = 0.0

    /// 给这张照片在硬件队列里占个位：排上它之后的总等待不超过 `limitMs`（或 `force`）才占得到。占到了解完必须 `release(costMs)`。
    func reserve(costMs: Double, limitMs: Double, force: Bool) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if noDecoder { return false }
        if force || backlogMs + costMs <= limitMs {
            backlogMs += costMs
            return true
        }
        return false
    }

    func release(_ costMs: Double) {
        lock.lock()
        backlogMs = max(0, backlogMs - costMs)
        lock.unlock()
    }

    /// 排进硬件线程，按顺序执行。
    func async(_ job: @escaping () -> Void) {
        worker.async(job)
    }

    private func drop(_ key: [Int]) {
        if let (s, _) = sessions.removeValue(forKey: key) { VTDecompressionSessionInvalidate(s) }
        recent.removeAll { $0 == key }
    }

    private func session(_ w: Int, _ h: Int) -> (VTDecompressionSession, CMVideoFormatDescription)? {
        let key = [w, h]
        if let s = sessions[key] { return s }
        var fmt: CMVideoFormatDescription?
        guard CMVideoFormatDescriptionCreate(allocator: nil, codecType: kCMVideoCodecType_JPEG, width: Int32(w), height: Int32(h), extensions: nil, formatDescriptionOut: &fmt) == noErr, let fmt = fmt else { return nil }
        // IOSurface 背的缓冲，Metal 能零拷贝当纹理用
        let attrs: [CFString: Any] = [kCVPixelBufferPixelFormatTypeKey: kCVPixelFormatType_32BGRA, kCVPixelBufferIOSurfacePropertiesKey: [:] as CFDictionary, kCVPixelBufferMetalCompatibilityKey: true]
        let spec: [CFString: Any] = [kVTVideoDecoderSpecification_RequireHardwareAcceleratedVideoDecoder: true]
        var s: VTDecompressionSession?
        let st = VTDecompressionSessionCreate(allocator: nil, formatDescription: fmt, decoderSpecification: spec as CFDictionary, imageBufferAttributes: attrs as CFDictionary, outputCallback: nil, decompressionSessionOut: &s)
        guard st == noErr, let s = s else {
            // 找不到硬件解码器（Intel Mac）就此作罢；别的错误（这个尺寸不收之类）连着三次也作罢
            failures += 1
            if st == kVTCouldNotFindVideoDecoderErr || st == kVTVideoDecoderNotAvailableNowErr || failures >= 3 {
                lock.lock()
                noDecoder = true
                lock.unlock()
            }
            return nil
        }
        failures = 0
        if sessions.count >= 4, let old = recent.first { drop(old) }
        sessions[key] = (s, fmt)
        recent.append(key)
        return (s, fmt)
    }

    /// 压缩数据的常驻输入缓冲：只在硬件线程上用，按需增长、从不缩回。每张都新映射文件的话，解码器读它时要重新缺页；
    /// 读进一块已经驻留的内存就省掉这些缺页（量过差别不大，但不留这个变数）。
    private var input: UnsafeMutableRawPointer?
    private var inputCapacity = 0

    /// 把文件整个读进常驻缓冲，给出字节数；读不全给 nil。
    private func load(_ path: String) -> Int? {
        let fd = open(path, O_RDONLY)
        guard fd >= 0 else { return nil }
        defer { close(fd) }
        var st = stat()
        guard fstat(fd, &st) == 0, st.st_size > 0, st.st_size < 1 << 31 else { return nil }
        let size = Int(st.st_size)
        if size > inputCapacity {
            input?.deallocate()
            inputCapacity = max(size, inputCapacity * 3 / 2)
            input = UnsafeMutableRawPointer.allocate(byteCount: inputCapacity, alignment: 16)
        }
        guard let buf = input else { return nil }
        var got = 0
        while got < size {
            let n = pread(fd, buf + got, size - got, off_t(got))
            if n <= 0 { return nil }
            got += n
        }
        return size
    }

    /// 整张解出来（存储方向、32BGRA）。失败给 nil，调用方回落软件。
    func decode(_ path: String, _ frame: JPEGFrame) -> CVPixelBuffer? {
        guard let (s, fmt) = session(frame.width, frame.height), let size = load(path), let base = input else { return nil }
        // 块缓冲直接指着常驻缓冲（kCFAllocatorNull：内存不归它管）；解码是同步的，返回前缓冲不会被动
        var block: CMBlockBuffer?
        guard CMBlockBufferCreateWithMemoryBlock(allocator: nil, memoryBlock: base, blockLength: size, blockAllocator: kCFAllocatorNull, customBlockSource: nil, offsetToData: 0, dataLength: size, flags: 0, blockBufferOut: &block) == noErr, let block = block else { return nil }
        var sample: CMSampleBuffer?
        var sampleSize = size
        guard CMSampleBufferCreateReady(allocator: nil, dataBuffer: block, formatDescription: fmt, sampleCount: 1, sampleTimingEntryCount: 0, sampleTimingArray: nil, sampleSizeEntryCount: 1, sampleSizeArray: &sampleSize, sampleBufferOut: &sample) == noErr, let sample = sample else { return nil }
        var out: CVPixelBuffer?
        var outStatus: OSStatus = noErr
        let st = VTDecompressionSessionDecodeFrame(s, sampleBuffer: sample, flags: [], infoFlagsOut: nil) { status, _, img, _, _ in
            outStatus = status
            out = img
        }
        if st == kVTInvalidSessionErr { drop([frame.width, frame.height]) }   // 会话失效（比如睡眠唤醒），下次重建
        return st == noErr && outStatus == noErr ? out : nil
    }
}

/// 把硬件解出来的整张 BGRA 缩到存储方向的 (tw, th)，再按 EXIF 方向摆正、压成紧凑 RGB。缩放滤波由 `method` 定：
/// 默认 GPU 上的三角滤波（`gpu`，和老路一样的权重）；"cg" 用 CoreGraphics 的高质量插值，"lanczos5" 用 vImage 的 Lanczos5，
/// "lanczos" 用 vImage 默认（Lanczos3）——这三个在 CPU 上，留着对比。不做色彩转换。
func shrinkAndOrient(_ pb: CVPixelBuffer, to tw: Int, _ th: Int, orientation: Int, method: String, gpu: GPUShrinker?) -> Data? {
    let sw = CVPixelBufferGetWidth(pb), sh = CVPixelBufferGetHeight(pb)
    guard tw > 0, th > 0, sw >= tw, sh >= th else { return nil }
    var small = [UInt8](repeating: 0, count: tw * th * 4)
    if method == "gpu", let gpu = gpu, sw != tw || sh != th {
        let ok = small.withUnsafeMutableBytes { gpu.shrink(pb, to: tw, th, into: $0.baseAddress!) }
        return ok ? orientRGB(small, tw, th, orientation) : nil
    }
    CVPixelBufferLockBaseAddress(pb, .readOnly)
    defer { CVPixelBufferUnlockBaseAddress(pb, .readOnly) }
    guard let base = CVPixelBufferGetBaseAddress(pb) else { return nil }
    let sbpr = CVPixelBufferGetBytesPerRow(pb)
    let ok: Bool = small.withUnsafeMutableBytes { dstRaw in
        guard let dptr = dstRaw.baseAddress else { return false }
        if sw == tw && sh == th {
            for y in 0..<th { memcpy(dptr + y * tw * 4, base + y * sbpr, tw * 4) }
            return true
        }
        if method == "cg" {
            let info = CGBitmapInfo(rawValue: CGBitmapInfo.byteOrder32Little.rawValue | CGImageAlphaInfo.noneSkipFirst.rawValue)
            guard let cs = CGColorSpace(name: CGColorSpace.sRGB),
                  let provider = CGDataProvider(dataInfo: nil, data: base, size: sbpr * sh, releaseData: { _, _, _ in }),
                  let img = CGImage(width: sw, height: sh, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: sbpr, space: cs, bitmapInfo: info, provider: provider, decode: nil, shouldInterpolate: true, intent: .defaultIntent),
                  let ctx = CGContext(data: dptr, width: tw, height: th, bitsPerComponent: 8, bytesPerRow: tw * 4, space: cs, bitmapInfo: info.rawValue) else { return false }
            ctx.interpolationQuality = .high
            ctx.draw(img, in: CGRect(x: 0, y: 0, width: tw, height: th))
            return true
        }
        var src = vImage_Buffer(data: base, height: vImagePixelCount(sh), width: vImagePixelCount(sw), rowBytes: sbpr)
        var dst = vImage_Buffer(data: dptr, height: vImagePixelCount(th), width: vImagePixelCount(tw), rowBytes: tw * 4)
        let flags = vImage_Flags(method == "lanczos5" ? kvImageHighQualityResampling : kvImageNoFlags)
        return vImageScale_ARGB8888(&src, &dst, nil, flags) == kvImageNoError
    }
    guard ok else { return nil }
    return orientRGB(small, tw, th, orientation)
}

/// 把存储方向的 BGRA 小图按 EXIF 方向（1–8）摆正，顺手压成紧凑 RGB。每种方向都是一个整数仿射映射：
/// 源坐标 sx = ax·x + bx·y + cx，sy = ay·x + by·y + cy，于是一行里源下标按固定步长走。
func orientRGB(_ bgra: [UInt8], _ tw: Int, _ th: Int, _ orientation: Int) -> Data {
    let turned = (5...8).contains(orientation)
    let (dw, dh) = turned ? (th, tw) : (tw, th)
    let (ax, bx, cx, ay, by, cy): (Int, Int, Int, Int, Int, Int)
    switch orientation {
    case 2: (ax, bx, cx, ay, by, cy) = (-1, 0, tw - 1, 0, 1, 0)           // 左右镜像
    case 3: (ax, bx, cx, ay, by, cy) = (-1, 0, tw - 1, 0, -1, th - 1)     // 转 180°
    case 4: (ax, bx, cx, ay, by, cy) = (1, 0, 0, 0, -1, th - 1)           // 上下镜像
    case 5: (ax, bx, cx, ay, by, cy) = (0, 1, 0, 1, 0, 0)                 // 转置
    case 6: (ax, bx, cx, ay, by, cy) = (0, 1, 0, -1, 0, th - 1)           // 顺时针转 90°（手机竖拍最常见）
    case 7: (ax, bx, cx, ay, by, cy) = (0, -1, tw - 1, -1, 0, th - 1)     // 反转置
    case 8: (ax, bx, cx, ay, by, cy) = (0, -1, tw - 1, 1, 0, 0)           // 逆时针转 90°
    default: (ax, bx, cx, ay, by, cy) = (1, 0, 0, 0, 1, 0)
    }
    let step = (ay * tw + ax) * 4
    var rgb = Data(count: dw * dh * 3)
    rgb.withUnsafeMutableBytes { (dst: UnsafeMutableRawBufferPointer) in
        let d = dst.baseAddress!.assumingMemoryBound(to: UInt8.self)
        bgra.withUnsafeBufferPointer { s in
            for y in 0..<dh {
                var si = ((by * y + cy) * tw + bx * y + cx) * 4
                var di = y * dw * 3
                for _ in 0..<dw {
                    d[di] = s[si + 2]; d[di + 1] = s[si + 1]; d[di + 2] = s[si]
                    si += step; di += 3
                }
            }
        }
    }
    return rgb
}


func serve(parallel: Int) {
    let env = ProcessInfo.processInfo.environment
    let trace = env["FG_IMAGE_TRACE"] == "1"
    let hwScale = env["FG_IMAGE_HW_SCALE"] ?? "gpu"
    // 调度按代价估：硬件整张解码的时间随像素数走（M1 上 12 MP 约 30 ms，再加几毫秒 GPU 缩放），排在它前面的也要等；软件路径在 DCT 域缩小，
    // 时间随文件大小（熵解码）和像素数走，几路同时解还会互相拖慢（十路只有四到六倍的扩展）。哪边估出来便宜走哪边。
    // 两亿像素的全景拼接图硬件要 500 ms 以上、还把队列堵死，而软件只要它的几分之一，所以大过 FG_IMAGE_HW_MAX_MP（默认 32）的只走软件；
    // FG_IMAGE_HW_BIAS 给硬件的估价乘个系数（小于 1 多走硬件，省 CPU；大于 1 少走），对比用。
    let hwMaxPixels = Int((Double(env["FG_IMAGE_HW_MAX_MP"] ?? "") ?? 32) * 1e6)
    let hwBias = Double(env["FG_IMAGE_HW_BIAS"] ?? "") ?? 1
    // 系数都是 M1 Max 上量的：硬件空闲时 12.6 MP 一张 30 ms，同进程里软件解码满负荷时 45 ms（取后者）；软件 23 MB / 12.6 MP 的单独 65 ms、
    // 十路在飞时中位 200 ms
    func hwCost(_ frame: JPEGFrame) -> Double { 6 + 45 * Double(frame.width * frame.height) / 12.6e6 }
    var swInflight = 0
    let swLock = NSLock()
    func swCost(bytes: Int, _ frame: JPEGFrame) -> Double {
        swLock.lock()
        let busy = swInflight
        swLock.unlock()
        let solo = 25 + 2 * Double(bytes) / 1e6 + 2.5 * Double(frame.width * frame.height) / 1e6
        return solo * (1 + Double(busy) / 10)
    }
    let input = Input()
    let outLock = NSLock()
    // 软件路径：`parallel` 条专用线程。显式要用户级 QoS：边车是没有界面的子进程，不指定时可能落到能效核上
    let swPool = WorkerPool(threads: parallel, name: "fg-image.sw", qos: .userInitiated)
    let inflight = DispatchGroup()
    let hw = HardwareJPEG()
    // GPU 缩放器建不起来（没有 Metal 设备）就退到 vImage
    let gpu: GPUShrinker? = hwScale == "gpu" ? GPUShrinker() : nil
    let hwScaleUsed = hwScale == "gpu" && gpu == nil ? "lanczos" : hwScale
    func reply(_ head: [String: Any], _ body: Data?) {
        guard var data = try? JSONSerialization.data(withJSONObject: head) else { return }
        data.append(0x0A)
        outLock.lock()
        FileHandle.standardOutput.write(data)
        if let body = body { FileHandle.standardOutput.write(body) }
        outLock.unlock()
    }
    func log(_ s: String) {
        if trace { FileHandle.standardError.write(Data((s + "\n").utf8)) }
    }
    let ms = { (a: DispatchTime, b: DispatchTime) in Double(b.uptimeNanoseconds - a.uptimeNanoseconds) / 1e6 }
    // 本进程到现在用掉的 CPU 毫秒（调性能用；只在一次只解一张时每个阶段的差值才有意义）
    func cpuMs() -> Double {
        var ru = rusage()
        getrusage(RUSAGE_SELF, &ru)
        return Double(ru.ru_utime.tv_sec + ru.ru_stime.tv_sec) * 1000 + Double(ru.ru_utime.tv_usec + ru.ru_stime.tv_usec) / 1000
    }
    /// 软件路径：ImageIO 缩略解码（JPEG 在 DCT 域缩放），同时最多 `parallel` 张，多的排队，不堵住读请求的主循环（硬件队列还要继续收）。
    func software(_ id: Int, _ path: String, _ maxSide: Int, _ managed: Bool) {
        inflight.enter()
        swLock.lock()
        swInflight += 1
        swLock.unlock()
        swPool.async {
            defer {
                swLock.lock()
                swInflight -= 1
                swLock.unlock()
                inflight.leave()
            }
            let t0 = DispatchTime.now()
            let c0 = trace ? cpuMs() : 0
            if let (rgb, w, h, ow, oh) = decodeScaled(path, maxSide, managed: managed) {
                let t1 = DispatchTime.now()
                let c1 = trace ? cpuMs() : 0
                reply(["id": id, "w": w, "h": h, "ow": ow, "oh": oh], rgb)
                log(String(format: "fg-image: sw decode %.1f ms (cpu %.0f), reply %.1f ms", ms(t0, t1), c1 - c0, ms(t1, DispatchTime.now())))
            } else {
                reply(["id": id, "error": "cannot decode \(path)"], nil)
            }
        }
    }
    /// 硬件路径：VideoToolbox 整张解码 + GPU 缩放 + 按 EXIF 摆正。会话建不起来、解失败，一律回落软件路径。
    /// `frame` 是主循环扫文件头时读到的；文件内容由硬件线程自己读进常驻缓冲。
    func hardware(_ id: Int, _ path: String, _ maxSide: Int, _ frame: JPEGFrame) {
        inflight.enter()
        let cost = hwCost(frame)
        hw.async {
            defer {
                hw.release(cost)
                inflight.leave()
            }
            let t0 = DispatchTime.now()
            let c0 = trace ? cpuMs() : 0
            let turned = (5...8).contains(frame.orientation)
            let (ow, oh) = turned ? (frame.height, frame.width) : (frame.width, frame.height)
            guard let pb = hw.decode(path, frame) else {
                log("fg-image: hw decode failed, falling back \(path)")
                software(id, path, maxSide, false)
                return
            }
            let t1 = DispatchTime.now()
            let c1 = trace ? cpuMs() : 0
            // 目标尺寸和软件路径一个算法（核心库 `fast_resize` 的四舍五入）；缩放在存储方向上做，摆正留给 orientRGB
            let side = min(maxSide, max(ow, oh))
            let scale = Double(side) / Double(max(ow, oh, 1))
            let (w, h) = ow > side || oh > side ? (max(1, Int((Double(ow) * scale).rounded())), max(1, Int((Double(oh) * scale).rounded()))) : (ow, oh)
            let (tw, th) = turned ? (h, w) : (w, h)
            guard let rgb = shrinkAndOrient(pb, to: tw, th, orientation: frame.orientation, method: hwScaleUsed, gpu: gpu) else {
                log("fg-image: hw shrink failed, falling back \(path)")
                software(id, path, maxSide, false)
                return
            }
            let t2 = DispatchTime.now()
            let c2 = trace ? cpuMs() : 0
            reply(["id": id, "w": w, "h": h, "ow": ow, "oh": oh, "hw": true], rgb)
            log(String(format: "fg-image: hw decode %.1f ms (cpu %.0f), shrink %.1f ms (cpu %.0f), reply %.1f ms  %dx%d %@", ms(t0, t1), c1 - c0, ms(t1, t2), c2 - c1, ms(t2, DispatchTime.now()), frame.width, frame.height, (path as NSString).lastPathComponent))
        }
    }
    while let line = input.line() {
        guard let data = line.data(using: .utf8), let req = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { continue }
        let id = req["id"] as? Int ?? 0
        let managed = req["managed"] as? Bool ?? false
        guard let path = req["path"] as? String, let maxSide = req["max"] as? Int, maxSide > 0 else {
            reply(["id": id, "error": "bad request"], nil)
            continue
        }
        // 调度：基线 JPEG、像素数在上限内、硬件那边估出来不比软件慢的走硬件（请求明说 "hw":true 就一定排硬件、"hw":false 一定软件）；
        // 别的格式、渐进式、太大的、要转 sRGB 的都走软件。文件头按内存映射读（网络卷等不宜映射的地方 Foundation 自己退回整读），只碰开头几 KB
        let want = req["hw"] as? Bool
        let ext = (path as NSString).pathExtension.lowercased()
        var routed = false
        if want != false, !managed, ext == "jpg" || ext == "jpeg", !hw.unavailable,
           let data = try? Data(contentsOf: URL(fileURLWithPath: path), options: .mappedIfSafe), let frame = scanJPEG(data), frame.hardwareFriendly,
           want == true || frame.width * frame.height <= hwMaxPixels,
           hw.reserve(costMs: hwCost(frame) * hwBias, limitMs: swCost(bytes: data.count, frame), force: want == true) {
            hardware(id, path, maxSide, frame)
            routed = true
        }
        if !routed { software(id, path, maxSide, managed) }
    }
    inflight.wait()
}

/// 诊断：硬件 JPEG 解码在这台机器上到底多快、什么会拖慢它（`fg-image bench-hw <jpeg> [n]`）。同一张图各种情形各解 n 次，打 ms/张。
func benchHardware(_ path: String, _ n: Int) {
    guard let data = try? Data(contentsOf: URL(fileURLWithPath: path), options: .mappedIfSafe), let frame = scanJPEG(data) else { fail("not a JPEG: \(path)") }
    print("\(frame.width)x\(frame.height), \(data.count / 1_000_000) MB, SOF\(frame.sof - 0xC0), orientation \(frame.orientation), hardware friendly: \(frame.hardwareFriendly)")
    let hw = HardwareJPEG()
    guard hw.decode(path, frame) != nil else { fail("hardware decode failed (no hardware JPEG decoder here?)") }
    let gpu = GPUShrinker()
    let side = min(1024, max(frame.width, frame.height))
    let scale = Double(side) / Double(max(frame.width, frame.height))
    let (tw, th) = (max(1, Int((Double(frame.width) * scale).rounded())), max(1, Int((Double(frame.height) * scale).rounded())))
    func time(_ label: String, _ body: () -> Void) {
        let t = DispatchTime.now()
        var ru0 = rusage(); getrusage(RUSAGE_SELF, &ru0)
        for _ in 0..<n { body() }
        var ru1 = rusage(); getrusage(RUSAGE_SELF, &ru1)
        let cpu = (Double(ru1.ru_utime.tv_sec - ru0.ru_utime.tv_sec + ru1.ru_stime.tv_sec - ru0.ru_stime.tv_sec) * 1e6 + Double(ru1.ru_utime.tv_usec - ru0.ru_utime.tv_usec + ru1.ru_stime.tv_usec - ru0.ru_stime.tv_usec)) / 1000
        print(String(format: "%@: %.1f ms/张（进程 CPU %.0f ms/张）", label, Double(DispatchTime.now().uptimeNanoseconds - t.uptimeNanoseconds) / 1e6 / Double(n), cpu / Double(n)))
    }
    time("紧循环只解码（每次重读文件进常驻缓冲）") { _ = hw.decode(path, frame) }
    time("解码 + GPU 缩放 + 摆正") { if let pb = hw.decode(path, frame) { _ = shrinkAndOrient(pb, to: tw, th, orientation: frame.orientation, method: "gpu", gpu: gpu) } }
    time("只解码，帧间停 10 ms") { _ = hw.decode(path, frame); usleep(10_000) }
    time("只解码，帧间停 50 ms") { _ = hw.decode(path, frame); usleep(50_000) }
    // 8 条线程空转占 CPU
    var stop = false
    let busy = DispatchGroup()
    for _ in 0..<8 { DispatchQueue.global(qos: .userInitiated).async(group: busy) { var x = 0.0; while !stop { x += 1e-9 }; _ = x } }
    time("只解码，8 条线程占着 CPU") { _ = hw.decode(path, frame) }
    time("解码 + GPU 缩放，8 条线程占着 CPU") { if let pb = hw.decode(path, frame) { _ = shrinkAndOrient(pb, to: tw, th, orientation: frame.orientation, method: "gpu", gpu: gpu) } }
    stop = true
    busy.wait()
    // 8 路软件解码同时进行（像分析流水线那样）
    var stop2 = false
    let sw = DispatchGroup()
    for _ in 0..<8 { DispatchQueue.global(qos: .userInitiated).async(group: sw) { while !stop2 { _ = decodeScaled(path, 1024, managed: false) } } }
    time("只解码，8 路软件解码同时进行（GCD）") { _ = hw.decode(path, frame) }
    stop2 = true
    sw.wait()
    var stop3 = false
    let pool = WorkerPool(threads: 8, name: "bench.sw", qos: .userInitiated)
    let done = DispatchGroup()
    for _ in 0..<8 { done.enter(); pool.async { while !stop3 { _ = decodeScaled(path, 1024, managed: false) }; done.leave() } }
    time("只解码，8 路软件解码同时进行（专用线程）") { _ = hw.decode(path, frame) }
    stop3 = true
    done.wait()
}

let args = CommandLine.arguments
guard args.count >= 2 else { fail("usage: fg-image info <in> | render <in> <out.jpg> <max> [sdr] | crop ... | serve [--par N] | bench-hw <jpeg> [n]") }
if args[1] == "bench-hw" {
    guard args.count >= 3 else { fail("bench-hw needs <jpeg>") }
    benchHardware(args[2], args.count > 3 ? Int(args[3]) ?? 20 : 20)
    exit(0)
}
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
