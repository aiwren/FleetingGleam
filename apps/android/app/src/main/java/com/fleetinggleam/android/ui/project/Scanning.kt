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
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import com.fleetinggleam.android.core.Progress
import com.fleetinggleam.android.core.Session
import com.fleetinggleam.android.core.duration
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgButton
import com.fleetinggleam.android.ui.components.LiveBar
import com.fleetinggleam.android.ui.components.Ticker
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Motion
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.io.File

/**
 * 分析中这一屏（作者 2026-10-22：不想等到最后才能看，分析的时候就把好的和要淘汰的放到对应的位置）：
 * 和分析完一样的页签与照片墙。"全部"里每张照片在自己的格子里显影，一出结果就盖上判定（保留的星星弹出来、淘汰的收一下光圈），
 * 同时一张小图从格子飞进上面对应的页签、页签上的数字跳一下；"保留 / 待复查 / 淘汰"三页就是眼下判成那样的照片，可以点开放大看。
 * 判定来自引擎的 "scan-decide"：每隔一会儿把已经出结果的照片整体分组、取舍一遍——后来的照片会改变分组和排名，
 * 所以一张照片可能从保留挪到淘汰（同一组里来了更好的一张），分析完换成正式的判定。
 * 暗色进度卡放在墙的最上面随墙滚走，页签下面一条细进度一直在。
 */
@Composable
fun ScanningStage(cull: CullState, progress: Progress, onStop: () -> Unit, bottomPad: Dp) {
    val c = LocalFg.current
    val files by Session.files.collectAsState()
    Session.tick.collectAsState().value
    // 页签上的数字：全部 = 文件数，其余按眼下的判定数
    val status = HashMap<String, Int>().apply {
        put("all", files.size); put("keep", 0); put("review", 0); put("reject", 0)
        for (f in files) Session.live[f.path]?.state?.let { put(it, (get(it) ?: 0) + 1) }
    }
    val mode = cull.mode
    val shown = if (mode == "all") emptyList() else files.mapNotNull { Session.results[it.path] }.filter { Session.live[it.path]?.state == mode }
    val data = CullData(shown, status, emptyList(), emptyList(), Session.results, null)
    val open: (String) -> Unit = remember(cull) { { path -> cull.select(path, byTap = true); cull.viewer = true } }
    Column(Modifier.fillMaxSize()) {
        StatusStrip(cull, data)
        LiveBar(if (progress.total > 0) progress.done.toFloat() / progress.total else 0f, Modifier.padding(start = 16.dp, end = 16.dp, bottom = 6.dp))
        AnimatedContent(targetState = mode, label = "scanmode", transitionSpec = modeTransition, modifier = Modifier.fillMaxSize().swipeModes(cull)) { m ->
            val state = rememberLazyListState()
            val pad = PaddingValues(start = 12.dp, end = 12.dp, top = 4.dp, bottom = bottomPad)
            val card: @Composable () -> Unit = { Box(Modifier.padding(start = 4.dp, end = 4.dp, top = 6.dp, bottom = 10.dp)) { ScanningCard(progress, onStop) } }
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

/** 一张正在飞向页签的小图。 */
private class Flight(val id: Long, val thumb: String, val from: Rect, val to: Rect) {
    val t = Animatable(0f)
}

/**
 * 分析中的"分拣"动效：一张照片判出来（或改判）时，它的小图从格子沿一道弧线飞进上面对应的页签，落下时页签的数字跳一下。
 * 只给屏幕上看得见的格子做，一批最多几张、错开一点出发，热闹而不乱。盖在整页上面。
 */
@Composable
fun JudgeFlights(cull: CullState) {
    val flights = remember { mutableStateListOf<Flight>() }
    val scope = rememberCoroutineScope()
    var origin by remember { mutableStateOf(Offset.Zero) }
    var seq by remember { mutableStateOf(0L) }
    LaunchedEffect(Unit) {
        Session.judged.collect { batch ->
            var wait = 0L
            for ((path, state) in batch) {
                if (flights.size >= 8) break
                val from = cull.rect(path) ?: continue
                val to = cull.tabRect(state) ?: continue
                if (from.bottom < to.top || from.height <= 0f) continue
                val thumb = Session.results[path]?.thumb ?: continue
                val f = Flight(++seq, thumb, from, to)
                flights += f
                val d = wait
                wait += 70
                scope.launch {
                    delay(d)
                    f.t.animateTo(1f, tween(640, easing = Motion.easeOut))
                    flights -= f
                    cull.tabBump[state] = (cull.tabBump[state] ?: 0) + 1
                }
            }
        }
    }
    Box(Modifier.fillMaxSize().onGloballyPositioned { origin = it.boundsInRoot().topLeft }) {
        for (f in flights) key(f.id) { FlightSprite(f, origin) }
    }
}

@Composable
private fun FlightSprite(f: Flight, origin: Offset) {
    val density = androidx.compose.ui.platform.LocalDensity.current
    val sizePx = with(density) { 64.dp.toPx() }
    val start = Offset(f.from.center.x - sizePx / 2, f.from.center.y - sizePx / 2)
    val end = Offset(f.to.center.x - sizePx / 2, f.to.center.y - sizePx / 2)
    // 二次贝塞尔：控制点在两点上方，走一道弧
    val ctrl = Offset((start.x + end.x) / 2, minOf(start.y, end.y) - sizePx * 1.6f)
    Box(
        Modifier.size(64.dp).graphicsLayer {
            val v = f.t.value
            val u = 1f - v
            val x = u * u * start.x + 2 * u * v * ctrl.x + v * v * end.x
            val y = u * u * start.y + 2 * u * v * ctrl.y + v * v * end.y
            translationX = x - origin.x; translationY = y - origin.y
            val s = 1f - 0.72f * v
            scaleX = s; scaleY = s
            alpha = if (v < 0.8f) 1f else (1f - (v - 0.8f) / 0.2f)
            shadowElevation = 12.dp.toPx() * (1f - v)
            shape = RoundedCornerShape(10.dp); clip = true
        },
    ) {
        AsyncImage(File(f.thumb), null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
    }
}

/**
 * 分析中的卡片：正在分析的那张照片糊在底下（每秒换一次，不跟着每个事件抖），白字压上去——已分析 / 总数、预计还要多久、此刻在做哪张，
 * 底边一条带暖光的进度。只有这张卡跟着分析事件刷新，整页不用。
 */
@Composable
private fun ScanningCard(progress: com.fleetinggleam.android.core.Progress, onStop: () -> Unit) {
    Session.tick.collectAsState().value
    val last = Session.lastStage
    var bg by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) {
        while (true) {
            Session.lastStage?.let { Session.stages[it.first]?.thumb }?.let { bg = it }
            kotlinx.coroutines.delay(900)
        }
    }
    val elapsed = (System.currentTimeMillis() - Session.startedAt) / 1000f
    val left = if (progress.done >= 4) maxOf(1, Math.round(elapsed / progress.done * (progress.total - progress.done))) else null
    androidx.compose.runtime.CompositionLocalProvider(LocalFg provides com.fleetinggleam.android.ui.theme.Dark) {
        val c = LocalFg.current
        Box(Modifier.fillMaxWidth().clip(RoundedCornerShape(20.dp)).background(androidx.compose.ui.graphics.Color(0xFF1C1C20))) {
            androidx.compose.animation.Crossfade(bg, animationSpec = tween(600), label = "scanbg", modifier = Modifier.matchParentSize()) { th ->
                if (th != null) coil3.compose.AsyncImage(
                    java.io.File(th), null, contentScale = androidx.compose.ui.layout.ContentScale.Crop,
                    modifier = Modifier.fillMaxSize().blur(26.dp).graphicsLayer { alpha = 0.6f },
                )
            }
            Box(Modifier.matchParentSize().background(androidx.compose.ui.graphics.Brush.verticalGradient(listOf(androidx.compose.ui.graphics.Color(0x55000000), androidx.compose.ui.graphics.Color(0xB8000000)))))
            Column(Modifier.padding(18.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(t("cull.analyzing"), style = Type.overline.copy(color = c.ash), modifier = Modifier.weight(1f))
                    FgButton(t("cull.stop"), onStop, small = true)
                }
                Row(Modifier.padding(top = 6.dp), verticalAlignment = Alignment.Bottom) {
                    Ticker(progress.done, Type.largeTitle.copy(color = c.ink))
                    Text(" / ${progress.total}", style = Type.title.copy(color = c.ash), modifier = Modifier.padding(bottom = 3.dp))
                    if (left != null) Text("  ·  " + t("android.cull.etaLeft", "time" to duration(left)), style = Type.monoSmall.copy(color = c.ash), modifier = Modifier.padding(bottom = 6.dp), maxLines = 1)
                }
                Text(last?.let { "${it.first.substringAfterLast('/')} · ${t("cull.stage.${it.second}")}" } ?: t("cull.loadingModel"), style = Type.monoSmall.copy(color = c.ash), maxLines = 1, overflow = Ellipsis, modifier = Modifier.padding(top = 4.dp))
                LiveBar(if (progress.total > 0) progress.done.toFloat() / progress.total else 0f, Modifier.padding(top = 14.dp))
                // 系统还在"优化"这个应用的电量：切到后台很可能被停掉。给个入口，点一下是系统自己的确认框
                val ctx = androidx.compose.ui.platform.LocalContext.current
                var exempt by remember { mutableStateOf(com.fleetinggleam.android.core.Scans.backgroundAllowed(ctx)) }
                androidx.lifecycle.compose.LifecycleResumeEffect(Unit) { exempt = com.fleetinggleam.android.core.Scans.backgroundAllowed(ctx); onPauseOrDispose { } }
                if (!exempt) Text(
                    t("android.cull.allowBackground"), style = Type.small.copy(color = c.gleam),
                    modifier = Modifier.padding(top = 12.dp).clip(RoundedCornerShape(8.dp)).clickable { com.fleetinggleam.android.core.Scans.askBackground(ctx) }.padding(vertical = 4.dp),
                )
                else Text(t("android.cull.canLeave"), style = Type.small.copy(color = c.ash), modifier = Modifier.padding(top = 12.dp))
            }
        }
    }
}
