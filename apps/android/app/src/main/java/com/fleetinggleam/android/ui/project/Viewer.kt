package com.fleetinggleam.android.ui.project

import android.app.Activity
import android.content.pm.ActivityInfo
import android.os.Build
import android.widget.VideoView
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.VectorConverter
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateCentroid
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
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
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import com.fleetinggleam.android.core.Core
import com.fleetinggleam.android.core.Decision
import com.fleetinggleam.android.core.Large
import com.fleetinggleam.android.core.Photo
import com.fleetinggleam.android.core.Session
import com.fleetinggleam.android.core.UiPrefs
import com.fleetinggleam.android.core.args
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.components.GlassBox
import com.fleetinggleam.android.ui.components.Icons
import com.fleetinggleam.android.ui.components.LocalBackdrop
import com.fleetinggleam.android.ui.components.glassStyle
import com.fleetinggleam.android.ui.components.anchor
import com.fleetinggleam.android.ui.components.rememberPress
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Motion
import com.fleetinggleam.android.ui.theme.Type
import com.kyant.backdrop.backdrops.layerBackdrop
import com.kyant.backdrop.backdrops.rememberLayerBackdrop
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch
import java.io.File
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min

/** 查看器的底色：照片四周是深的，照片自己的模糊副本把房间照亮（桌面端 .viewer 的 ambient）。 */
private val ROOM = Color(0xFF0B0B0D)

/**
 * 大图（桌面端 Viewer.tsx 的手机版）。打开时照片从它的格子里飞出来；左右滑换一张；双指或双击放大，放大后按住拖；
 * 放大到比已经解码的图更细时，屏幕上那一块从原图按它自己的分辨率解出来盖上去（像手机相册看 2 亿像素那样）。
 * 往下拖关掉（照片飞回格子），往上拖看这张的检查器。实况照片打开时安静地放一遍，HDR 照片在支持的屏幕上按 HDR 显示。
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Viewer(cull: CullState, data: CullData, decisions: Map<String, Decision>, onDecide: (String, String?) -> Unit) {
    val shown = data.shown
    val start = data.index(cull.selected).coerceAtLeast(0)
    if (shown.isEmpty()) { cull.viewer = false; return }
    val ctx = LocalContext.current
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    val haptic = LocalHapticFeedback.current
    val pager = rememberPagerState(initialPage = start) { shown.size }
    val photo = shown.getOrNull(pager.currentPage) ?: shown[0]
    var scale by remember { mutableFloatStateOf(1f) }
    var chrome by remember { mutableStateOf(true) }
    var info by remember { mutableStateOf(false) }
    val enter = remember { Animatable(0f) }
    val drag = remember { Animatable(0f) }
    var stage by remember { mutableStateOf<androidx.compose.ui.geometry.Rect?>(null) }
    val origin = remember { cull.rect(shown[start].path) }
    var liveOn by remember { mutableStateOf(UiPrefs.bool("live", true)) }
    var playing by remember { mutableIntStateOf(0) }
    var quiet by remember { mutableStateOf(true) }
    var clip by remember { mutableStateOf<String?>(null) }
    val backdrop = rememberLayerBackdrop()

    // 选中的跟着翻页走；从外面换了（下面的小图条）就翻过去
    LaunchedEffect(pager) { snapshotFlow { pager.settledPage }.distinctUntilChanged().collect { i -> shown.getOrNull(i)?.let { cull.select(it.path) } } }
    LaunchedEffect(cull.selected) { val i = data.index(cull.selected); if (i >= 0 && i != pager.currentPage) pager.animateScrollToPage(i) }
    LaunchedEffect(Unit) { enter.animateTo(1f, Motion.view()) }

    fun close() {
        scope.launch {
            enter.animateTo(0f, spring(dampingRatio = 0.9f, stiffness = 320f))
            cull.viewer = false
        }
    }
    BackHandler { if (info) info = false else close() }

    // 暗房间里状态栏的图标用浅色；关掉查看器时还给应用自己的深浅
    val view = androidx.compose.ui.platform.LocalView.current
    val appDark = LocalFg.current.dark
    DisposableEffect(Unit) {
        val w = (ctx as? Activity)?.window
        val ctl = w?.let { androidx.core.view.WindowCompat.getInsetsController(it, view) }
        ctl?.isAppearanceLightStatusBars = false; ctl?.isAppearanceLightNavigationBars = false
        onDispose { ctl?.isAppearanceLightStatusBars = !appDark; ctl?.isAppearanceLightNavigationBars = !appDark }
    }
    // HDR：这张带增益图、系统能显示时，把窗口切到 HDR 模式（Android 14+）；离开就切回来
    DisposableEffect(photo.path, photo.hdr) {
        val w = (ctx as? Activity)?.window
        if (Build.VERSION.SDK_INT >= 34 && photo.hdr) w?.colorMode = ActivityInfo.COLOR_MODE_HDR
        onDispose { if (Build.VERSION.SDK_INT >= 34) w?.colorMode = ActivityInfo.COLOR_MODE_DEFAULT }
    }
    // 实况：停稳了安静地放一遍（关了"实况"就不自动放）
    LaunchedEffect(photo.path, liveOn) {
        clip = null
        if (!photo.motion || !liveOn) return@LaunchedEffect
        delay(600)
        runCatching { Core.get<String>("motion_clip", args("path" to photo.path)) }.getOrNull()?.let { quiet = true; clip = it; playing++ }
    }
    suspend fun playMotion(sound: Boolean) {
        runCatching { Core.get<String>("motion_clip", args("path" to photo.path)) }.getOrNull()?.let { quiet = !sound; clip = it; playing++ }
    }

    val e = enter.value
    val dy = drag.value
    val fade = (1f - abs(dy) / with(density) { 360.dp.toPx() }).coerceIn(0f, 1f)
    Box(Modifier.fillMaxSize().onGloballyPositioned { stage = it.boundsInRoot() }) {
        // ---- 房间：深色底，照片的模糊副本把它照亮
        Box(Modifier.fillMaxSize().graphicsLayer { alpha = e * fade }.background(ROOM)) {
            AsyncImage(File(photo.thumb), null, Modifier.fillMaxSize().blur(60.dp).graphicsLayer { alpha = 0.42f }, contentScale = ContentScale.Crop)
        }
        // ---- 照片
        Box(Modifier.fillMaxSize().then(if (com.fleetinggleam.android.ui.components.LocalGlassOn.current) Modifier.layerBackdrop(backdrop) else Modifier)) {
            HorizontalPager(
                pager, Modifier.fillMaxSize().pointerInput(scale) {
                    // 没放大时竖着拖：往下关、往上看详情
                    if (scale > 1.01f) return@pointerInput
                    detectVerticalDragGestures(
                        onDragEnd = {
                            val v = drag.value
                            when {
                                v > with(density) { 110.dp.toPx() } -> close()
                                v < -with(density) { 80.dp.toPx() } -> { info = true; scope.launch { drag.animateTo(0f, Motion.glide()) } }
                                else -> scope.launch { drag.animateTo(0f, Motion.glide()) }
                            }
                        },
                        onDragCancel = { scope.launch { drag.animateTo(0f, Motion.glide()) } },
                    ) { change, d -> change.consume(); scope.launch { drag.snapTo(drag.value + if (drag.value < 0) d * 0.5f else d) } }
                },
                userScrollEnabled = scale <= 1.01f, beyondViewportPageCount = 1, key = { shown[it].path },
            ) { page ->
                val p = shown[page]
                val isOrigin = page == start
                ZoomPage(
                    p, active = page == pager.currentPage,
                    clip = if (page == pager.currentPage) clip else null, clipRun = playing, quiet = quiet, onClipEnd = { clip = null },
                    onScale = { if (page == pager.currentPage) scale = it },
                    onTap = { chrome = !chrome },
                    modifier = Modifier.graphicsLayer {
                        // 打开：从格子飞出来；关上：飞回此刻这张的格子（不在屏幕上就只淡出）
                        val from = if (isOrigin && page == pager.currentPage) origin else if (page == pager.currentPage) cull.rect(p.path) else null
                        val st = stage
                        if (from != null && st != null && e < 1f) {
                            val fit = fitRect(p.ratio, st.width, st.height)
                            val sx = from.width / fit.width; val sy = from.height / fit.height
                            val s = sx + (1f - sx) * e
                            val sY = sy + (1f - sy) * e
                            scaleX = s; scaleY = sY
                            translationX = (from.center.x - st.center.x) * (1f - e)
                            translationY = (from.center.y - st.center.y) * (1f - e)
                        } else if (e < 1f) { alpha = e; val s = 0.92f + 0.08f * e; scaleX = s; scaleY = s }
                        translationY += dy
                        if (dy > 0) { val s = 1f - 0.12f * (1f - fade); scaleX *= s; scaleY *= s }
                    },
                )
            }
        }

        // ---- 上面一条：关闭、名字、标签、实况、第几张。查看器总是暗的房间，所以玻璃件和文字一律用深色那套
        CompositionLocalProvider(LocalBackdrop provides backdrop, LocalFg provides com.fleetinggleam.android.ui.theme.Dark) {
            AnimatedVisibility(chrome && e > 0.6f, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.align(Alignment.TopCenter)) {
                Row(Modifier.fillMaxWidth().statusBarsPadding().padding(horizontal = 12.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    GlassRound(Icons.CLOSE, t("viewer.close")) { close() }
                    GlassBox(Modifier.padding(start = 10.dp).weight(1f, fill = false), style = glassStyle(strong = true)) {
                        Row(Modifier.padding(horizontal = 14.dp, vertical = 9.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text(photo.name, style = Type.monoSmall.copy(color = LocalFg.current.ink), maxLines = 1, overflow = Ellipsis, modifier = Modifier.weight(1f, fill = false))
                            Text("  ${pager.currentPage + 1} / ${shown.size}" + if (scale > 1.02f) " · ${Math.round(scale * 100)}%" else "", style = Type.monoSmall.copy(color = LocalFg.current.ash), maxLines = 1)
                        }
                    }
                    if (photo.motion) {
                        val overlay = com.fleetinggleam.android.ui.components.LocalOverlay.current
                        val anchor = com.fleetinggleam.android.ui.components.rememberAnchor()
                        Box(Modifier.padding(start = 8.dp).then(Modifier.anchor(anchor))) {
                            GlassPill(t("tag.motion"), on = clip != null) {
                                overlay.showMenu(anchor.value, listOf(
                                    com.fleetinggleam.android.ui.components.MenuItem(if (clip != null) t("viewer.stopMotion") else t("viewer.playMotion")) { if (clip != null) clip = null else scope.launch { playMotion(true) } },
                                    com.fleetinggleam.android.ui.components.MenuItem(t("viewer.replayMotion")) { scope.launch { playMotion(true) } },
                                    com.fleetinggleam.android.ui.components.MenuItem(if (liveOn) t("viewer.liveOff") else t("viewer.liveOn")) { liveOn = !liveOn; UiPrefs.put("live", liveOn); if (!liveOn) clip = null },
                                ), dark = true)
                            }
                        }
                    }
                }
            }

            // ---- 下面：小图条、保留 / 待复查 / 淘汰、详情
            AnimatedVisibility(chrome && e > 0.6f && fade > 0.7f, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.align(Alignment.BottomCenter)) {
                Column(Modifier.fillMaxWidth().navigationBarsPadding().padding(bottom = 10.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Strip(shown, decisions, pager.currentPage) { cull.select(it) }
                    val d = decisions[photo.path]
                    GlassBox(Modifier.padding(top = 10.dp), shape = RoundedCornerShape(50)) {
                        Row(Modifier.padding(6.dp), verticalAlignment = Alignment.CenterVertically) {
                            listOf("keep", "review", "reject").forEach { st ->
                                val on = d?.state == st
                                DecideBtn(st, on) { haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove); onDecide(photo.path, if (on && d?.manual == true) null else st) }
                            }
                            Box(Modifier.padding(horizontal = 4.dp).width(1.dp).height(26.dp).background(LocalFg.current.line))
                            DecideIcon(Icons.INSPECTOR, t("cull.inspector")) { info = true }
                        }
                    }
                }
            }
        }
    }

    if (info) {
        val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = false)
        val mates = remember(photo, data) { Session.photos.value.filter { it.group == photo.group } }
        ModalBottomSheet(onDismissRequest = { info = false }, sheetState = sheet, containerColor = LocalFg.current.canvas) {
            Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(horizontal = 20.dp).padding(bottom = 24.dp)) {
                InspectorBody(photo, decisions[photo.path], mates, onDecide = { onDecide(photo.path, it) }, onSelect = { cull.select(it) })
            }
        }
    }
}

/** 照片按比例放进一个框里时的位置与大小。 */
private fun fitRect(ratio: Float, w: Float, h: Float): androidx.compose.ui.geometry.Rect {
    val fw = min(w, h * ratio); val fh = fw / ratio
    return androidx.compose.ui.geometry.Rect((w - fw) / 2, (h - fh) / 2, (w + fw) / 2, (h + fh) / 2)
}

/**
 * 一页：照片按比例放在中间，可以缩放、平移。缩放画的是照片本身（不是放大一张位图），清晰的部分到了再叠上去。
 */
@Composable
private fun ZoomPage(p: Photo, active: Boolean, clip: String?, clipRun: Int, quiet: Boolean, onClipEnd: () -> Unit, onScale: (Float) -> Unit, onTap: () -> Unit, modifier: Modifier) {
    val ctx = LocalContext.current
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    val zoom = remember(p.path) { Animatable(1f) }
    val pan = remember(p.path) { Animatable(Offset.Zero, Offset.VectorConverter) }
    var large by remember(p.path) { mutableStateOf(Large.ready(p.path, true)) }
    var sharp by remember(p.path) { mutableStateOf(false) }
    var detail by remember(p.path) { mutableStateOf<Pair<String, androidx.compose.ui.geometry.Rect>?>(null) }
    LaunchedEffect(p.path, active) { if (large == null && active) large = Large.get(p.path, full = true) ?: Large.get(p.path) }
    LaunchedEffect(zoom.value) { if (active) onScale(zoom.value) }

    BoxWithConstraints(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        val W = constraints.maxWidth.toFloat(); val H = constraints.maxHeight.toFloat()
        val fit = fitRect(p.ratio, W, H)
        fun clamp(o: Offset, s: Float): Offset {
            val lx = max(0f, (fit.width * s - W) / 2); val ly = max(0f, (fit.height * s - H) / 2)
            return Offset(o.x.coerceIn(-lx, lx), o.y.coerceIn(-ly, ly))
        }
        fun zoomTo(target: Float, at: Offset?, animate: Boolean) {
            val s0 = zoom.value
            val n = target.coerceIn(1f, 10f)
            // 手指下那一点留在手指下
            val c = at?.let { Offset(it.x - W / 2, it.y - H / 2) } ?: Offset.Zero
            val o = clamp(c - (c - pan.value) * (n / s0), n)
            scope.launch {
                if (animate) { launch { zoom.animateTo(n, Motion.glide()) }; pan.animateTo(o, Motion.glide()) }
                else { zoom.snapTo(n); pan.snapTo(o) }
            }
        }
        // 清晰的部分：放大到比已经解码的（最长边 4096）还细时，按屏幕上那一块从原图解出来
        LaunchedEffect(zoom.value, pan.value, active) {
            if (!active) return@LaunchedEffect
            val long = max(p.width, p.height).toFloat()
            val have = min(long, 4096f)
            val s = zoom.value
            // 原图不比已解码的大，或者屏幕上画出来的还没超过已解码的像素：不用再取
            if (long <= have * 1.05f || max(fit.width, fit.height) * s <= have * 0.9f) {
                if (s < 1.05f) detail = null
                return@LaunchedEffect
            }
            delay(160)
            // 屏幕上看得到的那一块（照片的比例坐标），四周再留一些，拖一拖还在清晰范围里
            val o = pan.value
            val x0 = ((-W / 2 - o.x) / (fit.width * s) + 0.5f).coerceIn(0f, 1f); val x1 = ((W / 2 - o.x) / (fit.width * s) + 0.5f).coerceIn(0f, 1f)
            val y0 = ((-H / 2 - o.y) / (fit.height * s) + 0.5f).coerceIn(0f, 1f); val y1 = ((H / 2 - o.y) / (fit.height * s) + 0.5f).coerceIn(0f, 1f)
            val cur = detail?.second
            if (cur != null && x0 >= cur.left && y0 >= cur.top && x1 <= cur.right && y1 <= cur.bottom) return@LaunchedEffect
            val px = (x1 - x0) * 0.3f; val py = (y1 - y0) * 0.3f
            val g = androidx.compose.ui.geometry.Rect((x0 - px).coerceAtLeast(0f), (y0 - py).coerceAtLeast(0f), (x1 + px).coerceAtMost(1f), (y1 + py).coerceAtMost(1f))
            val outW = max(800, min((g.width * p.width).toInt(), (g.width * fit.width * s * 1.1f).toInt()))
            runCatching { Core.get<String>("photo_detail", args("path" to p.path, "x" to g.left, "y" to g.top, "w" to g.width, "h" to g.height, "out_w" to outW)) }.getOrNull()?.let { detail = it to g }
        }

        Box(
            Modifier.size(with(density) { fit.width.toDp() }, with(density) { fit.height.toDp() })
                .graphicsLayer { scaleX = zoom.value; scaleY = zoom.value; translationX = pan.value.x; translationY = pan.value.y }
                .pointerInput(p.path) {
                    detectTapGestures(
                        onTap = { onTap() },
                        onDoubleTap = { at ->
                            // 点在照片自己的坐标里；换算成屏幕上的位置
                            val sx = (at.x - fit.width / 2) * zoom.value + pan.value.x + W / 2
                            val sy = (at.y - fit.height / 2) * zoom.value + pan.value.y + H / 2
                            if (zoom.value > 1.02f) zoomTo(1f, null, true) else zoomTo(3f, Offset(sx, sy), true)
                        },
                    )
                }
                .pointerInput(p.path) {
                    // 两指缩放；放大后一指平移（没放大时一指的拖动留给翻页和下拉关闭）
                    awaitEachGesture {
                        awaitFirstDown(requireUnconsumed = false)
                        do {
                            val ev = awaitPointerEvent()
                            val fingers = ev.changes.count { it.pressed }
                            if (fingers >= 2) {
                                val z = ev.calculateZoom(); val c = ev.calculateCentroid(useCurrent = true)
                                val pp = ev.calculatePan()
                                val s0 = zoom.value
                                val n = (s0 * z).coerceIn(0.85f, 10f)
                                val cs = Offset((c.x - fit.width / 2) * s0 + pan.value.x, (c.y - fit.height / 2) * s0 + pan.value.y)
                                val o = cs - (cs - pan.value) * (n / s0) + pp * s0
                                scope.launch { zoom.snapTo(n); pan.snapTo(if (n <= 1f) o else clamp(o, n)) }
                                ev.changes.forEach { if (it.positionChanged()) it.consume() }
                            } else if (zoom.value > 1.01f) {
                                val pp = ev.calculatePan()
                                scope.launch { pan.snapTo(clamp(pan.value + pp * zoom.value, zoom.value)) }
                                ev.changes.forEach { if (it.positionChanged()) it.consume() }
                            }
                        } while (ev.changes.any { it.pressed })
                        if (zoom.value < 1f) zoomTo(1f, null, true)
                    }
                },
        ) {
            AsyncImage(File(p.thumb), null, Modifier.fillMaxSize(), contentScale = ContentScale.FillBounds)
            large?.let { src ->
                val req = remember(src) { ImageRequest.Builder(ctx).data(File(src)).size(4096, 4096).build() }
                AsyncImage(req, p.name, Modifier.fillMaxSize().graphicsLayer { alpha = if (sharp) 1f else 0f }, contentScale = ContentScale.FillBounds, onSuccess = { sharp = true })
            }
            detail?.let { (src, g) ->
                AsyncImage(File(src), null, contentScale = ContentScale.FillBounds,
                    modifier = Modifier.offset(with(density) { (g.left * fit.width).toDp() }, with(density) { (g.top * fit.height).toDp() })
                        .size(with(density) { (g.width * fit.width).toDp() }, with(density) { (g.height * fit.height).toDp() }))
            }
            if (clip != null) {
                androidx.compose.runtime.key(clipRun) {
                    AndroidView(factory = { c ->
                        VideoView(c).apply {
                            setVideoPath(clip)
                            setOnPreparedListener { mp -> if (quiet) mp.setVolume(0f, 0f); start() }
                            setOnCompletionListener { onClipEnd() }
                            setOnErrorListener { _, _, _ -> onClipEnd(); true }
                        }
                    }, modifier = Modifier.fillMaxSize().clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) { onClipEnd() })
                }
            }
        }
        // 放大时右下角的小地图：整张照片和此刻看着的那一块
        if (zoom.value > 1.02f) {
            val nw = min(120f, 90f * p.ratio); val nh = nw / p.ratio
            val s = zoom.value; val o = pan.value
            val fx0 = ((-W / 2 - o.x) / (fit.width * s) + 0.5f).coerceIn(0f, 1f); val fx1 = ((W / 2 - o.x) / (fit.width * s) + 0.5f).coerceIn(0f, 1f)
            val fy0 = ((-H / 2 - o.y) / (fit.height * s) + 0.5f).coerceIn(0f, 1f); val fy1 = ((H / 2 - o.y) / (fit.height * s) + 0.5f).coerceIn(0f, 1f)
            Box(Modifier.align(Alignment.BottomEnd).navigationBarsPadding().padding(end = 14.dp, bottom = 150.dp).size(nw.dp, nh.dp).clip(RoundedCornerShape(6.dp)).border(1.dp, Color.White.copy(alpha = 0.5f), RoundedCornerShape(6.dp))) {
                AsyncImage(File(p.thumb), null, Modifier.fillMaxSize(), contentScale = ContentScale.FillBounds)
                Box(Modifier.offset((fx0 * nw).dp, (fy0 * nh).dp).size(((fx1 - fx0) * nw).dp, ((fy1 - fy0) * nh).dp).border(1.5.dp, Color.White, RoundedCornerShape(2.dp)))
            }
        }
    }
}

/** 下面的小图条：当前那张有边框，保留的带一个金点、淘汰的带一个 ×。 */
@Composable
private fun Strip(shown: List<Photo>, decisions: Map<String, Decision>, index: Int, onPick: (String) -> Unit) {
    val c = LocalFg.current
    val state = rememberLazyListState(initialFirstVisibleItemIndex = maxOf(0, index - 3))
    val density = LocalDensity.current
    LaunchedEffect(index) {
        val vw = state.layoutInfo.viewportSize.width
        state.animateScrollToItem(index, -(vw / 2 - with(density) { 26.dp.toPx() }).toInt())
    }
    LazyRow(state = state, contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth().height(56.dp)) {
        itemsIndexed(shown, key = { _, p -> p.path }) { i, p ->
            val cur = i == index
            val st = decisions[p.path]?.state
            Box(
                Modifier.size(if (cur) 56.dp else 44.dp, 56.dp).clip(RoundedCornerShape(8.dp))
                    .border(if (cur) 2.dp else 0.dp, if (cur) Color.White else Color.Transparent, RoundedCornerShape(8.dp))
                    .clickable { onPick(p.path) },
            ) {
                AsyncImage(File(p.thumb), null, Modifier.fillMaxSize().graphicsLayer { alpha = if (st == "reject") 0.55f else 1f }, contentScale = ContentScale.Crop)
                if (st == "keep") Box(Modifier.align(Alignment.TopEnd).padding(4.dp).size(7.dp).clip(CircleShape).background(c.gleam))
            }
        }
    }
}

/** 玻璃上的保留 / 待复查 / 淘汰：选中的保留是琥珀色（只有"保留"用它），其余选中是墨色。 */
@Composable
private fun DecideBtn(st: String, on: Boolean, onClick: () -> Unit) {
    val c = LocalFg.current
    val src = remember { MutableInteractionSource() }
    val press = rememberPress(src)
    Row(
        Modifier.height(46.dp).graphicsLayer { val s = 1f + 0.07f * press(); scaleX = s; scaleY = s }.clip(RoundedCornerShape(50))
            .background(if (on) (if (st == "keep") c.gleam else c.ink) else Color.Transparent)
            .clickable(interactionSource = src, indication = null, onClick = onClick).padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val fg = if (on) (if (st == "keep") c.gleamInk else c.onInk) else c.ink
        StateIcon(st, fg, 14.dp)
        Text(t("state.$st"), style = Type.label.copy(color = fg, fontWeight = FontWeight.SemiBold), modifier = Modifier.padding(start = 6.dp), maxLines = 1)
    }
}

@Composable
private fun DecideIcon(icon: String, label: String, onClick: () -> Unit) {
    val c = LocalFg.current
    Box(Modifier.size(46.dp).clip(CircleShape).clickable(onClick = onClick), contentAlignment = Alignment.Center) { FgIcon(icon, size = 18.dp, color = c.ink) }
}

@Composable
private fun GlassRound(icon: String, label: String, onClick: () -> Unit) {
    val src = remember { MutableInteractionSource() }
    val press = rememberPress(src)
    GlassBox(Modifier.size(44.dp), shape = CircleShape, style = glassStyle(strong = true), press = press) {
        Box(Modifier.fillMaxSize().clickable(interactionSource = src, indication = null, onClick = onClick), contentAlignment = Alignment.Center) { FgIcon(icon, size = 16.dp, stroke = 1.6f, color = LocalFg.current.ink) }
    }
}

@Composable
private fun GlassPill(text: String, on: Boolean, onClick: () -> Unit) {
    val c = LocalFg.current
    val src = remember { MutableInteractionSource() }
    val press = rememberPress(src)
    GlassBox(style = glassStyle(strong = true), press = press) {
        Row(Modifier.clickable(interactionSource = src, indication = null, onClick = onClick).padding(horizontal = 12.dp, vertical = 9.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(8.dp).clip(CircleShape).background(if (on) c.ink else Color.Transparent).border(1.2.dp, c.ink, CircleShape))
            Text(text, style = Type.monoSmall.copy(color = c.ink, fontWeight = FontWeight.SemiBold), modifier = Modifier.padding(start = 6.dp))
        }
    }
}
