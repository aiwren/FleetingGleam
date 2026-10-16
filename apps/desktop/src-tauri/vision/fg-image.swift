// fg-image: one-shot image helper for FleetingGleam, on top of the system's ImageIO (macOS 15+).
// It is how the app sees what the camera really recorded: HDR gain maps, wide colour, HEIC / AVIF / JPEG XL / TIFF / camera RAW.
//
//   fg-image info   <in> [fast]             -> one JSON line: {"w":..,"h":..,"hdr":bool}   (size as displayed, EXIF orientation applied; "fast" skips the HDR check)
//   fg-image crop   <in> <out.jpg> <fx> <fy> <fw> <fh> <outW> -> the region (fractions of the upright photo, top-left origin) at up to <outW> pixels wide, as a JPEG
//   fg-image render <in> <out.jpg> <max> [sdr] -> a JPEG whose longest side is at most <max>, upright; keeps the HDR gain map unless "sdr" is given
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

let args = CommandLine.arguments
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
