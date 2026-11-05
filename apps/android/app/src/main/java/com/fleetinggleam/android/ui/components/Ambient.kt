package com.fleetinggleam.android.ui.components

import android.graphics.Bitmap
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import coil3.request.allowHardware
import coil3.request.transformations
import coil3.size.Size
import coil3.transform.Transformation
import java.io.File
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.roundToInt
import kotlin.math.sqrt

/**
 * 照片的模糊副本当背景（大图的房间、单张的光晕、分析卡）。以前是把 480 px 的缩略图铺满、再用几十 dp 的 RenderEffect 模糊，
 * 每帧都在 GPU 上模糊一整屏（审查第二批）。现在请求 64 px 的小图，在小图上按同比缩小的半径模糊一次（CPU 上几十微秒，结果进 Coil 的内存缓存），
 * 再铺满放大：模糊过的图放大看不出像素，看起来和以前一样。
 *
 * `blur` 是屏幕上的模糊半径（和 `Modifier.blur` 同一个意思）；`zoom` 是照片在框里再放大多少（框外的裁掉）。
 */
@Composable
fun AmbientImage(path: String, blur: Dp, modifier: Modifier = Modifier, zoom: Float = 1f) {
    val ctx = LocalContext.current
    val radius = with(LocalDensity.current) { blur.toPx() }
    BoxWithConstraints(modifier) {
        val w = constraints.maxWidth; val h = constraints.maxHeight
        if (w <= 0 || h <= 0 || w == androidx.compose.ui.unit.Constraints.Infinity || h == androidx.compose.ui.unit.Constraints.Infinity) return@BoxWithConstraints
        // 换了一张（大图翻页）：新的出来之前先留着上一张的模糊图，不闪一下底色
        var last by remember { mutableStateOf<coil3.memory.MemoryCache.Key?>(null) }
        val req = remember(path, w, h, radius, zoom) {
            ImageRequest.Builder(ctx).data(File(path)).size(SMALL).allowHardware(false).transformations(SmallBlur(radius, w, h, zoom)).placeholderMemoryCacheKey(last).build()
        }
        AsyncImage(req, null, Modifier.fillMaxSize().then(if (zoom != 1f) Modifier.clipToBounds().graphicsLayer { scaleX = zoom; scaleY = zoom } else Modifier),
            contentScale = ContentScale.Crop, onSuccess = { last = it.result.memoryCacheKey })
    }
}

/**
 * 四边向外淡出到透明。照片身后那团颜色是一个矩形的模糊图，不淡出的话边是硬的，看起来像照片后面垫了一块色板
 *（单张布局里很明显——走查 2026-11-05）。`edge` 是每一边用来淡出的比例。
 */
fun Modifier.fadeEdges(edge: Float = 0.42f): Modifier = this
    .graphicsLayer { compositingStrategy = androidx.compose.ui.graphics.CompositingStrategy.Offscreen }
    .drawWithContent {
        drawContent()
        val stops = arrayOf(0f to androidx.compose.ui.graphics.Color.Transparent, edge to androidx.compose.ui.graphics.Color.Black, 1f - edge to androidx.compose.ui.graphics.Color.Black, 1f to androidx.compose.ui.graphics.Color.Transparent)
        drawRect(androidx.compose.ui.graphics.Brush.horizontalGradient(*stops), blendMode = androidx.compose.ui.graphics.BlendMode.DstIn)
        drawRect(androidx.compose.ui.graphics.Brush.verticalGradient(*stops), blendMode = androidx.compose.ui.graphics.BlendMode.DstIn)
    }

/** 小图的长边。 */
private const val SMALL = 64

/** 在长边 64 px 的小图上做高斯模糊：σ 按铺满（Crop）时的放大倍数从屏幕换算到小图。 */
private class SmallBlur(private val radiusPx: Float, private val w: Int, private val h: Int, private val zoom: Float) : Transformation() {
    override val cacheKey: String = "fg.smallBlur($radiusPx,$w,$h,$zoom)"

    override suspend fun transform(input: Bitmap, size: Size): Bitmap {
        val k = SMALL.toFloat() / max(input.width, input.height)
        val src = if (k < 1f) Bitmap.createScaledBitmap(input, max(1, (input.width * k).roundToInt()), max(1, (input.height * k).roundToInt()), true) else input
        // 铺满时小图一个像素在屏幕上占多大（Crop 取较大的比例，再乘框里的放大）
        val scale = max(w.toFloat() / src.width, h.toFloat() / src.height) * zoom
        // Skia 的模糊半径换成 σ 的算法（RenderEffect 也是这样换的），再换到小图的像素
        val sigma = (0.57735f * radiusPx + 0.5f) / scale
        return if (sigma < 0.3f) src else gaussian(src, sigma)
    }
}

/** 三遍盒式模糊近似高斯（边缘按最近的像素延伸，和 RenderEffect 的 Clamp 一样）。 */
private fun gaussian(src: Bitmap, sigma: Float): Bitmap {
    val bw = src.width; val bh = src.height
    val a = IntArray(bw * bh)
    src.getPixels(a, 0, bw, 0, 0, bw, bh)
    val b = IntArray(bw * bh)
    for (box in boxes(sigma)) {
        val r = (box - 1) / 2
        blurPass(a, b, bw, bh, r, horizontal = true)
        blurPass(b, a, bw, bh, r, horizontal = false)
    }
    return Bitmap.createBitmap(a, bw, bh, Bitmap.Config.ARGB_8888)
}

/** 三遍盒式模糊各自的宽度（奇数），合起来接近给定的 σ。 */
private fun boxes(sigma: Float, n: Int = 3): IntArray {
    val ideal = sqrt(12.0 * sigma * sigma / n + 1)
    var wl = floor(ideal).toInt()
    if (wl % 2 == 0) wl--
    val wu = wl + 2
    val m = ((12.0 * sigma * sigma - n * wl * wl - 4.0 * n * wl - 3.0 * n) / (-4.0 * wl - 4)).roundToInt()
    return IntArray(n) { if (it < m) wl else wu }
}

/** 一遍盒式模糊（横或竖），四个通道一起，越界取最近的像素。 */
private fun blurPass(from: IntArray, to: IntArray, w: Int, h: Int, r: Int, horizontal: Boolean) {
    if (r <= 0) { from.copyInto(to); return }
    val len = if (horizontal) w else h
    val lines = if (horizontal) h else w
    val div = 2 * r + 1
    for (line in 0 until lines) {
        fun at(i: Int): Int { val c = i.coerceIn(0, len - 1); return if (horizontal) from[line * w + c] else from[c * w + line] }
        var sa = 0; var sr = 0; var sg = 0; var sb = 0
        for (i in -r..r) { val p = at(i); sa += p ushr 24; sr += (p shr 16) and 0xFF; sg += (p shr 8) and 0xFF; sb += p and 0xFF }
        for (i in 0 until len) {
            val o = if (horizontal) line * w + i else i * w + line
            to[o] = ((sa / div) shl 24) or ((sr / div) shl 16) or ((sg / div) shl 8) or (sb / div)
            val add = at(i + r + 1); val sub = at(i - r)
            sa += (add ushr 24) - (sub ushr 24)
            sr += ((add shr 16) and 0xFF) - ((sub shr 16) and 0xFF)
            sg += ((add shr 8) and 0xFF) - ((sub shr 8) and 0xFF)
            sb += (add and 0xFF) - (sub and 0xFF)
        }
    }
}
