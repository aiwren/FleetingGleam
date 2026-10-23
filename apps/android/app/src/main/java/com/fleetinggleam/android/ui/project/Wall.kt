package com.fleetinggleam.android.ui.project

import android.graphics.RenderEffect
import android.graphics.Shader
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.keyframes
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.asAndroidPath
import androidx.compose.ui.graphics.asComposePath
import androidx.compose.ui.graphics.asComposeRenderEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.sp
import coil3.compose.AsyncImage
import com.fleetinggleam.android.core.Photo
import com.fleetinggleam.android.core.StageInfo
import com.fleetinggleam.android.core.pct
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.theme.GoldEnd
import com.fleetinggleam.android.ui.theme.GoldMid
import com.fleetinggleam.android.ui.theme.GoldTop
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Motion
import com.fleetinggleam.android.ui.theme.Type
import java.io.File

// ---------------------------------------------------------------- 排版：两端一样的两种布局

/** 照片墙上的一项：它的键（路径）和最终的宽高比（分析前从文件头读到，所以瓷砖从头到尾不变大小）。 */
/** 照片格子的圆角（瀑布与网格共用）：直角，像系统相册那样一张挨一张（作者 2026-10-23）。 */
val TILE_RADIUS = 0.dp

data class WallItem(val key: String, val ratio: Float)

/** 一行：哪些项、行高。 */
data class WallRow(val from: Int, val to: Int, val height: Float, val widths: List<Float>)

/** 格子之间的缝（dp）：作者 2026-10-23 嫌 6 偏宽，收到 2。 */
const val GAP = 2f

/** Justified rows: every row is as wide as the wall and every photo keeps its own aspect ratio, so nothing is cropped (the Mac's Wall.tsx). */
fun justify(items: List<WallItem>, width: Float, target: Float): List<WallRow> {
    val rows = mutableListOf<WallRow>()
    var from = 0
    fun close(to: Int, last: Boolean) {
        if (to <= from) return
        var s = 0f
        for (i in from until to) s += items[i].ratio
        var h = (width - GAP * (to - from - 1)) / s
        if (last && h > target * 1.35f) h = target // a short last row keeps the natural height instead of stretching
        rows.add(WallRow(from, to, h, (from until to).map { items[it].ratio * h }))
        from = to
    }
    var sum = 0f
    for (i in items.indices) {
        sum += items[i].ratio
        if ((width - GAP * (i - from)) / sum <= target) { close(i + 1, false); sum = 0f }
    }
    close(items.size, true)
    return rows
}

/** 双指缩放的几档（作者 2026-10-23："像手机相册一样缩放调整网格大小"）：网格每行几张、瀑布的目标行高（dp）。档位越大照片越大。 */
val GRID_COLS = listOf(7, 5, 4, 3, 2)
val JUSTIFY_HEIGHTS = listOf(80f, 110f, 150f, 200f, 270f)
const val ZOOM_GRID_DEFAULT = 3
const val ZOOM_JUSTIFY_DEFAULT = 2

/** Uniform square cells (photos are cropped to fill them). `fixedCols` > 0 定死每行几张（缩放档位）。 */
fun grid(items: List<WallItem>, width: Float, cell: Float, fixedCols: Int = 0): List<WallRow> {
    val cols = if (fixedCols > 0) fixedCols else maxOf(2, Math.round(width / cell))
    val size = (width - GAP * (cols - 1)) / cols
    return items.indices.chunked(cols).map { idx -> WallRow(idx.first(), idx.last() + 1, size, idx.map { size }) }
}

/**
 * 照片墙：按行排好，只挂上屏幕附近的行（LazyColumn），几百张和几千张一样快。`tile(index, width, height)` 画一张。
 * 手机上目标行高 150dp（桌面端 200px）。
 */
@Composable
fun Wall(
    items: List<WallItem>,
    layout: String,
    state: LazyListState,
    contentPadding: PaddingValues,
    header: (@Composable () -> Unit)? = null,
    /** 选中的那张换了、又不是点出来的（撤销、单张里换的）：把它所在的那一行滚到眼前（已经在屏幕上就不动）。 */
    reveal: String? = null,
    /** 第 i 张的缩略图（有就预取：按滑动方向提前把后面约三屏解码进内存缓存，滑到时直接有图）。 */
    thumbOf: ((Int) -> String?)? = null,
    /** 缩放档位（`GRID_COLS` / `JUSTIFY_HEIGHTS` 的下标）；`onZoom` 不为空时双指捏合 / 张开换一档（+1 照片变大）。 */
    zoom: Int = -1,
    onZoom: ((Int) -> Unit)? = null,
    tile: @Composable (index: Int, w: Dp, h: Dp) -> Unit,
) {
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val density = LocalDensity.current
        val pad = with(density) { (contentPadding.calculateLeftPadding(androidx.compose.ui.unit.LayoutDirection.Ltr) + contentPadding.calculateRightPadding(androidx.compose.ui.unit.LayoutDirection.Ltr)).value }
        val width = maxWidth.value - pad
        val rows = remember(items, width, layout, zoom) {
            when {
                width <= 0f -> emptyList()
                layout == "grid" -> grid(items, width, 110f, if (zoom >= 0) GRID_COLS[zoom.coerceIn(0, GRID_COLS.size - 1)] else 0)
                else -> justify(items, width, if (zoom >= 0) JUSTIFY_HEIGHTS[zoom.coerceIn(0, JUSTIFY_HEIGHTS.size - 1)] else 150f)
            }
        }
        // 换档后让原来在最上面的那张照片还在最上面（行数变了，按行号滚会跳到别处）
        var anchor by remember { androidx.compose.runtime.mutableIntStateOf(-1) }
        val headOff0 = if (header != null) 1 else 0
        androidx.compose.runtime.LaunchedEffect(rows) {
            val a = anchor
            if (a < 0) return@LaunchedEffect
            anchor = -1
            val r = rows.indexOfFirst { a in it.from until it.to }
            if (r >= 0) state.scrollToItem(r + headOff0)
        }
        val haptic = androidx.compose.ui.platform.LocalHapticFeedback.current
        val zoomNow by androidx.compose.runtime.rememberUpdatedState(onZoom)
        val rowsNow by androidx.compose.runtime.rememberUpdatedState(rows)
        val pinch = if (onZoom == null) Modifier else Modifier.pointerInput(Unit) {
            // 两根手指时自己处理（在 Initial 阶段先拿到、吃掉，列表就不会跟着滚）；一根手指照常滚动
            awaitEachGesture {
                awaitFirstDown(requireUnconsumed = false)
                var acc = 1f
                do {
                    val ev = awaitPointerEvent(androidx.compose.ui.input.pointer.PointerEventPass.Initial)
                    if (ev.changes.count { it.pressed } >= 2) {
                        acc *= ev.calculateZoom()
                        val step = when { acc > 1.25f -> 1; acc < 0.8f -> -1; else -> 0 }
                        if (step != 0) {
                            val firstRow = rowsNow.getOrNull(state.firstVisibleItemIndex - headOff0)
                            anchor = firstRow?.from ?: anchor
                            zoomNow?.invoke(step)
                            haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.TextHandleMove)
                            acc = 1f
                        }
                        ev.changes.forEach { it.consume() }
                    }
                } while (ev.changes.any { it.pressed })
            }
        }
        androidx.compose.runtime.LaunchedEffect(reveal, rows) {
            val key = reveal ?: return@LaunchedEffect
            val r = rows.indexOfFirst { row -> (row.from until row.to).any { items[it].key == key } }
            if (r < 0) return@LaunchedEffect
            val idx = r + if (header != null) 1 else 0
            val visible = state.layoutInfo.visibleItemsInfo
            val inView = visible.firstOrNull { it.index == idx }?.let { it.offset >= 0 && it.offset + it.size <= state.layoutInfo.viewportEndOffset - 120 } == true
            if (!inView) state.animateScrollToItem(idx, -200)
        }
        // 预取（作者 2026-10-23：两千张的相册快速滑动时格子是白的，停下才出图）：格子滑进屏幕时才去磁盘读、解码缩略图，
        // 快速滑动时一屏几十张同时排队就跟不上。这里按滑动方向把后面 18 行（约三屏）提前解码进 Coil 的内存缓存，尺寸和格子一样
        if (thumbOf != null) {
            val ctx = androidx.compose.ui.platform.LocalContext.current
            val headOff = if (header != null) 1 else 0
            androidx.compose.runtime.LaunchedEffect(state, rows) {
                val loader = coil3.SingletonImageLoader.get(ctx)
                val done = HashSet<Int>()
                var lastFirst = -1
                androidx.compose.runtime.snapshotFlow { state.firstVisibleItemIndex to (state.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0) }.collect { (first, last) ->
                    val down = first >= lastFirst
                    lastFirst = first
                    val range = if (down) (last + 1)..(last + 18) else (first - 18) until first
                    for (li in range) {
                        val ri = li - headOff
                        val r = rows.getOrNull(ri) ?: continue
                        if (!done.add(ri)) continue
                        for (i in r.from until r.to) {
                            val th = thumbOf(i) ?: continue
                            val w = (r.widths[i - r.from] * density.density).toInt().coerceAtLeast(1)
                            val h = (r.height * density.density).toInt().coerceAtLeast(1)
                            loader.enqueue(coil3.request.ImageRequest.Builder(ctx).data(java.io.File(th)).size(w, h).build())
                        }
                    }
                }
            }
        }
        // 滑动中格子一有图就直接显示，"显影"入场只留给停下来时第一次出现的那些
        androidx.compose.runtime.CompositionLocalProvider(LocalWallScrolling provides state.isScrollInProgress) {
            LazyColumn(state = state, contentPadding = contentPadding, verticalArrangement = Arrangement.spacedBy(GAP.dp), modifier = Modifier.fillMaxSize().then(pinch)) {
                if (header != null) item(key = "head") { header() }
                items(rows, key = { "${items.getOrNull(it.from)?.key}" + "#${it.to - it.from}" }) { r ->
                    Row(horizontalArrangement = Arrangement.spacedBy(GAP.dp)) {
                        for (i in r.from until r.to) tile(i, r.widths[i - r.from].dp, r.height.dp)
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------- 照片自己的记号：logo 里的四角星

/** The app's own mark: the four-point gleam from the logo. Filled = keep, outline = maybe (24 视框，中心 12,12). */
private const val STAR = "M12,2 Q13.7,10.3 22,12 Q13.7,13.7 12,22 Q10.3,13.7 2,12 Q10.3,10.3 12,2Z"

@Composable
fun Star(filled: Boolean, size: Dp = 20.dp, color: Color = Color.White, glow: Boolean = filled, draw: Float = 1f) {
    val path = remember { PathParser().parsePathString(STAR).toPath() }
    val gold = Brush.linearGradient(listOf(GoldTop, GoldMid, GoldEnd), start = Offset(0.2f * 24, 0f), end = Offset(0.8f * 24, 24f))
    Canvas(Modifier.size(size)) {
        val k = this.size.minDimension / 24f
        scale(k, pivot = Offset.Zero) {
            if (filled) {
                if (glow) for (r in listOf(3.2f, 2.0f)) drawPath(path, GoldMid.copy(alpha = 0.18f), style = Stroke(width = r * 2, join = StrokeJoin.Round))
                drawPath(path, gold)
            } else {
                // 空心星：描边沿路径"画出来"（`draw` 0..1）
                val pm = android.graphics.PathMeasure(path.asAndroidPath(), true)
                val dst = android.graphics.Path()
                pm.getSegment(0f, pm.length * draw.coerceIn(0f, 1f), dst, true)
                drawPath(dst.asComposePath(), color, style = Stroke(width = 1.6f, join = StrokeJoin.Round, cap = StrokeCap.Round))
            }
        }
    }
}

// ---------------------------------------------------------------- 照片的入场：对焦

/** 照片墙正在滑动（格子据此跳过入场动画）。 */
val LocalWallScrolling = androidx.compose.runtime.compositionLocalOf { false }

/** 这次会话里已经出现过的图（换视图、滚回来时不再"对焦"一遍）。 */
private val seen = HashSet<String>()

/**
 * "Focus pull": a new photo arrives soft, slightly over-exposed and a touch large, then racks into focus (DESIGN.md §10.1).
 * `quick` (browsing finished photos) only fades in briefly. Missing thumbnails show as a quiet blank.
 */
@Composable
fun FocusImage(path: String, modifier: Modifier = Modifier, quick: Boolean = false, scale: ContentScale = ContentScale.Crop) {
    val c = LocalFg.current
    val scrolling = LocalWallScrolling.current
    // 滑动中进来的格子不播"显影"（等它显影完早滑过去了，看到的只是一块块白底）
    val first = remember(path) { path !in seen && !scrolling }
    val p = remember(path) { Animatable(if (first) 0f else 1f) }
    var loaded by remember(path) { mutableStateOf(!first) }
    var missing by remember(path) { mutableStateOf(false) }
    LaunchedEffect(loaded) {
        if (loaded && p.value < 1f) {
            seen.add(path)
            p.animateTo(1f, if (quick) Motion.fast() else Motion.develop())
        }
    }
    if (missing) { Box(modifier.background(c.surface2)); return }
    val v = p.value
    AsyncImage(
        File(path), null,
        modifier.graphicsLayer {
            alpha = if (quick) v else (v * 1.9f).coerceAtMost(1f)
            if (!quick && v < 1f) {
                val s = 1.09f - 0.09f * v
                scaleX = s; scaleY = s
                val blur = 18f * (1f - v)
                if (blur > 0.5f) renderEffect = RenderEffect.createBlurEffect(blur, blur, Shader.TileMode.CLAMP).asComposeRenderEffect()
            }
        },
        contentScale = scale,
        colorFilter = if (!quick && v < 1f) ColorFilter.colorMatrix(ColorMatrix().apply { val b = 1f + 0.55f * (1f - v); setToScale(b, b, b, 1f) }) else null,
        onSuccess = { loaded = true },
        onError = { missing = true },
    )
}

// ---------------------------------------------------------------- 一张照片

/** 照片额外的身份：RAW / HEIC 之类、实况或 HDR、长图（和桌面端 photoTags 一样的规则）。 */
private val RAW_EXT = setOf("dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef", "srw", "x3f", "3fr", "erf", "kdc", "mef", "mos", "mrw", "raw", "rwl", "iiq", "fff", "dcr", "k25", "rwz")
private val FORMAT_TAG = setOf("heic", "heif", "avif", "jxl", "tif", "tiff", "bmp", "gif")

fun photoTags(p: Photo): List<Pair<String, String?>> {
    val out = mutableListOf<Pair<String, String?>>()
    val e = p.path.substringAfterLast('.', "").lowercase()
    if (e in RAW_EXT) out.add("raw" to if (e == "dng") "RAW · DNG" else "RAW · ${e.uppercase()}")
    else if (e in FORMAT_TAG) out.add("format" to if (e == "tif") "TIFF" else e.uppercase())
    // Live and HDR are never both shown: a Live photo says "Live" (the more useful thing to know), otherwise HDR.
    if (p.motion) out.add("motion" to null) else if (p.hdr) out.add("hdr" to null)
    val r = p.ratio
    if (r >= 2.2f || r <= 1f / 2.2f) out.add("long" to null)
    return out
}

@Composable
fun PhotoTags(p: Photo, modifier: Modifier = Modifier) {
    val tags = photoTags(p)
    if (tags.isEmpty()) return
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        tags.forEach { (id, text) ->
            Row(Modifier.clip(RoundedCornerShape(4.dp)).background(Color(0xA3000000)).padding(horizontal = 6.dp, vertical = 2.dp), verticalAlignment = Alignment.CenterVertically) {
                if (id == "motion") Box(Modifier.size(5.dp).clip(CircleShape).background(Color.White).padding(end = 3.dp))
                Text(text ?: t("tag.$id"), style = Type.caption.copy(color = Color.White), modifier = Modifier.padding(start = if (id == "motion") 4.dp else 0.dp))
            }
        }
    }
}

/**
 * One photo (the Mac's Tile.tsx). Before analysis finishes it is a "developing" tile: the real thumbnail comes into focus stage by stage, a ring
 * shows the stage, a sweep passes over it; afterwards it is the finished photo. The same element is used throughout, so nothing jumps.
 *
 * Marks are made of light rather than badges: a kept photo is *lit* (gold gleam, warm halo), a maybe is a hollow gleam, a reject simply lets
 * the light fall away (an aperture closes for a moment, then the photo recedes — never greyed out, DESIGN.md §9).
 */
@Composable
fun Tile(
    path: String,
    w: Dp,
    h: Dp,
    photo: Photo?,
    stage: StageInfo?,
    state: String,
    selected: Boolean,
    reveal: Boolean,
    dim: Boolean,
    manual: Boolean,
    showScore: Boolean,
    index: Int,
    interactive: Boolean,
    /** 选中时在格子上放保留 / 淘汰的小坞（手机的照片墙把这些放在底栏，不放坞）。 */
    dock: Boolean = true,
    /** 从"还没判"到有判定也算一次变化（分析中第一次判出来：星星弹出来、淘汰收一下光圈）；分析完的墙上不算，免得整面墙一起动。 */
    stamp: Boolean = false,
    onSelect: (String) -> Unit,
    onOpen: (String) -> Unit,
    onDecide: (String, String?) -> Unit,
) {
    val c = LocalFg.current
    val src = remember { MutableInteractionSource() }
    // effects play when the user *changes* a photo's mark, not every time a view is re-drawn
    var prev by remember(path) { mutableStateOf(state) }
    var changedTo by remember(path) { mutableStateOf<String?>(null) }
    var changeN by remember(path) { mutableIntStateOf(0) }
    LaunchedEffect(state) {
        if ((prev.isNotEmpty() || stamp) && state.isNotEmpty() && prev != state) { changedTo = state; changeN++ }
        prev = state
    }
    val recede by animateFloatAsState(if (state == "reject" && dim) 1f else 0f, tween(Motion.SLOW, easing = Motion.easeOut), label = "recede")
    // 格子圆角见 TILE_RADIUS（现在是直角）
    val shape = RoundedCornerShape(TILE_RADIUS)
    val kept = photo != null && state == "keep"
    Box(
        Modifier
            .size(w, h)
            .graphicsLayer {
                // reject: smaller and lighter, keeps its colour
                val s = 1f - 0.04f * recede
                scaleX = s; scaleY = s
                alpha = 1f - 0.38f * recede
            }
            // 一个状态一种信号（设计审查 R3）：保留 = 星 + 1dp 琥珀描边，没有外圈光晕；待复查 = 空心星，不描边
            .clip(shape)
            .background(c.surface)
            .then(if (kept) Modifier.border(1.dp, c.gleam.copy(alpha = 0.55f), shape) else Modifier)
            .combinedClickable(interactionSource = src, indication = null, enabled = interactive, onClick = { onSelect(path) }, onDoubleClick = { onSelect(path); onOpen(path) }, onLongClick = { onSelect(path); onOpen(path) }),
    ) {
        if (photo != null) {
            FocusImage(photo.thumb, Modifier.fillMaxSize(), quick = interactive && !reveal)
        } else {
            Developing(path, stage)
        }
        if (photo != null) {
            // 格子上的覆盖层最多两个（R10）：格式标签只在 ≥ 160dp 的格子上；分数只在按评分排序时
            if (w >= 160.dp) PhotoTags(photo, Modifier.align(Alignment.TopStart).padding(8.dp))
            if (showScore) Text(
                "${pct(photo.score)}", style = Type.monoSmall.copy(color = Color.White),
                modifier = Modifier.align(Alignment.BottomStart).padding(8.dp).clip(RoundedCornerShape(4.dp)).background(Color(0xA3000000)).padding(horizontal = 6.dp, vertical = 2.dp),
            )
            Marks(state, changedTo, changeN, reveal, index, selected)
            if (selected && interactive) {
                FocusFrame()
                if (dock) Dock(state, manual, w, Modifier.align(Alignment.BottomCenter).padding(bottom = if (w < 150.dp) 6.dp else 9.dp), onKeep = { onDecide(path, if (state == "keep" && manual) null else "keep") }, onReject = { onDecide(path, if (state == "reject" && manual) null else "reject") }, onOpen = { onOpen(path) })
            }
        }
    }
}

/** 分析中：缩略图随阶段从模糊到清楚，环形进度、阶段名、文件名，一道扫过的光；没开始的那些只有扫光和名字。 */
/**
 * 分析中的格子（设计审查 A5.6 / R9）：没开始的只有底色，静止、没有字；进行中的是模糊渐清的缩略图加一个 32dp 的进度环，
 * 没有扫光、没有呼吸边、没有阶段名和文件名（那些在进度卡上）。同一时刻只有正在处理的格子在动。
 */
@Composable
private fun Developing(path: String, stage: StageInfo?) {
    val pctv by animateFloatAsState((stage?.pct ?: 0) / 100f, Motion.base(), label = "pct")
    Box(Modifier.fillMaxSize()) {
        stage?.thumb?.let { th ->
            val left = 1f - pctv
            AsyncImage(File(th), null, Modifier.fillMaxSize().graphicsLayer {
                val b = left * 20f
                if (b > 0.5f) renderEffect = RenderEffect.createBlurEffect(b, b, Shader.TileMode.CLAMP).asComposeRenderEffect()
            }, contentScale = ContentScale.Crop, colorFilter = ColorFilter.colorMatrix(ColorMatrix().apply { val k = 1f + left * 0.3f; setToScale(k, k, k, 1f) }))
        }
        if (stage != null) {
            Canvas(Modifier.align(Alignment.Center).size(32.dp)) {
                val sw = 2.5.dp.toPx()
                drawCircle(Color.White.copy(alpha = 0.28f), radius = size.minDimension / 2 - sw, style = Stroke(sw))
                drawArc(Color.White, -90f, 360f * pctv, false, topLeft = Offset(sw, sw), size = Size(size.width - 2 * sw, size.height - 2 * sw), style = Stroke(sw, cap = StrokeCap.Round))
            }
        }
    }
}

/** 记号：保留是点亮的金星（再有一道横向的光和一团光晕），待复查是空心星，淘汰没有图标，只有一下收拢的光圈。 */
@Composable
private fun androidx.compose.foundation.layout.BoxScope.Marks(state: String, changedTo: String?, changeN: Int, reveal: Boolean, index: Int, selected: Boolean) {
    val play = { s: String -> (changedTo == s && changeN > 0) || reveal }
    val delayMs = if (reveal && changedTo == null) 300 + minOf(index, 40) * 55 else 0
    if (state == "keep") {
        // 星的出现：缩放 0.6 → 1、淡入，tap 弹簧，不转、不弹跳（设计审查 R7）
        val pop = remember(changeN, reveal) { Animatable(if (play("keep")) 0f else 1f) }
        LaunchedEffect(changeN, reveal) { if (pop.value < 1f) { kotlinx.coroutines.delay(delayMs.toLong() + 50); pop.animateTo(1f, Motion.tap()) } }
        Box(Modifier.align(Alignment.TopEnd).padding(9.dp).graphicsLayer {
            val v = pop.value
            scaleX = 0.6f + 0.4f * v; scaleY = scaleX; alpha = v.coerceIn(0f, 1f)
        }) { Star(true, 20.dp) }
        if (play("keep")) Flare(changeN, reveal, delayMs)
    }
    if (state == "review") {
        val d = remember(changeN) { Animatable(if (changedTo == "review") 0f else 1f) }
        LaunchedEffect(changeN) { if (d.value < 1f) d.animateTo(1f, Motion.slow()) }
        Box(Modifier.align(Alignment.TopEnd).padding(9.dp)) { Star(false, 20.dp, color = Color.White.copy(alpha = 0.95f), draw = d.value) }
    }
    if (state == "reject" && changedTo == "reject" && changeN > 0) Iris(changeN)
}

/** 保留时的一道横向长光（像变形镜头的拉丝光）和星点处的一团光晕。 */
@Composable
private fun Flare(n: Int, reveal: Boolean, delayMs: Int) {
    val a = remember(n, reveal) { Animatable(0f) }
    LaunchedEffect(n, reveal) { kotlinx.coroutines.delay(delayMs.toLong()); a.animateTo(1f, Motion.slow()) }
    Canvas(Modifier.fillMaxSize()) {
        val v = a.value
        if (v <= 0f || v >= 1f) return@Canvas
        val y = 19.dp.toPx()
        val w = size.width * 0.72f * (0.6f + 0.9f * v)
        val x = -size.width * 0.85f + size.width * 2.45f * v
        val alpha = if (v < 0.14f) v / 0.14f else 1f - (v - 0.14f) / 0.86f
        drawRect(Brush.horizontalGradient(listOf(Color(0x00FFD678), Color(0xFFFFE9A6), Color.White, Color(0x00FFD678)), startX = x, endX = x + w), topLeft = Offset(x, y - 1.dp.toPx()), size = Size(w, 2.dp.toPx()), alpha = alpha)
        val bloom = 0.2f + 8.8f * v
        drawCircle(Brush.radialGradient(listOf(Color(0xF2FFE496), Color(0x00FFE496)), center = Offset(size.width - y, y), radius = 7.dp.toPx() * bloom), radius = 7.dp.toPx() * bloom, center = Offset(size.width - y, y), alpha = 0.95f * (1f - v))
    }
}

/** 淘汰时：一圈暗角从外向里一收，像光圈合上，然后照片退后。 */
@Composable
private fun Iris(n: Int) {
    val a = remember(n) { Animatable(0f) }
    LaunchedEffect(n) { a.animateTo(1f, Motion.slow()) }
    Canvas(Modifier.fillMaxSize()) {
        val v = a.value
        if (v >= 1f) return@Canvas
        val alpha = if (v < 0.32f) v / 0.32f else 1f - (v - 0.32f) / 0.68f
        val s = if (v < 0.32f) 1.55f - 0.55f * (v / 0.32f) else 1f - 0.1f * ((v - 0.32f) / 0.68f)
        val r = size.maxDimension * 0.8f * s
        drawRect(Brush.radialGradient(0.3f to Color(0x0006060A), 0.68f to Color(0xBD06060A), center = center, radius = r), alpha = alpha)
    }
}

/** 选中：四个角的对焦框，锁定时从外往里一收。 */
@Composable
private fun FocusFrame() {
    val a = remember { Animatable(0f) }
    LaunchedEffect(Unit) { a.animateTo(1f, spring(dampingRatio = 0.92f, stiffness = 380f)) }
    Canvas(Modifier.fillMaxSize()) {
        val inset = (-3f + 10f * a.value).dp.toPx()
        val l = 16.dp.toPx()
        val sw = 2.dp.toPx()
        val col = Color.White.copy(alpha = a.value.coerceIn(0f, 1f))
        val corners = listOf(Offset(inset, inset) to (1f to 1f), Offset(size.width - inset, inset) to (-1f to 1f), Offset(inset, size.height - inset) to (1f to -1f), Offset(size.width - inset, size.height - inset) to (-1f to -1f))
        for ((p, d) in corners) {
            drawLine(Color.Black.copy(alpha = 0.35f * a.value), p, Offset(p.x + l * d.first, p.y), sw + 2f, StrokeCap.Round)
            drawLine(Color.Black.copy(alpha = 0.35f * a.value), p, Offset(p.x, p.y + l * d.second), sw + 2f, StrokeCap.Round)
            drawLine(col, p, Offset(p.x + l * d.first, p.y), sw, StrokeCap.Round)
            drawLine(col, p, Offset(p.x, p.y + l * d.second), sw, StrokeCap.Round)
        }
    }
}

/** 选中的照片下方的玻璃小坞：留下（金星）/ 淘汰（叉）/ 放大。再点一次已有的手动记号就交还系统的建议。 */
@Composable
private fun Dock(state: String, manual: Boolean, tileW: Dp, modifier: Modifier, onKeep: () -> Unit, onReject: () -> Unit, onOpen: () -> Unit) {
    // 小格子用小一号的坞；很窄时不放"放大"（再点一下选中的照片就是放大）
    val small = tileW < 170.dp
    val btn = if (small) 28.dp else 34.dp
    val zoom = tileW >= 120.dp
    val c = LocalFg.current
    val a = remember { Animatable(0f) }
    LaunchedEffect(Unit) { a.animateTo(1f, spring(dampingRatio = 0.8f, stiffness = 380f)) }
    Row(
        modifier.graphicsLayer { alpha = a.value; translationY = (1f - a.value) * 10.dp.toPx(); scaleX = 0.94f + 0.06f * a.value; scaleY = scaleX }
            .clip(CircleShape).background(Color(0x8F101014)).border(1.dp, Color(0x2EFFFFFF), CircleShape).padding(3.dp),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        DockBtn(btn, on = state == "keep", tint = c.gleam, label = if (state == "keep" && manual) t("mark.clear") else t("mark.keepAct"), onClick = onKeep) { Star(true, if (small) 14.dp else 16.dp) }
        DockBtn(btn, on = state == "reject", tint = Color.White, label = if (state == "reject" && manual) t("mark.clear") else t("mark.rejectAct"), onClick = onReject) { FgIcon("M3 3l6 6M9 3L3 9", size = if (small) 10.dp else 12.dp, box = 12f, stroke = 1.7f, color = Color.White) }
        if (zoom) DockBtn(btn, on = false, tint = Color.White, label = t("inspector.zoomLabel"), onClick = onOpen) { FgIcon("M9.5 2.5h4v4M6.5 13.5h-4v-4M13.5 2.5L9 7M2.5 13.5L7 9", size = if (small) 11.dp else 13.dp, stroke = 1.5f, color = Color.White) }
    }
}

@Composable
private fun DockBtn(size: Dp, on: Boolean, tint: Color, label: String, onClick: () -> Unit, icon: @Composable () -> Unit) {
    Box(
        Modifier.size(size).semantics { contentDescription = label }.clip(CircleShape).background(if (on) tint.copy(alpha = 0.26f) else Color.Transparent).clickable(onClick = onClick)
            .then(Modifier.drawWithContent { drawContent() }),
        contentAlignment = Alignment.Center,
    ) { icon() }
}
