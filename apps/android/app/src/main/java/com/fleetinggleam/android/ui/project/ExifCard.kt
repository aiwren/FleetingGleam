package com.fleetinggleam.android.ui.project

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.fleetinggleam.android.core.FileInfo
import com.fleetinggleam.android.core.Photo
import com.fleetinggleam.android.core.fmtBytes
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type

/** 和核心 `imageio::RAW_EXT` 一样的 RAW 扩展名（判断角标用）。 */
private val RAW_EXT = setOf("dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef", "srw", "x3f", "3fr", "erf", "kdc", "mef", "mos", "mrw", "raw", "rwl", "iiq", "fff", "dcr", "k25", "rwz")

private fun extOf(path: String) = path.substringAfterLast('.', "").lowercase()

/** 卡片顶部的角标：只有 JPG / RAW + JPG / 只有 RAW（HEIC 等照写自己的格式）。 */
fun pairBadge(p: Photo): String {
    val e = extOf(p.path)
    val name = when (e) { "jpeg", "jpe" -> "JPG"; "tif" -> "TIFF"; else -> e.uppercase() }
    return when {
        e in RAW_EXT -> t("android.exif.rawOnly")
        p.companions.isNotEmpty() -> t("android.exif.rawPlus", "fmt" to name)
        else -> t("android.exif.only", "fmt" to name)
    }
}

/**
 * 完整 EXIF 卡（路线图 A7，参照调研里 LensLink 的右侧卡片）：角标、快门 / 光圈 / ISO / 焦距四格大字（缺的写 —），
 * 下面机身、镜头、时间与各项拍摄参数（有值才列），再是格式 / 像素 / 色彩，最后"包含文件"逐个列出名字和大小。
 * 检查器里放一张；查看器的"信息"浮卡也是它（`compact`：更窄，四格排成两行两列）。
 */
@Composable
fun ExifCard(photo: Photo, modifier: Modifier = Modifier, compact: Boolean = false) {
    val c = LocalFg.current
    val info = rememberInfo(photo.path)
    Column(
        modifier.clip(RoundedCornerShape(14.dp)).background(c.panel).then(if (c.dark) Modifier.border(1.dp, c.line, RoundedCornerShape(14.dp)) else Modifier)
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(t("file.title"), style = Type.rowTitle.copy(color = c.ink), modifier = Modifier.weight(1f))
            Text(pairBadge(photo), style = Type.monoSmall.copy(color = c.ink, fontWeight = FontWeight.Medium), maxLines = 1,
                modifier = Modifier.clip(CircleShape).border(1.dp, c.line, CircleShape).padding(horizontal = 8.dp, vertical = 2.dp))
        }
        ShotGrid(info, compact)
        val rows = exifRows(photo, info)
        if (rows.isNotEmpty()) Kv(rows)
        Files(photo, info)
    }
}

/** 快门 / 光圈 / ISO / 焦距：四格大字，窄的时候两行两列。 */
@Composable
private fun ShotGrid(info: FileInfo?, compact: Boolean) {
    val cells = listOf(
        t("android.exif.shutter") to info?.shutter?.let { niceShutter(it) },
        t("android.exif.aperture") to info?.aperture,
        "ISO" to info?.iso?.removePrefix("ISO")?.trim(),
        t("android.exif.focal") to info?.focal,
    )
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val perRow = if (compact || maxWidth < 300.dp) 2 else 4
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            cells.chunked(perRow).forEach { row ->
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    row.forEach { (label, value) -> Cell(label, value, Modifier.weight(1f)) }
                }
            }
        }
    }
}

@Composable
private fun Cell(label: String, value: String?, modifier: Modifier) {
    val c = LocalFg.current
    Column(modifier.clip(RoundedCornerShape(10.dp)).background(c.surface).padding(horizontal = 10.dp, vertical = 8.dp)) {
        Text(value?.takeIf { it.isNotBlank() } ?: "—", style = Type.mono.copy(fontSize = 15.sp, lineHeight = 20.sp, fontWeight = FontWeight.Medium, color = c.ink), maxLines = 1, overflow = Ellipsis)
        Text(label, style = Type.small.copy(color = c.ash), maxLines = 1)
    }
}

/** 机身、镜头、时间与各项参数（有值才列），再是格式 / 像素 / 色彩（原来"文件信息"里的几行）。 */
private fun exifRows(photo: Photo, info: FileInfo?): List<Pair<String, Any>> = buildList {
    info?.camera?.let { add(t("file.camera") to it) }
    info?.lens?.let { add(t("file.lens") to it) }
    info?.taken?.let { add(t("file.taken") to niceTime(it)) }
    info?.focal35?.let { add(t("android.exif.focal35") to it) }
    info?.exposureBias?.let { add(t("android.exif.bias") to it) }
    info?.whiteBalance?.let { add(t("android.exif.wb") to it) }
    info?.flash?.let { add(t("android.exif.flash") to it) }
    info?.metering?.let { add(t("android.exif.metering") to it) }
    info?.exposureProgram?.let { add(t("android.exif.program") to it) }
    listOfNotNull(info?.format, info?.let { fmtBytes(it.bytes) }).joinToString(" · ").takeIf { it.isNotEmpty() }?.let { add(t("file.format") to it) }
    val mp = photo.width.toFloat() * photo.height / 1e6f
    add(t("file.pixels") to "${photo.width} × ${photo.height} · ${"%.1f".format(mp)} MP")
    val color = info?.colorSpace?.takeIf { it != "Uncalibrated" }
    val depth = info?.bitDepth?.let { "${it.split(",")[0].trim()} bit" }
    listOfNotNull(color, depth).joinToString(" · ").takeIf { it.isNotEmpty() }?.let { add(t("file.color") to it) }
}

/** 包含文件：每个文件名 + 大小。核心给了 `files` 就用它（带大小），老核心按照片和伴随 RAW 列名字。 */
@Composable
private fun Files(photo: Photo, info: FileInfo?) {
    val c = LocalFg.current
    val files = info?.files?.takeIf { it.isNotEmpty() }?.map { it.path to it.bytes }
        ?: (listOf(photo.path to (info?.bytes ?: 0L)) + photo.companions.map { it to 0L })
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(t("file.files"), style = Type.overline.copy(color = c.ash))
        files.forEach { (path, bytes) ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(path.substringAfterLast('/'), style = Type.mono.copy(color = c.ink), maxLines = 1, overflow = Ellipsis, modifier = Modifier.weight(1f))
                if (bytes > 0) Text(fmtBytes(bytes), style = Type.monoSmall.copy(color = c.ash), modifier = Modifier.padding(start = 8.dp))
            }
        }
    }
}
