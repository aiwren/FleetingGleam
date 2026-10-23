package com.fleetinggleam.android.ui.project

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import com.fleetinggleam.android.core.Progress
import com.fleetinggleam.android.core.Session
import com.fleetinggleam.android.core.duration
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgButton
import com.fleetinggleam.android.ui.components.Ticker
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Motion
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.io.File
import com.fleetinggleam.android.ui.components.LiveBar
import androidx.compose.ui.layout.ContentScale
import coil3.compose.AsyncImage

/**
 * 分析中这一屏（作者 2026-10-22：不想等到最后才能看，分析的时候就把好的和要淘汰的放到对应的位置）：
 * 和分析完一样的页签与照片墙。"全部"里每张照片在自己的格子里显影，一出结果就盖上判定（保留的星星弹出来、淘汰的收一下光圈），
 * 同时一粒光（保留是琥珀四角星，待复查是灰色空心星）从格子升起、飘进上面对应的页签后消散，页签上的数字像计数器一样滚动；
 * 淘汰不飞，只有格子上的光圈收拢。"保留 / 待复查 / 淘汰"三页就是眼下判成那样的照片，可以点开放大看。
 * 判定来自引擎的 "scan-decide"：每隔一会儿把已经出结果的照片整体分组、取舍一遍——后来的照片会改变分组和排名，
 * 所以一张照片可能从保留挪到淘汰（同一组里来了更好的一张），分析完换成正式的判定。
 * 暗色进度卡放在墙的最上面随墙滚走；滚走之后页签下面浮出一条窄胶囊（已分析 / 总数、还要多久，底边一线琥珀进度），点它回到顶上。
 * （作者 2026-10-23：之前页签下常驻一条细进度，和卡片里的重复，而且是按深色卡配的色，放在白底上很脏——去掉了。）
 */
/** 分析中的页签数据：全部 = 文件数，其余按眼下的判定数；"保留 / 待复查 / 淘汰"页是眼下判成那样的照片。每个分析事件后重算。 */
@Composable
fun rememberScanData(cull: CullState): CullData {
    val files by Session.files.collectAsState()
    Session.tick.collectAsState().value
    val status = HashMap<String, Int>().apply {
        put("all", files.size); put("keep", 0); put("review", 0); put("reject", 0)
        for (f in files) Session.live[f.path]?.state?.let { put(it, (get(it) ?: 0) + 1) }
    }
    val mode = cull.mode
    val shown = if (mode == "all") emptyList() else files.mapNotNull { Session.results[it.path] }.filter { Session.live[it.path]?.state == mode }
    return CullData(shown, status, emptyList(), emptyList(), Session.results, null)
}

/**
 * 分析中这一屏的内容层：暗色进度卡放在墙的最上面随墙滚走；"全部"里每张照片在自己的格子里显影，一出结果就盖上判定。
 * 页签和（卡滚走后的）窄胶囊在项目页的玻璃顶栏里，飘向页签的光点在 `JudgeFlights`。
 */
@Composable
fun ScanningStage(cull: CullState, data: CullData, progress: Progress, onStop: () -> Unit, topPad: Dp, bottomPad: Dp) {
    val c = LocalFg.current
    val files by Session.files.collectAsState()
    val mode = cull.mode
    val shown = data.shown
    val open: (String) -> Unit = remember(cull) { { path -> cull.select(path, byTap = true); cull.viewer = true } }
    var cardHeight by remember { mutableStateOf(0) }
    AnimatedContent(targetState = mode, label = "scanmode", transitionSpec = modeTransition, modifier = Modifier.fillMaxSize().swipeModes(cull)) { m ->
        val state = rememberLazyListState()
        LaunchedEffect(state) {
            cull.scanBackToTop = { state.animateScrollToItem(0) }
            androidx.compose.runtime.snapshotFlow { state.firstVisibleItemIndex > 0 || (cardHeight > 0 && state.firstVisibleItemScrollOffset > cardHeight - 8) }.collect { cull.scanCardGone = it }
        }
        // 页边 16dp，和页签对齐（设计审查 R14 / A5.10）
        val pad = PaddingValues(start = 16.dp, end = 16.dp, top = topPad + 4.dp, bottom = bottomPad)
        val card: @Composable () -> Unit = { Box(Modifier.padding(top = 4.dp, bottom = 10.dp).onGloballyPositioned { cardHeight = it.size.height }) { ScanningCard(progress, onStop) } }
        if (m == "all") {
            val items = remember(files) { files.map { WallItem(it.path, it.width.toFloat() / it.height.coerceAtLeast(1)) } }
            Wall(items, "grid", state, pad, header = card) { i, w, h ->
                val path = items[i].key
                val done = Session.results[path]
                Box(Modifier.onGloballyPositioned { cull.coords[path] = it }) {
                    Tile(path, w, h, done, Session.stages[path], Session.live[path]?.state ?: "", selected = false, reveal = false, dim = false, manual = false, showScore = false,
                        index = i, interactive = done != null, dock = false, stamp = true, onSelect = open, onOpen = open, onDecide = { _, _ -> })
                }
            }
        } else {
            val items = shown.map { WallItem(it.path, it.ratio) }
            Wall(items, "grid", state, pad, header = {
                card()
                if (items.isEmpty()) Text(t("android.scan.landing"), style = Type.rowDesc.copy(color = c.ash), modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 28.dp), textAlign = androidx.compose.ui.text.style.TextAlign.Center)
            }) { i, w, h ->
                val p = shown[i]
                Box(Modifier.onGloballyPositioned { cull.coords[p.path] = it }) {
                    Tile(p.path, w, h, p, null, m, selected = false, reveal = false, dim = false, manual = false, showScore = false,
                        index = i, interactive = true, dock = false, onSelect = open, onOpen = open, onDecide = { _, _ -> })
                }
            }
        }
    }
}

/** 分析中看大图：只看眼下这一页里已经分析完的（打开那一刻的样子），可以放大、左右翻；改判等分析完。 */
@Composable
fun ScanPeek(cull: CullState) {
    val shown = remember {
        val mode = cull.mode
        Session.files.value.mapNotNull { Session.results[it.path] }.filter { mode == "all" || Session.live[it.path]?.state == mode }
    }
    if (shown.isEmpty()) { cull.viewer = false; return }
    val data = remember(shown) { CullData(shown, emptyMap(), emptyList(), emptyList(), shown.associateBy { it.path }, null) }
    Viewer(cull, data, emptyMap(), readOnly = true) { _, _, _ -> }
}

/** 暗色卡滚走后页签下的窄胶囊：已分析 / 总数、还要多久，底边一线进度；点它回到顶上。画在项目页的玻璃顶栏里。 */
@Composable
fun ScanPill(progress: Progress, onTap: () -> Unit) {
    val c = LocalFg.current
    val elapsed = (System.currentTimeMillis() - Session.startedAt) / 1000f
    val left = if (progress.done >= 4) maxOf(1, Math.round(elapsed / progress.done * (progress.total - progress.done))) else null
    val f by androidx.compose.animation.core.animateFloatAsState(if (progress.total > 0) progress.done.toFloat() / progress.total else 0f, tween(500, easing = Motion.easeOut), label = "pill")
    // 32dp 高：左边 `24 / 30`，右边还要多久；底边 2dp 墨色进度线（琥珀只给保留——设计审查 A5.9 / R1）
    Box(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 8.dp).fillMaxWidth().height(32.dp).clip(RoundedCornerShape(8.dp)).background(c.surface).clickable(onClick = onTap)) {
        Row(Modifier.fillMaxSize().padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("${progress.done} / ${progress.total}", style = Type.mono.copy(color = c.ink), maxLines = 1, modifier = Modifier.weight(1f))
            if (left != null) Text(t("android.cull.etaLeft", "time" to duration(left)), style = Type.mono.copy(color = c.ash), maxLines = 1, overflow = Ellipsis)
        }
        Box(Modifier.align(Alignment.BottomStart).fillMaxWidth(f).height(2.dp).background(c.ink))
    }
}

/** 一粒正在飘向页签的光。 */
private class Mote(val id: Long, val keep: Boolean, val from: Offset, val to: Offset) {
    val t = Animatable(0f)
}

/**
 * 分析中的"分拣"动效：一张照片判为保留 / 待复查时，一粒光（保留是琥珀四角星，待复查是灰色空心星）从格子上的星点升起、
 * 飘进上面对应的页签，到了就消散；淘汰不飞。只给屏幕上看得见的格子做，一批最多几粒、错开一点出发。盖在整页上面。
 * （作者 2026-10-23 嫌之前"飞小图"的做法差：标记在这个应用里一直是"光"，不是搬东西。）
 */
@Composable
fun JudgeFlights(cull: CullState) {
    val motes = remember { mutableStateListOf<Mote>() }
    val scope = rememberCoroutineScope()
    val density = androidx.compose.ui.platform.LocalDensity.current
    var origin by remember { mutableStateOf(Offset.Zero) }
    var seq by remember { mutableStateOf(0L) }
    LaunchedEffect(Unit) {
        Session.judged.collect { batch ->
            var wait = 0L
            // 诊断（作者 2026-10-23 说动效没了）：这一批来了几条、各因为什么没飞
            var noTile = 0; var noTab = 0; var above = 0; var flown = 0
            for ((path, state) in batch) {
                if (state == "reject") continue
                // 一批最多 6 粒、错开 60 ms（设计审查 A5.7）
                if (motes.size >= 6) break
                val from = cull.rect(path) ?: run { noTile++; null } ?: continue
                val to = cull.tabRect(state) ?: run { noTab++; null } ?: continue
                if (from.bottom < to.top || from.height <= 0f) { above++; continue }
                flown++
                // 起点是格子上星出现的位置（右上 19dp），终点是页签数字的左侧 6dp
                val inset = with(density) { 19.dp.toPx() }
                val m = Mote(++seq, state == "keep", Offset(from.right - inset, from.top + inset), Offset(to.left - with(density) { 6.dp.toPx() }, to.center.y))
                motes += m
                val d = wait
                wait += 60
                scope.launch {
                    delay(d)
                    m.t.animateTo(1f, Motion.slow())
                    motes -= m
                }
            }
            android.util.Log.i("FleetingGleam", "分拣动效：这批 ${batch.size} 条，飞了 $flown，格子不在屏上 $noTile，没找到页签 $noTab，格子在页签上方 $above，飞行中 ${motes.size}")
        }
    }
    Box(Modifier.fillMaxSize().onGloballyPositioned { origin = it.boundsInRoot().topLeft }) {
        for (m in motes) key(m.id) { MoteSprite(m, origin) }
    }
}

@Composable
private fun MoteSprite(m: Mote, origin: Offset) {
    val c = LocalFg.current
    val density = androidx.compose.ui.platform.LocalDensity.current
    val half = with(density) { 8.dp.toPx() }
    val lift = with(density) { 28.dp.toPx() }
    Box(
        Modifier.size(16.dp).graphicsLayer {
            val v = m.t.value
            // 直线飘过去，中途微微上扬；先亮起来，快到时淡出
            val x = m.from.x + (m.to.x - m.from.x) * v
            val y = m.from.y + (m.to.y - m.from.y) * v - kotlin.math.sin(v * Math.PI).toFloat() * lift
            translationX = x - half - origin.x; translationY = y - half - origin.y
            val s = 1f - 0.4f * v
            scaleX = s; scaleY = s
            alpha = when { v < 0.12f -> v / 0.12f; v > 0.72f -> 1f - (v - 0.72f) / 0.28f; else -> 1f }
        },
    ) {
        Star(m.keep, 16.dp, color = c.ash, glow = m.keep)
    }
}

/**
 * 分析中的卡片：正在分析的那张照片糊在底下（每秒换一次，不跟着每个事件抖），白字压上去——已分析 / 总数、预计还要多久、此刻在做哪张，
 * 底边一条带暖光的进度。只有这张卡跟着分析事件刷新，整页不用。
 */
/**
 * 分析中的卡片（设计审查 A5.1–A5.5）：三行——`24 / 30` 大数字 + 右侧还要多久；进度条；此刻在做哪一步。右上角一粒 ×（停止）。
 * 背景是正在分析的那张照片糊在底下，每 4 秒换一张、900 ms 交叉淡入，不再一直晃。阶段那一行最多 600 ms 更新一次，不显示文件名。
 * 只有这张卡跟着分析事件刷新，整页不用。
 */
@Composable
private fun ScanningCard(progress: com.fleetinggleam.android.core.Progress, onStop: () -> Unit) {
    var bg by remember { mutableStateOf<String?>(null) }
    var stage by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) {
        var n = 0
        while (true) {
            stage = Session.lastStage?.second
            if (n % 7 == 0) Session.lastStage?.let { Session.stages[it.first]?.thumb }?.let { bg = it }
            n++
            kotlinx.coroutines.delay(600)
        }
    }
    val elapsed = (System.currentTimeMillis() - Session.startedAt) / 1000f
    val left = if (progress.done >= 4) maxOf(1, Math.round(elapsed / progress.done * (progress.total - progress.done))) else null
    androidx.compose.runtime.CompositionLocalProvider(LocalFg provides com.fleetinggleam.android.ui.theme.Dark) {
        val c = LocalFg.current
        Box(Modifier.fillMaxWidth().height(112.dp).clip(RoundedCornerShape(16.dp)).background(androidx.compose.ui.graphics.Color(0xFF1C1C20))) {
            androidx.compose.animation.Crossfade(bg, animationSpec = Motion.develop(), label = "scanbg", modifier = Modifier.matchParentSize()) { th ->
                if (th != null) coil3.compose.AsyncImage(
                    java.io.File(th), null, contentScale = androidx.compose.ui.layout.ContentScale.Crop,
                    modifier = Modifier.fillMaxSize().blur(26.dp).graphicsLayer { alpha = 0.6f },
                )
            }
            Box(Modifier.matchParentSize().background(androidx.compose.ui.graphics.Brush.verticalGradient(listOf(androidx.compose.ui.graphics.Color(0x66000000), androidx.compose.ui.graphics.Color(0xB8000000)))))
            // 右上角：停止。白 14% 的圆、白色 ×
            Box(
                Modifier.align(Alignment.TopEnd).padding(10.dp).size(32.dp).clip(androidx.compose.foundation.shape.CircleShape)
                    .background(androidx.compose.ui.graphics.Color.White.copy(alpha = 0.14f)).clickable(onClick = onStop)
                    .semantics { contentDescription = t("cull.stop") },
                contentAlignment = Alignment.Center,
            ) { com.fleetinggleam.android.ui.components.FgIcon(com.fleetinggleam.android.ui.components.Icons.CLOSE, size = 14.dp, stroke = 1.6f, color = c.ink) }
            Column(Modifier.padding(16.dp)) {
                Row(verticalAlignment = Alignment.Bottom) {
                    Ticker(progress.done, Type.bigNumber.copy(color = c.ink))
                    Text(" / ${progress.total}", style = Type.mono.copy(color = c.ash), modifier = Modifier.padding(bottom = 4.dp).weight(1f))
                    if (left != null) Text(t("android.cull.etaLeft", "time" to duration(left)), style = Type.monoSmall.copy(color = c.ash), modifier = Modifier.padding(bottom = 4.dp, end = 40.dp), maxLines = 1)
                }
                LiveBar(if (progress.total > 0) progress.done.toFloat() / progress.total else 0f, Modifier.padding(top = 10.dp))
                // 系统还在"优化"这个应用的电量：切到后台很可能被停掉。给个入口（白字下划线，不用琥珀——R1），点一下是系统自己的确认框
                val ctx = androidx.compose.ui.platform.LocalContext.current
                var exempt by remember { mutableStateOf(com.fleetinggleam.android.core.Scans.backgroundAllowed(ctx)) }
                androidx.lifecycle.compose.LifecycleResumeEffect(Unit) { exempt = com.fleetinggleam.android.core.Scans.backgroundAllowed(ctx); onPauseOrDispose { } }
                Row(Modifier.padding(top = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(stage?.let { t("cull.stage.$it").takeIf { s -> !s.startsWith("cull.") } ?: it } ?: t("cull.loadingModel"), style = Type.monoSmall.copy(color = c.ash), maxLines = 1, overflow = Ellipsis, modifier = Modifier.weight(1f))
                    if (!exempt) Text(
                        t("android.cull.allowBackground"), style = Type.small.copy(color = c.ink.copy(alpha = 0.85f), textDecoration = androidx.compose.ui.text.style.TextDecoration.Underline),
                        modifier = Modifier.clickable { com.fleetinggleam.android.core.Scans.askBackground(ctx) }, maxLines = 1,
                    )
                }
            }
        }
    }
}
