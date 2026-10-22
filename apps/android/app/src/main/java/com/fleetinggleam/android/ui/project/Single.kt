package com.fleetinggleam.android.ui.project

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.util.lerp
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import com.fleetinggleam.android.core.Decision
import com.fleetinggleam.android.core.I18n
import com.fleetinggleam.android.core.Large
import com.fleetinggleam.android.core.Photo
import com.fleetinggleam.android.core.pct
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Motion
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch
import java.io.File
import kotlin.math.abs

/**
 * 一次一张（桌面端 Single.tsx 的手机版）：照片尽量大，下面是小图条和判定。左右滑翻页——旧的一张像书页那样转开、缩向远处，
 * 新的一张从另一边转进来正对你。手机上多了一种快速挑法：在照片上往上推 = 保留，往下拉 = 淘汰，然后自动到下一张。
 */
@Composable
fun SinglePager(shown: List<Photo>, decisions: Map<String, Decision>, cull: CullState, onDecide: (String, String?) -> Unit, onOpen: (String) -> Unit, bottomPad: Dp) {
    val c = LocalFg.current
    val scope = rememberCoroutineScope()
    val haptic = LocalHapticFeedback.current
    val start = shown.indexOfFirst { it.path == cull.selected }.coerceAtLeast(0)
    val pager = rememberPagerState(initialPage = start) { shown.size }
    // 翻页 → 选中：用最新的列表（以前闭包里是第一次的那份，列表一变页码就对到别的照片上，和下面那条来回跳）
    val shownNow by androidx.compose.runtime.rememberUpdatedState(shown)
    LaunchedEffect(pager) { snapshotFlow { pager.settledPage }.distinctUntilChanged().collect { i -> shownNow.getOrNull(i)?.let { if (it.path != cull.selected) cull.select(it.path) } } }
    // 选中 → 翻页：选中换了（小图条点的）就翻过去；只是列表变了（改判的那张离开了当前页签）就直接对准，不做动画；正在翻时不抢
    var lastSel by remember { mutableStateOf(cull.selected) }
    LaunchedEffect(cull.selected, shown) {
        if (pager.isScrollInProgress) return@LaunchedEffect
        val i = shown.indexOfFirst { it.path == cull.selected }
        if (i >= 0 && i != pager.currentPage) { if (cull.selected != lastSel) pager.animateScrollToPage(i) else pager.scrollToPage(i) }
        lastSel = cull.selected
    }
    val photo = shown.getOrNull(pager.currentPage) ?: return

    Column(Modifier.fillMaxSize().padding(bottom = bottomPad)) {
        Box(Modifier.weight(1f).fillMaxWidth()) {
            // 照片的颜色在它身后晕开
            AsyncImage(File(photo.thumb), null, Modifier.align(Alignment.Center).fillMaxSize(0.6f).blur(48.dp).graphicsLayer { alpha = 0.5f; scaleX = 1.5f; scaleY = 1.5f }, contentScale = ContentScale.Crop)
            HorizontalPager(pager, Modifier.fillMaxSize(), key = { shown[it].path }, beyondViewportPageCount = 1) { page ->
                val p = shown[page]
                val f = ((pager.currentPage - page) + pager.currentPageOffsetFraction).coerceIn(-1f, 1f)
                SwipeDecide(p, decisions[p.path], onDecide = { st ->
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    val d = decisions[p.path]
                    onDecide(p.path, if (d?.state == st && d.manual) null else st)
                    // 手势挑片：定了就到下一张
                    if (page < shown.size - 1) scope.launch { pager.animateScrollToPage(page + 1) }
                }, onOpen = { onOpen(p.path) }, cull = cull,
                    modifier = Modifier.graphicsLayer {
                        // 书页那样转开：转过一点角度、缩向远处，里面的照片往反方向漂一点（视差）
                        cameraDistance = 14f * density
                        rotationY = 22f * f
                        val s = 1f - 0.12f * abs(f)
                        scaleX = s; scaleY = s
                        alpha = 1f - 0.5f * abs(f)
                    })
            }
            Text("${pager.currentPage + 1} / ${shown.size}", style = Type.monoSmall.copy(color = c.ash), modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = 2.dp))
        }
        Filmstrip(shown, decisions, pager.currentPage) { path -> cull.select(path) }
        CompactVerdict(photo, decisions[photo.path], onDecide = { st -> onDecide(photo.path, st) })
    }
}

/** 一页：照片按比例放好；上推保留、下拉淘汰（拉的时候照片跟着手，出现星或 ×，过了一定距离松手就定下）。 */
@Composable
private fun SwipeDecide(p: Photo, d: Decision?, onDecide: (String) -> Unit, onOpen: () -> Unit, cull: CullState, modifier: Modifier) {
    val c = LocalFg.current
    val ctx = LocalContext.current
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    val dy = remember(p.path) { Animatable(0f) }
    var large by remember(p.path) { mutableStateOf(Large.ready(p.path)) }
    var sharp by remember(p.path) { mutableStateOf(false) }
    LaunchedEffect(p.path) { if (large == null) large = Large.get(p.path) }
    val limit = with(density) { 90.dp.toPx() }
    BoxWithConstraints(modifier.fillMaxSize().padding(start = 20.dp, end = 20.dp, top = 10.dp, bottom = 26.dp), contentAlignment = Alignment.Center) {
        val W = constraints.maxWidth.toFloat(); val H = constraints.maxHeight.toFloat()
        val fw = minOf(W, H * p.ratio); val fh = fw / p.ratio
        val k = (dy.value / limit).coerceIn(-1.4f, 1.4f)
        // 上推的星 / 下拉的 ×：越接近定下越清楚
        if (k < -0.05f) Column(Modifier.align(Alignment.BottomCenter).graphicsLayer { alpha = (-k).coerceAtMost(1f) }, horizontalAlignment = Alignment.CenterHorizontally) {
            Star(true, 34.dp, color = c.gleam)
            Text(t("mark.keepAct"), style = Type.label.copy(color = c.ink, fontWeight = FontWeight.SemiBold), modifier = Modifier.padding(top = 4.dp))
        }
        if (k > 0.05f) Column(Modifier.align(Alignment.TopCenter).graphicsLayer { alpha = k.coerceAtMost(1f) }, horizontalAlignment = Alignment.CenterHorizontally) {
            FgIcon("M3 3l6 6M9 3L3 9", size = 26.dp, box = 12f, stroke = 1.8f, color = c.ink)
            Text(t("mark.rejectAct"), style = Type.label.copy(color = c.ink, fontWeight = FontWeight.SemiBold), modifier = Modifier.padding(top = 4.dp))
        }
        val kept = d?.state == "keep"
        Box(
            Modifier.size(with(density) { fw.toDp() }, with(density) { fh.toDp() })
                .onGloballyPositioned { cull.coords[p.path] = it }
                .graphicsLayer {
                    translationY = dy.value
                    val s = 1f - 0.08f * abs(k).coerceAtMost(1f)
                    scaleX = s; scaleY = s
                    rotationZ = if (k > 0) 2.5f * k else 0f
                }
                .shadow(if (kept) 18.dp else 10.dp, RoundedCornerShape(12.dp), ambientColor = if (kept) c.gleam else Color.Black, spotColor = if (kept) c.gleam else Color.Black)
                .clip(RoundedCornerShape(12.dp))
                .border(if (kept) 1.dp else 0.dp, if (kept) c.gleam.copy(alpha = 0.6f) else Color.Transparent, RoundedCornerShape(12.dp))
                .clickable { onOpen() }
                .pointerInput(p.path) {
                    detectVerticalDragGestures(
                        onDragEnd = {
                            val v = dy.value
                            when {
                                v < -limit -> { scope.launch { dy.animateTo(-limit * 1.6f, Motion.glide()); dy.snapTo(0f) }; onDecide("keep") }
                                v > limit -> { scope.launch { dy.animateTo(limit * 1.6f, Motion.glide()); dy.snapTo(0f) }; onDecide("reject") }
                                else -> scope.launch { dy.animateTo(0f, Motion.tap()) }
                            }
                        },
                        onDragCancel = { scope.launch { dy.animateTo(0f, Motion.tap()) } },
                    ) { ch, d -> ch.consume(); scope.launch { dy.snapTo(dy.value + d) } }
                },
        ) {
            AsyncImage(File(p.thumb), null, Modifier.fillMaxSize(), contentScale = ContentScale.FillBounds)
            large?.let { src ->
                val req = remember(src) { ImageRequest.Builder(ctx).data(File(src)).size(2560, 2560).build() }
                AsyncImage(req, p.name, Modifier.fillMaxSize().graphicsLayer { alpha = if (sharp) 1f else 0f }, contentScale = ContentScale.FillBounds, onSuccess = { sharp = true })
            }
            PhotoTags(p, Modifier.align(Alignment.TopStart).padding(8.dp))
            if (kept) Box(Modifier.align(Alignment.TopEnd).padding(9.dp)) { Star(true, 20.dp) }
        }
    }
}

/** 下面的小图条（桌面端 Single 的 filmstrip）：当前的有一个指示，保留的带金点。 */
@Composable
fun Filmstrip(shown: List<Photo>, decisions: Map<String, Decision>, index: Int, onPick: (String) -> Unit) {
    val c = LocalFg.current
    val density = LocalDensity.current
    val state = rememberLazyListState(initialFirstVisibleItemIndex = maxOf(0, index - 3))
    LaunchedEffect(index) {
        val vw = state.layoutInfo.viewportSize.width
        state.animateScrollToItem(index, -(vw / 2 - with(density) { 28.dp.toPx() }).toInt())
    }
    LazyRow(state = state, contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.fillMaxWidth().height(64.dp).padding(vertical = 4.dp)) {
        itemsIndexed(shown, key = { _, p -> p.path }) { i, p ->
            val cur = i == index
            val st = decisions[p.path]?.state
            Box(
                Modifier.size(56.dp).graphicsLayer { val s = if (cur) 1f else 0.9f; scaleX = s; scaleY = s }
                    .clip(RoundedCornerShape(9.dp)).border(if (cur) 2.dp else 0.dp, if (cur) c.ink else Color.Transparent, RoundedCornerShape(9.dp))
                    .clickable { onPick(p.path) },
            ) {
                AsyncImage(File(p.thumb), null, Modifier.fillMaxSize().graphicsLayer { alpha = if (st == "reject") 0.5f else 1f }, contentScale = ContentScale.Crop)
                if (st == "keep") Box(Modifier.align(Alignment.TopEnd).padding(3.dp)) { Star(true, 11.dp, glow = false) }
            }
        }
    }
}

/** 单张下面的判定：星、分数、三个按钮、一行理由。 */
@Composable
private fun CompactVerdict(photo: Photo, d: Decision?, onDecide: (String?) -> Unit) {
    val c = LocalFg.current
    val state = d?.state ?: "review"
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            val n = stars(photo.score)
            (0 until 5).forEach { i -> Star(i < n, 15.dp, color = if (i < n) c.gleam else c.ash, glow = false) }
            Text("${pct(photo.score)}", style = Type.monoSmall.copy(color = c.ash), modifier = Modifier.padding(start = 6.dp))
            Text(d?.reasons?.joinToString(if (I18n.isZh) "；" else "; ") { reasonText(it) } ?: "", style = Type.rowDesc.copy(color = c.ash), maxLines = 1,
                overflow = com.fleetinggleam.android.ui.theme.Ellipsis, modifier = Modifier.weight(1f).padding(start = 10.dp))
        }
        Row(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf("keep", "review", "reject").forEach { st ->
                val on = state == st
                Row(
                    Modifier.weight(1f).height(40.dp).clip(RoundedCornerShape(12.dp))
                        .background(if (on) (if (st == "keep") c.gleam else c.ink) else c.surface)
                        .border(1.dp, if (on) Color.Transparent else c.line, RoundedCornerShape(12.dp))
                        .clickable { onDecide(if (on && d?.manual == true) null else st) },
                    horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically,
                ) {
                    val fg = if (on) (if (st == "keep") c.gleamInk else c.onInk) else c.ink
                    StateIcon(st, fg, 13.dp)
                    Text(t("state.$st"), style = Type.label.copy(color = fg, fontWeight = FontWeight.SemiBold), modifier = Modifier.padding(start = 6.dp), maxLines = 1)
                }
            }
        }
    }
}
