package com.fleetinggleam.android.ui.project

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateCentroid
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import com.fleetinggleam.android.core.Decision
import com.fleetinggleam.android.core.Large
import com.fleetinggleam.android.core.Photo
import com.fleetinggleam.android.core.UiPrefs
import com.fleetinggleam.android.core.pct
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgButton
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.io.File
import kotlin.math.ceil
import kotlin.math.max
import kotlin.math.min

/** 共同的视野：放大倍数与平移（平移按画面自身大小的比例记，所以每一张看的都是同一处）。 */
private data class View(val s: Float = 1f, val x: Float = 0f, val y: Float = 0f)

/**
 * 从一组连拍里挑最好的（桌面端 Compare.tsx 的手机版）。几张照片尽量大地排开；任意一张上双指缩放、拖动、双击，
 * 所有照片一起动，同一处细节（眼睛、手）可以逐张对照。每张上面有它的几项数字（同组最高的加粗）和保留 / 淘汰。
 * 在底栏上左右滑、或点箭头换一组；下面的小图把同组其他照片换进最后一格。
 */
@Composable
fun CompareView(photos: List<Photo>, all: List<Photo>, scopeProp: String, cull: CullState, decisions: Map<String, Decision>, onDecide: (String, String?) -> Unit, onOpen: (String) -> Unit, bottomPad: Dp) {
    val c = LocalFg.current
    val scope = rememberCoroutineScope()
    val cur = photos.firstOrNull { it.path == cull.selected } ?: photos.firstOrNull() ?: return
    var scopeMode by remember(scopeProp) { mutableStateOf(scopeProp) }
    val keyOf: (Photo) -> Int = { if (scopeMode == "scene") it.scene else it.group }
    val members = remember(all, scopeMode) { all.groupBy(keyOf) }
    val groups = remember(photos, members, scopeMode) {
        val ids = ArrayList<Int>()
        for (p in photos) { val k = keyOf(p); if ((members[k]?.size ?: 0) > 1 && k !in ids) ids.add(k) }
        ids
    }
    val gi = groups.indexOf(keyOf(cur))
    fun ranked(id: Int) = (members[id] ?: emptyList()).sortedByDescending { it.score }
    var count by remember { mutableIntStateOf(UiPrefs.int("cmpn", 2).coerceIn(2, 4)) }
    val groupPhotos = remember(gi, cur.group, cur.scene, members, scopeMode) { if (gi >= 0) ranked(keyOf(cur)) else emptyList() }
    val shownN = min(count, groupPhotos.size)
    // 屏幕上的几张住在固定的格子里；只有最后一格（挑战者）会被换掉
    var slots by remember { mutableStateOf(listOf<String>()) }
    LaunchedEffect(cur.path, cur.group, cur.scene, shownN, groupPhotos) {
        val ids = groupPhotos.map { it.path }
        val next = slots.filter { it in ids }.take(shownN).toMutableList()
        for (id in ids) if (next.size < shownN && id !in next) next.add(id)
        if (cur.path in ids && cur.path !in next && next.isNotEmpty()) next[next.size - 1] = cur.path
        if (next != slots) slots = next
    }
    val set = remember(slots, groupPhotos, shownN) {
        val by = groupPhotos.associateBy { it.path }
        slots.mapNotNull { by[it] }.ifEmpty { groupPhotos.take(shownN) }
    }
    val candidates = groupPhotos.filter { g -> set.none { it.path == g.path } }
    fun jump(d: Int) {
        val from = if (gi >= 0) gi else if (d > 0) -1 else groups.size
        groups.getOrNull((from + d).coerceIn(0, groups.size - 1))?.let { cull.select(ranked(it)[0].path) }
    }
    val view = remember { Animatable(View(), androidx.compose.animation.core.TwoWayConverter({ androidx.compose.animation.core.AnimationVector3D(it.s, it.x, it.y) }, { View(it.v1, it.v2, it.v3) })) }
    LaunchedEffect(cur.group, cur.scene) { view.snapTo(View()) }
    fun apply(v: View) {
        val lim = max(0f, (v.s - 1) / 2)
        val n = View(v.s, if (v.s <= 1) 0f else v.x.coerceIn(-lim, lim), if (v.s <= 1) 0f else v.y.coerceIn(-lim, lim))
        scope.launch { view.snapTo(n) }
    }

    if (groups.isEmpty() || gi < 0) {
        Column(Modifier.fillMaxSize().padding(24.dp).padding(bottom = bottomPad), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
            AsyncImage(File(cur.thumb), null, Modifier.size(180.dp, 120.dp).clip(RoundedCornerShape(12.dp)), contentScale = ContentScale.Crop)
            Text(t("cmp.emptyTitle"), style = Type.heading.copy(color = c.ink), modifier = Modifier.padding(top = 16.dp))
            Text(t("cmp.emptyBody"), style = Type.rowDesc.copy(color = c.ash), modifier = Modifier.padding(top = 6.dp))
            if (groups.isNotEmpty()) FgButton(t("cmp.jump"), { jump(1) }, primary = true, modifier = Modifier.padding(top = 16.dp))
            ScopeToggle(scopeMode) { scopeMode = it }
        }
        return
    }
    val faces = set.any { it.faceCount > 0 }
    val metrics: List<Triple<String, String, (Photo) -> Int?>> = buildList {
        add(Triple("score", t("inspector.overall")) { p: Photo -> pct(p.score) })
        add(Triple("sharp", t("inspector.sharp")) { p: Photo -> pct(p.sharpScore) })
        add(Triple("expo", t("inspector.expo")) { p: Photo -> pct(p.expoScore) })
        if (faces) add(Triple("face", t("inspector.faces")) { p: Photo -> if (p.faceCount > 0) pct(p.faceMin ?: 0f) else null })
    }

    Column(Modifier.fillMaxSize().padding(bottom = bottomPad)) {
        BoxWithConstraints(Modifier.weight(1f).fillMaxWidth().padding(horizontal = 10.dp, vertical = 6.dp)) {
            val gap = 8f
            val bw = constraints.maxWidth.toFloat(); val bh = constraints.maxHeight.toFloat()
            // 最合适的排法：让照片最大的那个列数
            val ratio = set.map { it.ratio }.average().toFloat()
            var cols = 1; var best = 0f
            for (cc in 1..set.size) {
                val rows = ceil(set.size / cc.toFloat()).toInt()
                val cw = (bw - gap * (cc - 1)) / cc; val ch = (bh - gap * (rows - 1)) / rows
                val w = min(cw, ch * ratio)
                if (w * w / ratio > best) { best = w * w / ratio; cols = cc }
            }
            val rows = ceil(set.size / cols.toFloat()).toInt()
            val density = LocalDensity.current
            val cellW = with(density) { ((bw - gap * (cols - 1)) / cols).toDp() }
            val cellH = with(density) { ((bh - gap * (rows - 1)) / rows).toDp() }
            Column(verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxSize()) {
                set.chunked(cols).forEachIndexed { r, row ->
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        row.forEachIndexed { k, p ->
                            Cell(p, r * cols + k, decisions[p.path], view.value, cellW, cellH, metrics, set, set.size, cull,
                                onDecide = { st -> onDecide(p.path, st) }, onOpen = { onOpen(p.path) }, apply = { apply(it) },
                                reset = { scope.launch { view.animateTo(View(), com.fleetinggleam.android.ui.theme.Motion.glide()) } })
                        }
                    }
                }
            }
        }
        // ---- 底栏：上一组 · 第几组 · 下一组；张数；范围；同组其他照片
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 12.dp).pointerInput(gi) {
                var acc = 0f
                detectHorizontalDragGestures(onDragEnd = { if (acc < -60) jump(1) else if (acc > 60) jump(-1); acc = 0f }) { ch, d -> ch.consume(); acc += d }
            },
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ArrowBtn("M10 3L5 8l5 5", gi > 0) { jump(-1) }
            Text(if (scopeMode == "scene") t("cmp.sceneOf", "n" to groupPhotos.size) else t("cmp.groupOf", "i" to gi + 1, "n" to groups.size), style = Type.monoSmall.copy(color = c.ash),
                maxLines = 1, overflow = Ellipsis, modifier = Modifier.weight(1f).padding(horizontal = 6.dp), textAlign = androidx.compose.ui.text.style.TextAlign.Center)
            ArrowBtn("M6 3l5 5-5 5", gi < groups.size - 1) { jump(1) }
        }
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            ScopeToggle(scopeMode) { scopeMode = it }
            if (groupPhotos.size > 2) Row(Modifier.padding(start = 8.dp).clip(RoundedCornerShape(10.dp)).background(c.surface).border(1.dp, c.line, RoundedCornerShape(10.dp)).padding(2.dp)) {
                listOf(2, 3, 4).filter { it <= groupPhotos.size }.forEach { n ->
                    val on = shownN == n
                    Text("$n", style = Type.mono.copy(color = if (on) c.onInk else c.ink), modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(if (on) c.ink else Color.Transparent)
                        .clickable { count = n; UiPrefs.put("cmpn", n) }.padding(horizontal = 12.dp, vertical = 5.dp))
                }
            }
            Row(Modifier.weight(1f).padding(start = 8.dp).horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                candidates.forEach { p ->
                    AsyncImage(File(p.thumb), null, Modifier.size(38.dp).clip(RoundedCornerShape(7.dp)).clickable {
                        slots = slots.toMutableList().also { if (it.isNotEmpty()) it[it.size - 1] = p.path }
                    }, contentScale = ContentScale.Crop)
                }
            }
        }
    }
}

@Composable
private fun ScopeToggle(mode: String, onChange: (String) -> Unit) {
    val c = LocalFg.current
    Row(Modifier.padding(top = 4.dp).clip(RoundedCornerShape(10.dp)).background(c.surface).border(1.dp, c.line, RoundedCornerShape(10.dp)).padding(2.dp)) {
        listOf("group" to t("cmp.byGroup"), "scene" to t("cmp.byScene")).forEach { (id, label) ->
            val on = mode == id
            Text(label, style = Type.rowDesc.copy(color = if (on) c.onInk else c.ink, fontWeight = FontWeight.Medium), modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(if (on) c.ink else Color.Transparent)
                .clickable { onChange(id) }.padding(horizontal = 10.dp, vertical = 5.dp))
        }
    }
}

@Composable
private fun ArrowBtn(d: String, enabled: Boolean, onClick: () -> Unit) {
    val c = LocalFg.current
    Box(Modifier.size(40.dp).clip(CircleShape).graphicsLayer { alpha = if (enabled) 1f else 0.3f }.clickable(enabled = enabled, onClick = onClick), contentAlignment = Alignment.Center) {
        FgIcon(d, size = 18.dp, stroke = 1.6f, color = c.ink)
    }
}

@Composable
private fun Cell(
    p: Photo, index: Int, d: Decision?, view: View, w: Dp, h: Dp, metrics: List<Triple<String, String, (Photo) -> Int?>>, set: List<Photo>, count: Int, cull: CullState,
    onDecide: (String?) -> Unit, onOpen: () -> Unit, apply: (View) -> Unit, reset: () -> Unit,
) {
    val c = LocalFg.current
    val ctx = LocalContext.current
    val density = LocalDensity.current
    val st = d?.state ?: "review"
    val manual = d?.manual == true
    // 大预览在照片出现后稍等一下才准备，放大时多半已经好了；好之前一直显示缩略图
    var warm by remember(p.path) { mutableStateOf(false) }
    var large by remember(p.path) { mutableStateOf(Large.ready(p.path)) }
    var sharp by remember(p.path) { mutableStateOf(false) }
    LaunchedEffect(p.path) { delay(500); warm = true }
    LaunchedEffect(p.path, warm, view.s > 1.02f) { if (large == null && (warm || view.s > 1.02f)) large = Large.get(p.path) }
    val W = with(density) { w.toPx() }; val H = with(density) { h.toPx() }
    val fw = min(W, H * p.ratio); val fh = fw / p.ratio
    val shape = RoundedCornerShape(12.dp)
    // 手势里要的是此刻的视野，不是手势开始那一刻捕获的
    val latest by androidx.compose.runtime.rememberUpdatedState(view)
    Box(Modifier.size(w, h), contentAlignment = Alignment.Center) {
        Box(
            Modifier.size(with(density) { fw.toDp() }, with(density) { fh.toDp() })
                .onGloballyPositioned { cull.rects[p.path] = it.boundsInRoot() }
                .clip(shape)
                .border(if (st == "keep") 1.5.dp else 0.dp, if (st == "keep") c.gleam else Color.Transparent, shape)
                .background(c.surface)
                .pointerInput(p.path) {
                    detectTapGestures(onDoubleTap = { at ->
                        if (latest.s > 1.02f) reset()
                        else { val cx = at.x / fw - 0.5f; val cy = at.y / fh - 0.5f; val n = 2.5f; apply(View(n, cx - n * cx, cy - n * cy)) }
                    })
                }
                .pointerInput(p.path, fw, fh) {
                    awaitEachGesture {
                        awaitFirstDown(requireUnconsumed = false)
                        var v = latest
                        do {
                            val ev = awaitPointerEvent()
                            val fingers = ev.changes.count { it.pressed }
                            if (fingers >= 2) {
                                val z = ev.calculateZoom(); val cen = ev.calculateCentroid(useCurrent = true)
                                val cx = cen.x / fw - 0.5f; val cy = cen.y / fh - 0.5f
                                val n = (v.s * z).coerceIn(1f, 12f)
                                val pp = ev.calculatePan()
                                v = View(n, cx - (n / v.s) * (cx - v.x) + pp.x / fw, cy - (n / v.s) * (cy - v.y) + pp.y / fh)
                                apply(v)
                                ev.changes.forEach { if (it.positionChanged()) it.consume() }
                            } else if (v.s > 1.01f) {
                                val pp = ev.calculatePan()
                                v = View(v.s, v.x + pp.x / fw, v.y + pp.y / fh)
                                apply(v)
                                ev.changes.forEach { if (it.positionChanged()) it.consume() }
                            }
                        } while (ev.changes.any { it.pressed })
                    }
                },
        ) {
            // 缩放改的是照片真实的大小与位置（不是拉伸一张位图）
            Box(Modifier.fillMaxSize().graphicsLayer { scaleX = view.s; scaleY = view.s; translationX = view.x * fw; translationY = view.y * fh }) {
                AsyncImage(File(p.thumb), null, Modifier.fillMaxSize(), contentScale = ContentScale.FillBounds)
                large?.let { src ->
                    val req = remember(src) { ImageRequest.Builder(ctx).data(File(src)).size(2560, 2560).build() }
                    AsyncImage(req, null, Modifier.fillMaxSize().graphicsLayer { alpha = if (sharp) 1f else 0f }, contentScale = ContentScale.FillBounds, onSuccess = { sharp = true })
                }
            }
            // 标签：推荐 / 已保留 / 已淘汰，放大倍数，放大查看
            Row(Modifier.align(Alignment.TopStart).padding(7.dp), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                PhotoTags(p)
                if (p.groupBest && count > 1) Tag(t("cmp.best"), star = true)
                if (st == "keep") Tag(t("cmp.kept"), star = true)
                if (st == "reject") Tag(t("cmp.rejected"))
                if (view.s > 1.02f) Tag("%.1f×".format(view.s))
            }
            Box(Modifier.align(Alignment.TopEnd).padding(6.dp).size(30.dp).clip(CircleShape).background(Color(0x99101014)).clickable(onClick = onOpen), contentAlignment = Alignment.Center) {
                FgIcon("M9.5 2.5h4v4M6.5 13.5h-4v-4M13.5 2.5L9 7M2.5 13.5L7 9", size = 13.dp, stroke = 1.5f, color = Color.White)
            }
            // 底下：编号、几项数字（同组最高的加粗）、保留 / 淘汰
            Row(
                Modifier.align(Alignment.BottomCenter).fillMaxWidth()
                    .background(androidx.compose.ui.graphics.Brush.verticalGradient(listOf(Color.Transparent, Color(0xB0000000))))
                    .padding(start = 8.dp, end = 6.dp, top = 18.dp, bottom = 6.dp),
                verticalAlignment = Alignment.Bottom,
            ) {
                Column(Modifier.weight(1f)) {
                    Text("${index + 1}  ${p.name}", style = Type.monoSmall.copy(color = Color.White.copy(alpha = 0.85f)), maxLines = 1, overflow = Ellipsis)
                    Row(Modifier.padding(top = 3.dp).horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        metrics.forEach { (key, label, get) ->
                            val v = get(p)
                            val vals = set.mapNotNull(get)
                            val top = v != null && vals.size > 1 && v == vals.max() && v > vals.min()
                            Row(Modifier.clip(RoundedCornerShape(5.dp)).background(Color.White.copy(alpha = if (key == "score") 0.22f else 0.12f)).padding(horizontal = 5.dp, vertical = 1.dp)) {
                                Text(label, style = Type.monoSmall.copy(color = Color.White.copy(alpha = 0.75f), fontSize = androidx.compose.ui.unit.TextUnit(10f, androidx.compose.ui.unit.TextUnitType.Sp)))
                                Text(" ${v ?: "—"}", style = Type.monoSmall.copy(color = if (top) c.gleam else Color.White, fontWeight = if (top) FontWeight.Bold else FontWeight.Normal, fontSize = androidx.compose.ui.unit.TextUnit(10f, androidx.compose.ui.unit.TextUnitType.Sp)))
                            }
                        }
                    }
                }
                DecideDot(on = st == "keep", keep = true) { onDecide(if (st == "keep" && manual) null else "keep") }
                Spacer(Modifier.size(5.dp))
                DecideDot(on = st == "reject", keep = false) { onDecide(if (st == "reject" && manual) null else "reject") }
            }
        }
    }
}

@Composable
private fun Tag(text: String, star: Boolean = false) {
    Row(Modifier.clip(RoundedCornerShape(6.dp)).background(Color(0xA30E0E12)).padding(horizontal = 6.dp, vertical = 2.dp), verticalAlignment = Alignment.CenterVertically) {
        if (star) Star(true, 10.dp, glow = false)
        Text(text, style = Type.monoSmall.copy(color = Color.White, fontSize = androidx.compose.ui.unit.TextUnit(10f, androidx.compose.ui.unit.TextUnitType.Sp)), modifier = Modifier.padding(start = if (star) 3.dp else 0.dp))
    }
}

@Composable
private fun DecideDot(on: Boolean, keep: Boolean, onClick: () -> Unit) {
    val c = LocalFg.current
    Box(
        Modifier.size(36.dp).clip(CircleShape).background(if (on) (if (keep) c.gleam else Color.White) else Color(0x66000000)).border(1.dp, Color.White.copy(alpha = 0.35f), CircleShape).clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        if (keep) Star(true, 17.dp, color = if (on) c.gleamInk else Color.White, glow = false)
        else FgIcon("M3 3l6 6M9 3L3 9", size = 12.dp, box = 12f, stroke = 1.8f, color = if (on) Color.Black else Color.White)
    }
}
