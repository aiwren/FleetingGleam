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
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.derivedStateOf
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
import androidx.compose.ui.draw.drawWithContent
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.foundation.layout.Spacer
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
import kotlinx.coroutines.flow.collectLatest
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
fun Viewer(cull: CullState, data: CullData, decisions: Map<String, Decision>, readOnly: Boolean = false, onDecide: (String, String?, List<Photo>) -> Unit) {
    // 大图开着时照片的顺序和成员定在打开那一刻：在"保留"页签里把一张改成淘汰，它不会从翻页器里消失
    // （以前会：后面的照片整体前移一位，页码对不上，翻页器和选中来回跳，就是一直闪）。照片记录本身按路径取最新的（转向等）
    val order = remember { data.shown.map { it.path } }
    val shown = remember(data.byPath) { order.mapNotNull { data.byPath[it] } }
    val indexOf = { path: String? -> if (path == null) -1 else shown.indexOfFirst { it.path == path } }
    val start = indexOf(cull.selected).coerceAtLeast(0)
    if (shown.isEmpty()) { cull.viewer = false; return }
    val ctx = LocalContext.current
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    val haptic = LocalHapticFeedback.current
    val pager = rememberPagerState(initialPage = start) { shown.size }
    val photo = shown.getOrNull(pager.currentPage) ?: shown[0]
    // 眼前这页放大了没有（只在跨过 1 倍时变：手势检测器和翻页开关跟着它，不再每帧重启）
    var zoomed by remember { mutableStateOf(false) }
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
    LaunchedEffect(cull.selected) { val i = indexOf(cull.selected); if (i >= 0 && i != pager.currentPage && !pager.isScrollInProgress) pager.animateScrollToPage(i) }
    LaunchedEffect(Unit) { enter.animateTo(1f, Motion.view()) }

    // 进入和关闭用同一条弹簧（设计审查 A9.5）
    fun close() {
        scope.launch {
            enter.animateTo(0f, Motion.view())
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

    // 进出场和下拉每帧都在变：只在 graphicsLayer 里读；显隐条件用 derivedStateOf，整页不跟着每帧重组
    val dismissPx = with(density) { 360.dp.toPx() }
    fun fadeNow() = (1f - abs(drag.value) / dismissPx).coerceIn(0f, 1f)
    val chromeIn by remember { derivedStateOf { enter.value > 0.6f } }
    val barsIn by remember { derivedStateOf { enter.value > 0.6f && fadeNow() > 0.7f } }
    Box(Modifier.fillMaxSize().onGloballyPositioned { stage = it.boundsInRoot() }) {
        // ---- 房间：深色底，照片的模糊副本把它照亮
        Box(Modifier.fillMaxSize().graphicsLayer { alpha = enter.value * fadeNow() }.background(ROOM)) {
            AsyncImage(File(photo.thumb), null, Modifier.fillMaxSize().blur(60.dp).graphicsLayer { alpha = 0.42f }, contentScale = ContentScale.Crop)
        }
        // ---- 照片
        Box(Modifier.fillMaxSize().then(if (com.fleetinggleam.android.ui.components.LocalGlassOn.current) Modifier.layerBackdrop(backdrop) else Modifier)) {
            HorizontalPager(
                pager, Modifier.fillMaxSize().pointerInput(zoomed) {
                    // 没放大时竖着拖：往下关、往上看详情
                    if (zoomed) return@pointerInput
                    detectVerticalDragGestures(
                        onDragEnd = {
                            val v = drag.value
                            when {
                                v > with(density) { 110.dp.toPx() } -> close()
                                v < -with(density) { 110.dp.toPx() } -> { if (!readOnly) info = true; scope.launch { drag.animateTo(0f, Motion.glide()) } }
                                else -> scope.launch { drag.animateTo(0f, Motion.glide()) }
                            }
                        },
                        onDragCancel = { scope.launch { drag.animateTo(0f, Motion.glide()) } },
                    ) { change, d -> change.consume(); scope.launch { drag.snapTo(drag.value + if (drag.value < 0) d * 0.5f else d) } }
                },
                userScrollEnabled = !zoomed, beyondViewportPageCount = 1, key = { shown[it].path },
            ) { page ->
                val p = shown[page]
                val isOrigin = page == start
                ZoomPage(
                    p, active = page == pager.currentPage, resident = page == pager.currentPage || page == pager.settledPage,
                    clip = if (page == pager.currentPage) clip else null, clipRun = playing, quiet = quiet, onClipEnd = { clip = null },
                    onZoomed = { if (page == pager.currentPage) zoomed = it },
                    onTap = { chrome = !chrome },
                    modifier = Modifier.graphicsLayer {
                        val e = enter.value; val dy = drag.value; val fade = fadeNow()
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
                        if (dy > 0) { val s = 1f - 0.1f * (1f - fade); scaleX *= s; scaleY *= s }
                    },
                )
            }
        }

        // ---- 上面一条：关闭、名字、标签、实况、第几张。查看器总是暗的房间，所以玻璃件和文字一律用深色那套
        CompositionLocalProvider(LocalBackdrop provides backdrop, LocalFg provides com.fleetinggleam.android.ui.theme.Dark) {
            AnimatedVisibility(chrome && chromeIn, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.align(Alignment.TopCenter)) {
                // 顶栏（设计审查 A9.1 / A9.3 / A9.4）：左 × ，中间文件名 + 页码，右边实况、旋转、检查器；全部 40dp 玻璃圆
                Row(Modifier.fillMaxWidth().statusBarsPadding().padding(horizontal = 12.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(8.dp)) {
                    GlassRound(Icons.CLOSE, t("viewer.close")) { close() }
                    GlassBox(Modifier.weight(1f, fill = false), style = glassStyle(strong = true)) {
                        Row(Modifier.padding(horizontal = 14.dp, vertical = 9.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text(photo.name, style = Type.monoSmall.copy(color = LocalFg.current.ink), maxLines = 1, overflow = Ellipsis, modifier = Modifier.weight(1f, fill = false))
                            Text("  ${pager.currentPage + 1} / ${shown.size}", style = Type.monoSmall.copy(color = LocalFg.current.ash), maxLines = 1)
                        }
                    }
                    Spacer(Modifier.weight(1f))
                    if (!readOnly) {
                        com.fleetinggleam.android.ui.components.GlassIconBtn(Icons.ROTATE, t("viewer.rotate"), { haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove); scope.launch { runCatching { Session.rotate(photo.path) } } })
                        com.fleetinggleam.android.ui.components.GlassIconBtn(Icons.INSPECTOR, t("cull.inspector"), { info = true })
                    }
                    if (photo.motion) {
                        val overlay = com.fleetinggleam.android.ui.components.LocalOverlay.current
                        val anchor = com.fleetinggleam.android.ui.components.rememberAnchor()
                        Box(Modifier.anchor(anchor)) {
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
            AnimatedVisibility(chrome && barsIn, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.align(Alignment.BottomCenter)) {
                Column(Modifier.fillMaxWidth().navigationBarsPadding().padding(bottom = 10.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    // 小图条和单张同一规格（A7.5）：当前项白色描边
                    Filmstrip(shown, decisions, pager.currentPage, Modifier.fillMaxWidth(), ring = Color.White) { cull.select(it) }
                    val d = decisions[photo.path]
                    // 只看（分析中点开的）：不放改判。底部胶囊只剩三个判定项；"待复查"只放图标（A9.1）
                    if (!readOnly) GlassBox(Modifier.padding(top = 6.dp), shape = RoundedCornerShape(50)) {
                        Row(Modifier.padding(6.dp), verticalAlignment = Alignment.CenterVertically) {
                            listOf("keep", "review", "reject").forEach { st ->
                                val on = d?.state == st
                                DecideBtn(st, on, iconOnly = st == "review") { haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove); onDecide(photo.path, if (on && d?.manual == true) null else st, shown) }
                            }
                        }
                    }
                }
            }
        }
    }

    // 暗房间里拉上来的检查器也是暗的（设计审查 A9.2）
    if (info) {
        val mates = remember(photo, data) { Session.photos.value.filter { it.group == photo.group } }
        com.fleetinggleam.android.ui.components.GlassSheet(true, { info = false }, dark = true) {
            Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(horizontal = 16.dp).padding(bottom = 24.dp)) {
                InspectorBody(photo, decisions[photo.path], mates, onDecide = { onDecide(photo.path, it, shown) }, onSelect = { cull.select(it) })
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
private fun ZoomPage(p: Photo, active: Boolean, resident: Boolean, clip: String?, clipRun: Int, quiet: Boolean, onClipEnd: () -> Unit, onZoomed: (Boolean) -> Unit, onTap: () -> Unit, modifier: Modifier) {
    val ctx = LocalContext.current
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    // 以缩略图的路径为键：照片转过之后缩略图换了文件，大图、清晰块和缩放都从头来
    val zoom = remember(p.thumb) { Animatable(1f) }
    val pan = remember(p.thumb) { Animatable(Offset.Zero, Offset.VectorConverter) }
    // 大图解码的最长边：屏幕长边 × 2（再细由下面的清晰块接手），封顶 4096。一张 4096² 就是 64 MB，以前左右两页也各解一张
    val decodeMax = remember { ctx.resources.displayMetrics.let { min(4096, max(it.widthPixels, it.heightPixels) * 2) } }
    // 大图只给眼前这一页（翻页途中还看得到的上一页也留着）；左右预备的那两页先只放缩略图，翻到了再解
    var large by remember(p.thumb) { mutableStateOf(if (active) Large.ready(p.path, true) else null) }
    var sharp by remember(p.thumb) { mutableStateOf(false) }
    var detail by remember(p.thumb) { mutableStateOf<Pair<String, androidx.compose.ui.geometry.Rect>?>(null) }
    LaunchedEffect(p.thumb, active, resident) {
        if (!resident) { large = null; sharp = false; detail = null; return@LaunchedEffect }
        if (large == null && active) large = Large.get(p.path, full = true) ?: Large.get(p.path)
    }
    // 只在跨过 1 倍时告诉外面（以前每帧回调一次，外面的状态跟着每帧变）
    LaunchedEffect(zoom, active) { if (active) snapshotFlow { zoom.value > 1.01f }.distinctUntilChanged().collect { onZoomed(it) } }

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
        // 清晰的部分：放大到比已经解码的（最长边 decodeMax）还细时，按屏幕上那一块从原图解出来
        // 缩放 / 平移经 snapshotFlow 进来、停 160 毫秒才取（collectLatest：手还在动就作废重来），不再每帧重启一个 LaunchedEffect
        LaunchedEffect(zoom, active, fit, W, H) {
            if (!active) return@LaunchedEffect
            snapshotFlow { zoom.value to pan.value }.collectLatest { (s, o) ->
                val long = max(p.width, p.height).toFloat()
                val have = min(long, decodeMax.toFloat())
                // 原图不比已解码的大，或者屏幕上画出来的还没超过已解码的像素：不用再取
                if (long <= have * 1.05f || max(fit.width, fit.height) * s <= have * 0.9f) {
                    if (s < 1.05f) detail = null
                    return@collectLatest
                }
                delay(160)
                // 屏幕上看得到的那一块（照片的比例坐标），四周再留一些，拖一拖还在清晰范围里
                val x0 = ((-W / 2 - o.x) / (fit.width * s) + 0.5f).coerceIn(0f, 1f); val x1 = ((W / 2 - o.x) / (fit.width * s) + 0.5f).coerceIn(0f, 1f)
                val y0 = ((-H / 2 - o.y) / (fit.height * s) + 0.5f).coerceIn(0f, 1f); val y1 = ((H / 2 - o.y) / (fit.height * s) + 0.5f).coerceIn(0f, 1f)
                val cur = detail?.second
                if (cur != null && x0 >= cur.left && y0 >= cur.top && x1 <= cur.right && y1 <= cur.bottom) return@collectLatest
                val px = (x1 - x0) * 0.3f; val py = (y1 - y0) * 0.3f
                val g = androidx.compose.ui.geometry.Rect((x0 - px).coerceAtLeast(0f), (y0 - py).coerceAtLeast(0f), (x1 + px).coerceAtMost(1f), (y1 + py).coerceAtMost(1f))
                val outW = max(800, min((g.width * p.width).toInt(), (g.width * fit.width * s * 1.1f).toInt()))
                runCatching { Core.get<String>("photo_detail", args("path" to p.path, "x" to g.left, "y" to g.top, "w" to g.width, "h" to g.height, "out_w" to outW)) }.getOrNull()?.let { detail = it to g }
            }
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
                // 不进 Coil 的内存缓存：几十 MB 一张，进去就把墙上那些 480 px 的缩略图挤出去，退回墙上又得重解
                val req = remember(src) { ImageRequest.Builder(ctx).data(File(src)).size(decodeMax, decodeMax).memoryCachePolicy(coil3.request.CachePolicy.DISABLED).build() }
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
        // 放大时右下角的小地图：整张照片和此刻看着的那一块；倍数写在地图上方（A9.4）。显不显示用 derivedStateOf，框在绘制阶段读，平移时不重组
        val showMap by remember(zoom) { derivedStateOf { zoom.value > 1.02f } }
        if (showMap) ZoomMap(p, zoom, pan, W, H, fit, Modifier.align(Alignment.BottomEnd))
    }
}

/** 小地图：倍数只在整数百分比变了时重组；看着的那一块是画出来的，每帧读缩放 / 平移也只重画、不重组。 */
@Composable
private fun ZoomMap(p: Photo, zoom: Animatable<Float, *>, pan: Animatable<Offset, *>, W: Float, H: Float, fit: androidx.compose.ui.geometry.Rect, modifier: Modifier) {
    val nw = min(120f, 90f * p.ratio); val nh = nw / p.ratio
    val pct by remember(zoom) { derivedStateOf { Math.round(zoom.value * 100) } }
    Column(modifier.navigationBarsPadding().padding(end = 14.dp, bottom = 150.dp), horizontalAlignment = Alignment.End) {
        Text("$pct%", style = Type.monoSmall.copy(color = Color.White.copy(alpha = 0.85f)), modifier = Modifier.padding(bottom = 4.dp))
        Box(Modifier.size(nw.dp, nh.dp).clip(RoundedCornerShape(6.dp)).border(1.dp, Color.White.copy(alpha = 0.5f), RoundedCornerShape(6.dp)).drawWithContent {
            drawContent()
            val s = zoom.value; val o = pan.value
            val fx0 = ((-W / 2 - o.x) / (fit.width * s) + 0.5f).coerceIn(0f, 1f); val fx1 = ((W / 2 - o.x) / (fit.width * s) + 0.5f).coerceIn(0f, 1f)
            val fy0 = ((-H / 2 - o.y) / (fit.height * s) + 0.5f).coerceIn(0f, 1f); val fy1 = ((H / 2 - o.y) / (fit.height * s) + 0.5f).coerceIn(0f, 1f)
            val stroke = 1.5.dp.toPx()
            drawRoundRect(Color.White, topLeft = Offset(fx0 * size.width + stroke / 2, fy0 * size.height + stroke / 2),
                size = androidx.compose.ui.geometry.Size(((fx1 - fx0) * size.width - stroke).coerceAtLeast(0f), ((fy1 - fy0) * size.height - stroke).coerceAtLeast(0f)),
                cornerRadius = androidx.compose.ui.geometry.CornerRadius(2.dp.toPx()), style = androidx.compose.ui.graphics.drawscope.Stroke(stroke))
        }) {
            AsyncImage(File(p.thumb), null, Modifier.fillMaxSize(), contentScale = ContentScale.FillBounds)
        }
    }
}

/** 玻璃上的保留 / 待复查 / 淘汰（R13 的玻璃版）：选中的保留是琥珀色（只有"保留"用它），其余选中是墨色；44dp 高。`iconOnly` 只放图标。 */
@Composable
private fun DecideBtn(st: String, on: Boolean, iconOnly: Boolean = false, onClick: () -> Unit) {
    val c = LocalFg.current
    val src = remember { MutableInteractionSource() }
    val press = rememberPress(src)
    val bg by androidx.compose.animation.animateColorAsState(if (on) (if (st == "keep") c.gleam else c.ink) else Color.Transparent, Motion.fast(), label = "decideBg")
    Row(
        Modifier.height(44.dp).graphicsLayer { val s = 1f + 0.07f * press(); scaleX = s; scaleY = s }.clip(RoundedCornerShape(50))
            .background(bg)
            .clickable(interactionSource = src, indication = null, onClick = onClick).padding(horizontal = if (iconOnly) 15.dp else 16.dp)
            .semantics { contentDescription = t("state.$st") },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val fg = if (on) (if (st == "keep") c.gleamInk else c.onInk) else c.ink
        StateIcon(st, fg, 14.dp)
        if (!iconOnly) Text(t("state.$st"), style = Type.label.copy(color = fg, fontWeight = FontWeight.SemiBold), modifier = Modifier.padding(start = 6.dp), maxLines = 1)
    }
}

@Composable
private fun GlassRound(icon: String, label: String, onClick: () -> Unit) {
    val src = remember { MutableInteractionSource() }
    val press = rememberPress(src)
    GlassBox(Modifier.size(40.dp), shape = CircleShape, style = glassStyle(strong = true), press = press) {
        Box(Modifier.fillMaxSize().clickable(interactionSource = src, indication = null, onClick = onClick).semantics { contentDescription = label }, contentAlignment = Alignment.Center) { FgIcon(icon, size = 16.dp, stroke = 1.6f, color = LocalFg.current.ink) }
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
