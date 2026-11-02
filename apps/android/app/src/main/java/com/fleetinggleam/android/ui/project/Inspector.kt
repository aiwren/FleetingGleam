package com.fleetinggleam.android.ui.project

import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import com.fleetinggleam.android.core.Session
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import com.fleetinggleam.android.core.Core
import com.fleetinggleam.android.core.Decision
import com.fleetinggleam.android.core.FileInfo
import com.fleetinggleam.android.core.I18n
import com.fleetinggleam.android.core.Photo
import com.fleetinggleam.android.core.args
import com.fleetinggleam.android.core.fmtBytes
import com.fleetinggleam.android.core.pct
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.components.InfoPop
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Motion
import com.fleetinggleam.android.ui.theme.Type
import java.io.File
import java.util.concurrent.ConcurrentHashMap

/** 1–5 颗星（桌面端 stars）。 */
fun stars(v: Float): Int = kotlin.math.ceil(v * 5).toInt().coerceIn(1, 5)

/**
 * 一张照片的检查器（桌面端右侧的 inspector）：判定（星、三个按钮、理由、几条数据），同组照片，详细数据，文件信息。
 * 手机上它在大图下面拉上来的面板里，和单张模式的照片下面。
 */
@Composable
fun InspectorBody(photo: Photo, decision: Decision?, mates: List<Photo>, onDecide: (String?) -> Unit, onSelect: (String) -> Unit, dark: Boolean = false, stars: Int = 0, onRate: ((Int?) -> Unit)? = null) {
    val c = LocalFg.current
    // 区块之间 16（设计审查 A10.5）；详细数据的摘要带名字、不再列一遍"综合"（A10.2）；人脸也是一行名字 · 值（A10.3）
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        PhotoVerdict(photo, decision, onDecide, stars, onRate)
        if (mates.size > 1) Mates(photo, mates, onSelect)
        Column(Modifier.clip(RoundedCornerShape(14.dp)).background(c.panel).then(if (c.dark) Modifier.border(1.dp, c.line, RoundedCornerShape(14.dp)) else Modifier)) {
            Accordion("M2.5 12.5V8.5M6.5 12.5V4M10.5 12.5V6.5M14 12.5H1.5", t("inspector.details"),
                "${t("inspector.local")} ${photo.clipScore?.let { pct(it).toString() } ?: "—"} · ${t("inspector.system")} ${photo.sysScore?.let { "%.2f".format(it) } ?: "—"}") {
                Kv(buildList<Pair<String, Any>> {
                    add(t("inspector.local") to (photo.clipScore?.let { "${pct(it)}" } ?: "—"))
                    add(t("inspector.system") to (photo.sysScore?.let { "%.2f".format(it) } ?: "—"))
                    add(t("inspector.clip") to "%.1f%% / %.1f%%".format(photo.clippedHigh * 100, photo.clippedLow * 100))
                    if (photo.faceCount > 0) add(t("inspector.faces") to t("inspector.faceValue", "count" to photo.faceCount, "q" to (photo.faceMin?.let { "%.2f".format(it) } ?: "—")))
                }, mono = true)
            }
            Box(Modifier.fillMaxWidth().height(0.5.dp).background(c.line))
            FileSection(photo)
        }
    }
}

/**
 * 保留 / 待复查 / 淘汰三个钮，两端同一种选中态（设计审查 R13）：选中 = 实心（保留琥珀底、其余墨色底），未选 = surface 底、不描边；44dp、圆角 10。
 * 再点一次自己标的 = 交还系统的建议。检查器、单张共用；大图里是玻璃胶囊。
 */
@Composable
fun DecideTrio(decision: Decision?, onDecide: (String?) -> Unit, modifier: Modifier = Modifier) {
    val c = LocalFg.current
    val state = decision?.state ?: "review"
    val manual = decision?.manual == true
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        listOf("keep", "review", "reject").forEach { st ->
            val on = state == st
            val bg by androidx.compose.animation.animateColorAsState(if (on) (if (st == "keep") c.gleam else c.ink) else c.surface, Motion.fast(), label = "trioBg")
            val fg = if (on) (if (st == "keep") c.gleamInk else c.onInk) else c.ink
            Row(
                Modifier.weight(1f).height(44.dp).clip(RoundedCornerShape(10.dp)).background(bg)
                    .clickable { onDecide(if (on && manual) null else st) },
                horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically,
            ) {
                StateIcon(st, fg, 13.dp)
                Text(t("state.$st"), style = Type.label.copy(color = fg, fontWeight = FontWeight.SemiBold), modifier = Modifier.padding(start = 6.dp), maxLines = 1)
            }
        }
    }
}

/** 判定：名字、星、保留 / 待复查 / 淘汰、理由、几条数据条（桌面端 PhotoVerdict）。 */
/** 判定（设计审查 A10.1 / R2）：名字、综合分（24sp 等宽数字，不画星）+ "?"、三个判定钮、理由、几条数据条。 */
@Composable
fun PhotoVerdict(photo: Photo, decision: Decision?, onDecide: (String?) -> Unit, stars: Int = 0, onRate: ((Int?) -> Unit)? = null) {
    val c = LocalFg.current
    val reasons = decision?.reasons?.map { reasonText(it) }?.joinToString(if (I18n.isZh) "；" else "; ")
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(photo.name, style = Type.mono.copy(color = c.ink), maxLines = 1, modifier = Modifier.weight(1f, fill = false))
            if (photo.kind.isNotEmpty() && I18n.has("kind.${photo.kind}")) Text(t("kind.${photo.kind}"), style = Type.monoSmall.copy(color = c.ash),
                modifier = Modifier.padding(start = 8.dp).clip(CircleShape).border(1.dp, c.line, CircleShape).padding(horizontal = 8.dp, vertical = 2.dp))
            // 格式标签（"JPG + RAW"、"RAW · DNG"、"HEIC"…），和格子左上角的一样
            photoTags(photo).filter { (id, text) -> text != null && (id == "raw" || id == "format") }.forEach { (_, text) ->
                Text(text!!, style = Type.monoSmall.copy(color = c.ash), maxLines = 1,
                    modifier = Modifier.padding(start = 6.dp).clip(CircleShape).border(1.dp, c.line, CircleShape).padding(horizontal = 8.dp, vertical = 2.dp))
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("${pct(photo.score)}", style = Type.score.copy(color = c.ink))
            Text(t("inspector.overall"), style = Type.small.copy(color = c.ash), modifier = Modifier.padding(start = 8.dp))
            InfoPop(t("inspector.overall"), t("inspector.scoreTip"))
        }
        DecideTrio(decision, onDecide, Modifier.padding(top = 4.dp))
        // 手动星级：一行五颗可点的星，和保留 / 淘汰互不影响
        if (onRate != null) Row(verticalAlignment = Alignment.CenterVertically) {
            Text(t("stars.label"), style = Type.rowDesc.copy(color = c.ash), modifier = Modifier.width(76.dp))
            RateStars(stars, onRate, Modifier.padding(start = 0.dp), star = 18.dp, hit = 36.dp)
        }
        if (!reasons.isNullOrEmpty()) Text(reasons, style = Type.rowDesc.copy(color = c.ash))
        Column(Modifier.padding(top = 4.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            MBar(t("inspector.sharp"), pct(photo.sharpScore))
            MBar(t("inspector.expo"), pct(photo.expoScore))
            photo.aiScore?.let { MBar(t("inspector.ai"), pct(it)) }
            photo.prefScore?.let { MBar(t("inspector.pref"), pct(it)) }
        }
    }
}

/** 一条数据：名字、一根长度随分数长出来的条、数字。 */
@Composable
private fun MBar(label: String, value: Int) {
    val c = LocalFg.current
    val a = remember { Animatable(0f) }
    LaunchedEffect(value) { a.animateTo(maxOf(2, value) / 100f, Motion.soft()) }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(label, style = Type.rowDesc.copy(color = c.ash), modifier = Modifier.width(76.dp), maxLines = 1)
        Box(Modifier.weight(1f).height(3.dp).clip(CircleShape).background(c.surface2)) {
            Box(Modifier.fillMaxHeight().fillMaxWidth(a.value).clip(CircleShape).background(c.ink))
        }
        Text("$value", style = Type.monoSmall.copy(color = c.ink), modifier = Modifier.width(34.dp).padding(start = 8.dp))
    }
}

/** 同组照片：一排小图，最好的那张带一个金点，点一下换过去。 */
@Composable
private fun Mates(photo: Photo, mates: List<Photo>, onSelect: (String) -> Unit) {
    val c = LocalFg.current
    Column {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(t("inspector.mates"), style = Type.overline.copy(color = c.ash))
            Text("${mates.size}", style = Type.monoSmall.copy(color = c.ash), modifier = Modifier.padding(start = 6.dp))
        }
        Row(Modifier.padding(top = 8.dp).horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            mates.forEach { g ->
                val cur = g.path == photo.path
                Box(Modifier.size(56.dp).clip(RoundedCornerShape(6.dp)).border(2.dp, if (cur) c.ink else Color.Transparent, RoundedCornerShape(6.dp))
                    .semantics { contentDescription = photoLabel(g.name, Session.decisions.value[g.path]?.state); selected = cur }.clickable { onSelect(g.path) }) {
                    AsyncImage(File(g.thumb), null, Modifier.size(56.dp).graphicsLayer { alpha = if (cur) 1f else 0.6f }, contentScale = ContentScale.Crop)
                    if (g.groupBest) Box(Modifier.align(Alignment.TopEnd).padding(4.dp).size(7.dp).clip(CircleShape).background(c.gleam))
                }
            }
        }
    }
}

/** 可以收起的一行（桌面端 Accordion）：整行是按钮，右边一句小结，箭头转过去，内容平滑展开。 */
@Composable
fun Accordion(icon: String, title: String, hint: String?, content: @Composable () -> Unit) {
    val c = LocalFg.current
    var open by remember { mutableStateOf(false) }
    val turn by animateFloatAsState(if (open) 90f else 0f, Motion.glide(), label = "chev")
    Column {
        Row(Modifier.fillMaxWidth().clickable { open = !open }.padding(horizontal = 14.dp, vertical = 13.dp), verticalAlignment = Alignment.CenterVertically) {
            FgIcon(icon, size = 15.dp, stroke = 1.3f, color = c.ash)
            Text(title, style = Type.rowTitle.copy(color = c.ink), modifier = Modifier.weight(1f).padding(start = 10.dp))
            if (hint != null && !open) Text(hint, style = Type.monoSmall.copy(color = c.ash), maxLines = 1, modifier = Modifier.padding(end = 8.dp))
            FgIcon("M3 2.5L6.5 5 3 7.5", size = 10.dp, box = 10f, stroke = 1.6f, color = c.ash, modifier = Modifier.graphicsLayer { rotationZ = turn })
        }
        AnimatedVisibility(open, enter = expandVertically(Motion.glide()) + fadeIn(), exit = shrinkVertically(Motion.glide()) + fadeOut()) {
            Box(Modifier.padding(start = 14.dp, end = 14.dp, bottom = 14.dp)) { content() }
        }
    }
}

/** 对齐的两列（名字 · 值）。 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun Kv(rows: List<Pair<String, Any>>, mono: Boolean = false) {
    val c = LocalFg.current
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        rows.forEach { (k, v) ->
            Row(verticalAlignment = Alignment.Top) {
                Text(k, style = Type.rowDesc.copy(color = c.ash), modifier = Modifier.width(84.dp))
                when (v) {
                    is String -> Text(v, style = (if (mono) Type.mono else Type.rowDesc).copy(color = c.ink), modifier = Modifier.weight(1f))
                    is List<*> -> FlowRow(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(5.dp), verticalArrangement = Arrangement.spacedBy(5.dp)) {
                        v.forEach { x -> Text(x.toString(), style = Type.monoSmall.copy(color = c.ink), modifier = Modifier.clip(RoundedCornerShape(5.dp)).background(c.surface).padding(horizontal = 6.dp, vertical = 2.dp)) }
                    }
                    is Pair<*, *> -> Column(Modifier.weight(1f)) {
                        Text(v.first.toString(), style = Type.rowDesc.copy(color = c.ink))
                        v.second?.let { Text(it.toString(), style = Type.monoSmall.copy(color = c.ash)) }
                    }
                }
            }
        }
    }
}

private val infoCache = ConcurrentHashMap<String, FileInfo>()

@Composable
private fun rememberInfo(path: String): FileInfo? {
    var info by remember(path) { mutableStateOf(infoCache[path]) }
    LaunchedEffect(path) {
        if (info != null) return@LaunchedEffect
        runCatching { Core.get<FileInfo>("photo_info", args("path" to path)) }.getOrNull()?.let { infoCache[path] = it; info = it }
    }
    return info
}

/** EXIF 的日期写成 "2026:06:13 14:31:17"；按人写的样子显示，不要秒。 */
private fun niceTime(v: String) = v.replace(Regex("^(\\d{4}):(\\d{2}):(\\d{2})"), "$1-$2-$3").replace(Regex(":\\d{2}$"), "")

/** 曝光时间可能是很长的分数（"1/813.2157314956827 s"）：按人写的样子显示（"1/813 s"）。 */
private fun niceShutter(v: String): String {
    Regex("^1/(\\d+(?:\\.\\d+)?)\\s*s?$").find(v)?.let { return "1/${Math.round(it.groupValues[1].toDouble())} s" }
    Regex("^(\\d+(?:\\.\\d+)?)\\s*s?$").find(v)?.let { return "${"%.2f".format(it.groupValues[1].toDouble()).trimEnd('0').trimEnd('.')} s" }
    return v
}

/** 文件信息（桌面端 FileFacts / FileSection）：收起时右边写着格式和大小。 */
@Composable
fun FileSection(photo: Photo) {
    val info = rememberInfo(photo.path)
    val hint = info?.let { listOfNotNull(it.format, fmtBytes(it.bytes)).joinToString(" · ") }
    Accordion("M4 1.8h5l3 3V13a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V2.8a1 1 0 0 1 1-1zM9 1.8V5h3", t("file.title"), hint) {
        val mp = photo.width.toFloat() * photo.height / 1e6f
        val color = info?.colorSpace?.takeIf { it != "Uncalibrated" }
        val depth = info?.bitDepth?.let { "${it.split(",")[0].trim()} bit" }
        val tags = listOfNotNull(info?.focal, info?.aperture, info?.shutter?.let { niceShutter(it) }, info?.iso?.let { "ISO $it" })
        val rows = buildList<Pair<String, Any>> {
            listOfNotNull(info?.format, info?.let { fmtBytes(it.bytes) }).joinToString(" · ").takeIf { it.isNotEmpty() }?.let { add(t("file.format") to it) }
            add(t("file.pixels") to "${photo.width} × ${photo.height} · ${"%.1f".format(mp)} MP")
            listOfNotNull(color, depth).joinToString(" · ").takeIf { it.isNotEmpty() }?.let { add(t("file.color") to it) }
            (info?.camera ?: info?.lens)?.let { add(t("file.camera") to (it to info?.lens?.takeIf { _ -> info.camera != null })) }
            if (tags.isNotEmpty()) add(t("file.exposure") to tags)
            info?.taken?.let { add(t("file.taken") to niceTime(it)) }
            // RAW + JPG 配对：这张实际由哪几个文件组成
            if (photo.companions.isNotEmpty()) add(t("file.files") to (listOf(photo.path) + photo.companions).map { it.substringAfterLast('/') })
        }
        Kv(rows)
    }
}
