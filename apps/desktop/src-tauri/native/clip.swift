// The CLIP image encoder (a ViT: ViT-B/32 or ViT-L/14) on the GPU, with Metal Performance Shaders Graph.
//
// Analysis spends most of its time in this network. ONNX Runtime's CoreML provider refuses every node of the float16 models the app ships (so
// they ran on the CPU, ~500 ms a picture for ViT-L/14, with the GPU idle) and compiling them for CoreML takes minutes and gigabytes. Here the
// graph is built directly from the model's weights, which takes a moment, and runs on the GPU: the residual stream, layer norms, softmax and
// activation in float32, the matrix products in float16 (as the model's own weights are), which is also how the ONNX models are evaluated.
//
// The weights arrive as one buffer of float16 in a fixed order (see `fg_clip_create`); the Rust side (clipgpu.rs) reads them out of the ONNX file.
import Foundation
import Metal
import MetalPerformanceShadersGraph

/// Float16 bits to Float.
private func half(_ h: UInt16) -> Float { Float(Float16(bitPattern: h)) }

final class ClipGPU {
    let embedDim: Int
    /// The most pictures one run takes. The graph is built once for any number of pictures and compiled for 1 and for `maxBatch`: a run of 2 or 3
    /// is padded up to `maxBatch`, so callers should send one or `maxBatch`.
    let maxBatch: Int
    private let queue: MTLCommandQueue
    private let gdevice: MPSGraphDevice
    private let single: MPSGraphExecutable
    private let batched: MPSGraphExecutable
    private let lock = NSLock()

    /// `cfg`: [dim, tokens, patch, layers, heads, embed, mlp, max batch]. `w`: the weights, float16, `lens[i]` bytes each, in this order:
    /// patch conv [dim,3,patch,patch], class embedding [dim], position embedding [tokens,dim], pre-norm weight and bias [dim],
    /// then per layer: norm1 weight, bias; q weight [dim,dim], bias; k; v; out; norm2 weight, bias; fc1 [dim,mlp], bias; fc2 [mlp,dim], bias,
    /// then post-norm weight and bias [dim] and the projection [dim,embed]. Weights are [in, out], as the ONNX file has them.
    init?(cfg: [Int], w: UnsafePointer<UInt8>, lens: [Int]) {
        guard cfg.count >= 8, let device = MTLCreateSystemDefaultDevice(), let q = device.makeCommandQueue() else { return nil }
        let (D, N, P, L, H, E, M, B) = (cfg[0], cfg[1], cfg[2], cfg[3], cfg[4], cfg[5], cfg[6], max(2, cfg[7]))
        let hd = D / H
        let g = MPSGraph()
        var at = 0, idx = 0
        func shape(_ s: [Int]) -> [NSNumber] { s.map { NSNumber(value: $0) } }
        /// The next tensor of the buffer as a float16 constant.
        func take16(_ s: [Int]) -> MPSGraphTensor? {
            let n = s.reduce(1, *)
            guard idx < lens.count, lens[idx] == n * 2 else { return nil }
            let t = g.constant(Data(bytes: w + at, count: n * 2), shape: shape(s), dataType: .float16)
            at += n * 2; idx += 1
            return t
        }
        /// The next tensor as a float32 constant (small things: norms, biases, embeddings).
        func take32(_ s: [Int]) -> MPSGraphTensor? {
            let n = s.reduce(1, *)
            guard idx < lens.count, lens[idx] == n * 2 else { return nil }
            var f = [Float](repeating: 0, count: n)
            w.withMemoryRebound(to: UInt16.self, capacity: (at + n * 2) / 2) { p in
                let base = p + at / 2
                for i in 0..<n { f[i] = half(base[i]) }
            }
            at += n * 2; idx += 1
            return g.constant(f.withUnsafeBufferPointer { Data(buffer: $0) }, shape: shape(s), dataType: .float32)
        }
        func cast(_ t: MPSGraphTensor, _ ty: MPSDataType) -> MPSGraphTensor { g.cast(t, to: ty, name: nil) }
        func scalar(_ v: Double) -> MPSGraphTensor { g.constant(v, dataType: .float32) }
        let eps = scalar(1e-5)

        func norm(_ x: MPSGraphTensor, _ gamma: MPSGraphTensor, _ beta: MPSGraphTensor) -> MPSGraphTensor {
            let mean = g.mean(of: x, axes: [2], name: nil)
            let xc = g.subtraction(x, mean, name: nil)
            let v = g.mean(of: g.multiplication(xc, xc, name: nil), axes: [2], name: nil)
            let r = g.reciprocalSquareRoot(g.addition(v, eps, name: nil), name: nil)
            return g.addition(g.multiplication(g.multiplication(xc, r, name: nil), gamma, name: nil), beta, name: nil)
        }
        /// x [1,n,in] (float32) times W [in,out] (float16) plus bias: the product in float16, the rest in float32.
        func linear(_ x: MPSGraphTensor, _ wt: MPSGraphTensor, _ b: MPSGraphTensor?) -> MPSGraphTensor {
            let y = cast(g.matrixMultiplication(primary: cast(x, .float16), secondary: wt, name: nil), .float32)
            return b.map { g.addition(y, $0, name: nil) } ?? y
        }

        let input = g.placeholder(shape: shape([-1, 3, 224, 224]), dataType: .float32, name: "pixels")   // any number of pictures
        guard let convW = take16([D, 3, P, P]), let cls1 = take32([1, 1, D]), let pos = take32([1, N, D]), let preG = take32([D]), let preB = take32([D]) else { return nil }
        let cd = MPSGraphConvolution2DOpDescriptor(strideInX: P, strideInY: P, dilationRateInX: 1, dilationRateInY: 1, groups: 1,
                                                  paddingLeft: 0, paddingRight: 0, paddingTop: 0, paddingBottom: 0,
                                                  paddingStyle: .explicit, dataLayout: .NCHW, weightsLayout: .OIHW)!
        let grid = 224 / P
        let patches = cast(g.convolution2D(cast(input, .float16), weights: convW, descriptor: cd, name: nil), .float32)    // [B,D,grid,grid]
        let tokens = g.transposeTensor(g.reshape(patches, shape: shape([-1, D, grid * grid]), name: nil), dimension: 1, withDimension: 2, name: nil)
        // the class token for every picture: zeros the shape of one token per picture, plus the class embedding
        let zeros = g.multiplication(g.sliceTensor(tokens, dimension: 1, start: 0, length: 1, name: nil), scalar(0), name: nil)
        let cls = g.addition(zeros, cls1, name: nil)
        var x = g.addition(g.concatTensors([cls, tokens], dimension: 1, name: nil), pos, name: nil)
        x = norm(x, preG, preB)

        let scale = scalar(1.0 / Double(hd).squareRoot())
        let quick = scalar(1.702)
        for _ in 0..<L {
            guard let n1g = take32([D]), let n1b = take32([D]),
                  let wq = take16([D, D]), let bq = take32([D]), let wk = take16([D, D]), let bk = take32([D]),
                  let wv = take16([D, D]), let bv = take32([D]), let wo = take16([D, D]), let bo = take32([D]),
                  let n2g = take32([D]), let n2b = take32([D]),
                  let w1 = take16([D, M]), let b1 = take32([M]), let w2 = take16([M, D]), let b2 = take32([D]) else { return nil }
            let h = norm(x, n1g, n1b)
            func heads(_ t: MPSGraphTensor) -> MPSGraphTensor {   // [B,N,D] -> [B,H,N,hd]
                g.transposeTensor(g.reshape(t, shape: shape([-1, N, H, hd]), name: nil), dimension: 1, withDimension: 2, name: nil)
            }
            let q = heads(g.multiplication(linear(h, wq, bq), scale, name: nil))
            let k = heads(linear(h, wk, bk))
            let v = heads(linear(h, wv, bv))
            let kt = g.transposeTensor(k, dimension: 2, withDimension: 3, name: nil)                                    // [1,H,hd,N]
            let scores = cast(g.matrixMultiplication(primary: cast(q, .float16), secondary: cast(kt, .float16), name: nil), .float32)
            let probs = g.softMax(with: scores, axis: 3, name: nil)
            let ctx = g.matrixMultiplication(primary: cast(probs, .float16), secondary: cast(v, .float16), name: nil)  // [1,H,N,hd]
            let merged = g.reshape(g.transposeTensor(ctx, dimension: 1, withDimension: 2, name: nil), shape: shape([-1, N, D]), name: nil)
            x = g.addition(x, linear(cast(merged, .float32), wo, bo), name: nil)
            let h2 = norm(x, n2g, n2b)
            let a = linear(h2, w1, b1)
            let act = g.multiplication(a, g.sigmoid(with: g.multiplication(a, quick, name: nil), name: nil), name: nil)   // quick GELU
            x = g.addition(x, linear(act, w2, b2), name: nil)
        }
        guard let postG = take32([D]), let postB = take32([D]), let proj = take16([D, E]), idx == lens.count else { return nil }
        let first = g.sliceTensor(x, dimension: 1, start: 0, length: 1, name: nil)                                       // the class token
        let out = g.reshape(linear(norm(first, postG, postB), proj, nil), shape: shape([-1, E]), name: nil)

        let gdev = MPSGraphDevice(mtlDevice: device)
        func compile(_ n: Int) -> MPSGraphExecutable {
            g.compile(with: gdev, feeds: [input: MPSGraphShapedType(shape: shape([n, 3, 224, 224]), dataType: .float32)], targetTensors: [out], targetOperations: nil, compilationDescriptor: nil)
        }
        self.queue = q
        self.gdevice = gdev
        self.single = compile(1)
        self.batched = compile(B)
        self.embedDim = E
        self.maxBatch = B
    }

    /// `count` pictures (1 to `maxBatch`) of 3x224x224 floats, normalised the way CLIP wants. `out`: `count` (not yet normalised) embeddings.
    func embed(_ pixels: UnsafePointer<Float>, _ count: Int, _ out: UnsafeMutablePointer<Float>) -> Bool {
        guard count >= 1, count <= maxBatch else { return false }
        lock.lock(); defer { lock.unlock() }
        let one = 3 * 224 * 224
        let n = count == 1 ? 1 : maxBatch
        var data = Data(bytes: pixels, count: count * one * 4)
        if n > count { data.append(Data(count: (n - count) * one * 4)) }      // padding: pictures of zeros, their embeddings are not looked at
        let td = MPSGraphTensorData(device: gdevice, data: data, shape: [NSNumber(value: n), 3, 224, 224], dataType: .float32)
        let res = (n == 1 ? single : batched).run(with: queue, inputs: [td], results: nil, executionDescriptor: nil)
        guard let r = res.first else { return false }
        if n == count {
            r.mpsndarray().readBytes(out, strideBytes: nil)
        } else {
            var all = [Float](repeating: 0, count: n * embedDim)
            r.mpsndarray().readBytes(&all, strideBytes: nil)
            for i in 0..<(count * embedDim) { out[i] = all[i] }
        }
        return true
    }
}

/// Builds the encoder from its weights. `cfg`: dim, tokens, patch, layers, heads, embed, mlp, the most pictures one run takes (8 numbers). Returns null when the graph cannot be built here.
@_cdecl("fg_clip_create")
public func fg_clip_create(_ cfg: UnsafePointer<Int32>, _ weights: UnsafePointer<UInt8>, _ lens: UnsafePointer<Int64>, _ count: Int32) -> UnsafeMutableRawPointer? {
    let c = (0..<8).map { Int(cfg[$0]) }
    let l = (0..<Int(count)).map { Int(lens[$0]) }
    guard let m = ClipGPU(cfg: c, w: weights, lens: l) else { return nil }
    return Unmanaged.passRetained(m).toOpaque()
}

/// `count` pictures: 3x224x224 floats each in, `count` embeddings out. 1 on success.
@_cdecl("fg_clip_embed")
public func fg_clip_embed(_ h: UnsafeMutableRawPointer, _ pixels: UnsafePointer<Float>, _ count: Int32, _ out: UnsafeMutablePointer<Float>) -> Int32 {
    Unmanaged<ClipGPU>.fromOpaque(h).takeUnretainedValue().embed(pixels, Int(count), out) ? 1 : 0
}

@_cdecl("fg_clip_free")
public func fg_clip_free(_ h: UnsafeMutableRawPointer) {
    Unmanaged<ClipGPU>.fromOpaque(h).release()
}
