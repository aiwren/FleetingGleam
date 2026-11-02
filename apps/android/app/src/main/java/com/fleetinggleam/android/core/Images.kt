package com.fleetinggleam.android.core

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.BitmapRegionDecoder
import android.graphics.Matrix
import android.graphics.Rect
import android.os.Build
import androidx.exifinterface.media.ExifInterface
import uniffi.fleetinggleam.ImageHelper
import uniffi.fleetinggleam.Pixels
import java.io.File
import java.nio.ByteBuffer
import kotlin.math.roundToInt

/**
 * 核心库的系统解码器（`platform.rs` 的 `SystemImages`），Android 版：用 BitmapFactory 的 `inSampleSize` 按需缩放解码、
 * BitmapRegionDecoder 只解屏幕上那一块，所以两亿像素的照片分析时只解到约 1/8（约 3 MP）、放大查看时只解看得到的区域，
 * 不会整张（600 MB）进内存。顺带认识 HEIC / AVIF（Android 12+）。失败返回错误文字，成功返回空串。
 * 会从核心库的工作线程并发调用；没有共享状态。
 */
object AndroidImages : ImageHelper {
    /**
     * 相机 RAW：Android 的解码器（Skia）认 DNG，其它常见 RAW 取文件里内嵌的预览图。要报出来，核心列文件时才看得见它们：
     * 和同名 JPG 配成一张（RAW 作伴随文件，导出、回收、写回一起走），单独的 RAW 解不出来的照旧在探测时剔除。
     */
    private val RAW = setOf("dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef", "srw", "x3f", "3fr", "erf", "kdc", "mef", "mos", "mrw", "raw", "rwl", "iiq", "fff", "dcr", "k25", "rwz")
    private val EXT = setOf("jpg", "jpeg", "jpe", "png", "webp", "gif", "bmp", "heic", "heif") + RAW + if (Build.VERSION.SDK_INT >= 31) setOf("avif") else emptySet()

    override fun supports(ext: String): Boolean = ext.lowercase() in EXT

    /** EXIF 里的旋转角度（只管 0 / 90 / 180 / 270，镜像不管）。 */
    private fun rotationOf(path: String): Int = runCatching { ExifInterface(path).rotationDegrees }.getOrDefault(0)

    private fun bounds(path: String): BitmapFactory.Options = BitmapFactory.Options().apply { inJustDecodeBounds = true; BitmapFactory.decodeFile(path, this) }

    /** 最大的 2 的幂，使得长边缩完仍不小于 `max`（解码器按它跳过像素）。 */
    private fun sampleFor(longEdge: Int, max: Int): Int {
        var s = 1
        while (longEdge / (s * 2) >= max) s *= 2
        return s
    }

    override fun info(path: String): String {
        val b = bounds(path)
        if (b.outWidth <= 0 || b.outHeight <= 0) return ""
        return if (rotationOf(path) % 180 == 90) "${b.outHeight},${b.outWidth}" else "${b.outWidth},${b.outHeight}"
    }

    override fun render(path: String, out: String, max: UInt): String = runCatching {
        val b = bounds(path)
        if (b.outWidth <= 0 || b.outHeight <= 0) error("cannot decode $path")
        val opts = BitmapFactory.Options().apply { inSampleSize = sampleFor(maxOf(b.outWidth, b.outHeight), max.toInt()); inPreferredConfig = Bitmap.Config.ARGB_8888 }
        var bm = BitmapFactory.decodeFile(path, opts) ?: error("cannot decode $path")
        bm = shrink(upright(bm, rotationOf(path)), max.toInt())
        save(bm, out, 88)
        ""
    }.getOrElse { it.toString() }

    /**
     * 分析用的解码：直接把像素交给核心库（RGBA，ARGB_8888 位图在内存里的原样），不再压 JPEG 写盘、让核心库再解一遍。
     * 文件头只读一次，原图的显示尺寸跟着像素一起回去（核心库不用再问 `info`）。解不了给 null，核心库退回 `render`。
     */
    override fun decode(path: String, max: UInt): Pixels? = runCatching {
        val b = bounds(path)
        if (b.outWidth <= 0 || b.outHeight <= 0) return null
        val rot = rotationOf(path)
        val opts = BitmapFactory.Options().apply { inSampleSize = sampleFor(maxOf(b.outWidth, b.outHeight), max.toInt()); inPreferredConfig = Bitmap.Config.ARGB_8888 }
        var bm = BitmapFactory.decodeFile(path, opts) ?: return null
        bm = shrink(upright(bm, rot), max.toInt())
        // 16 位的（广色域 PNG、HDR 的 HEIC）解码器可能不听 inPreferredConfig：转成 8 位再拷
        if (bm.config != Bitmap.Config.ARGB_8888) bm = bm.copy(Bitmap.Config.ARGB_8888, false).also { bm.recycle() } ?: return null
        try {
            // 行尾有填充的位图不是紧排的 w*h*4，交回去核心库会读错：让它走老路
            if (bm.rowBytes != bm.width * 4) return null
            val rgba = ByteArray(bm.width * bm.height * 4)
            bm.copyPixelsToBuffer(ByteBuffer.wrap(rgba))
            val (fw, fh) = if (rot % 180 == 90) b.outHeight to b.outWidth else b.outWidth to b.outHeight
            Pixels(bm.width.toUInt(), bm.height.toUInt(), fw.toUInt(), fh.toUInt(), rgba)
        } finally {
            bm.recycle()
        }
    }.getOrNull()

    override fun crop(path: String, out: String, x: Double, y: Double, w: Double, h: Double, outW: UInt): String = runCatching {
        val rot = rotationOf(path)
        val b = bounds(path)
        if (b.outWidth <= 0 || b.outHeight <= 0) error("cannot decode $path")
        val (rw, rh) = b.outWidth to b.outHeight
        // 区域是摆正后的比例坐标：换算回文件里的（未旋转的）坐标
        val u = when (rot) {
            90 -> doubleArrayOf(y, 1 - x - w, h, w)
            180 -> doubleArrayOf(1 - x - w, 1 - y - h, w, h)
            270 -> doubleArrayOf(1 - y - h, x, h, w)
            else -> doubleArrayOf(x, y, w, h)
        }
        val rect = Rect((u[0] * rw).roundToInt(), (u[1] * rh).roundToInt(), ((u[0] + u[2]) * rw).roundToInt(), ((u[1] + u[3]) * rh).roundToInt())
        if (!rect.intersect(0, 0, rw, rh) || rect.isEmpty) error("empty region")
        // `outW` 是摆正后那块的宽：90 / 270 时对应文件里那块的高
        val uprightW = if (rot % 180 == 90) rect.height() else rect.width()
        val opts = BitmapFactory.Options().apply { inSampleSize = sampleFor(uprightW, outW.toInt()); inPreferredConfig = Bitmap.Config.ARGB_8888 }
        val dec = BitmapRegionDecoder.newInstance(path)
        val part = try { dec.decodeRegion(rect, opts) ?: error("cannot decode region") } finally { dec.recycle() }
        val bm = shrink(upright(part, rot), outW.toInt())
        save(bm, out, 90)
        ""
    }.getOrElse { it.toString() }

    private fun upright(bm: Bitmap, rot: Int): Bitmap {
        if (rot == 0) return bm
        val m = Matrix().apply { postRotate(rot.toFloat()) }
        val r = Bitmap.createBitmap(bm, 0, 0, bm.width, bm.height, m, true)
        if (r !== bm) bm.recycle()
        return r
    }

    private fun shrink(bm: Bitmap, max: Int): Bitmap {
        val long = maxOf(bm.width, bm.height)
        if (long <= max) return bm
        val s = max.toFloat() / long
        val r = Bitmap.createScaledBitmap(bm, (bm.width * s).roundToInt().coerceAtLeast(1), (bm.height * s).roundToInt().coerceAtLeast(1), true)
        if (r !== bm) bm.recycle()
        return r
    }

    /** 先写临时文件再改名：读的一方不会看到半张。 */
    private fun save(bm: Bitmap, out: String, quality: Int) {
        val tmp = File("$out.tmp")
        tmp.outputStream().use { bm.compress(Bitmap.CompressFormat.JPEG, quality, it) }
        bm.recycle()
        if (!tmp.renameTo(File(out))) { tmp.copyTo(File(out), overwrite = true); tmp.delete() }
    }
}
