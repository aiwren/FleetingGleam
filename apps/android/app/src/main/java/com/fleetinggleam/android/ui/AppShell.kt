package com.fleetinggleam.android.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import com.fleetinggleam.android.ui.components.glass
import com.fleetinggleam.android.ui.components.glassStyle
import com.fleetinggleam.android.ui.components.LocalBackdrop
import com.kyant.backdrop.backdrops.layerBackdrop
import kotlinx.coroutines.launch
import com.fleetinggleam.android.core.Projects
import com.fleetinggleam.android.core.Session
import com.fleetinggleam.android.core.rememberLang
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.components.Icons
import com.fleetinggleam.android.ui.components.ToastHost
import com.fleetinggleam.android.ui.home.ProjectHome
import com.fleetinggleam.android.ui.models.ModelGate
import com.fleetinggleam.android.ui.picker.FolderPicker
import com.fleetinggleam.android.ui.project.ProjectScreen
import com.fleetinggleam.android.ui.settings.SettingsScreen
import com.fleetinggleam.android.ui.settings.StylesScreen
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Motion
import com.fleetinggleam.android.ui.theme.Type

/** 底部的两个页签（作者 2026-10-21：只有"照片"和"设置"；NAS、模型、备份都在设置里）。 */
private enum class Tab(val label: String, val icon: String) { Cull("android.tab.photos", Icons.CULL), Settings("settings.title", Icons.SETTINGS) }

/**
 * 应用的外壳（桌面端 App.tsx 的手机版）：左侧栏换成浮在底部的液态玻璃标签栏；进了项目它让位给照片墙（像 iOS 推进详情页时那样）。
 * 全局挂着：模型下载对话框、"项目文件夹在哪"的选择器、底部的提示条。
 */
@Composable
fun AppShell() {
    rememberLang()
    val c = LocalFg.current
    val status by Projects.status.collectAsState()
    val store by Projects.store.collectAsState()
    var tab by rememberSaveable { mutableStateOf(Tab.Cull) }
    /** 风格页是从哪儿打开的：它的返回键回到那里。 */
    var styles by rememberSaveable { mutableStateOf<String?>(null) }
    val project = store.projects.firstOrNull { it.id == store.activeId && it.trashed == null }
    // 页面内容录进这一层，悬浮的玻璃件（标签栏）把它折射出来
    val backdrop = com.kyant.backdrop.backdrops.rememberLayerBackdrop()

    BackHandler(enabled = styles != null) { styles = null }
    BackHandler(enabled = styles == null && tab == Tab.Cull && project != null) { Projects.setActive(null) }
    BackHandler(enabled = styles == null && tab == Tab.Settings) { tab = Tab.Cull }

    Box(Modifier.fillMaxSize().background(c.canvas)) {
        if (!status.loaded) return@Box
        Box(Modifier.fillMaxSize().layerBackdrop(backdrop)) {
            val screen = when {
                styles != null -> "styles"
                tab == Tab.Settings -> "settings"
                project != null -> "project:${project.id}"
                else -> "home"
            }
            AnimatedContent(screen, transitionSpec = {
                (fadeIn(tween(Motion.BASE)) + slideInVertically(tween(Motion.BASE, easing = Motion.easeOut)) { it / 24 }) togetherWith fadeOut(tween(Motion.FAST))
            }, label = "screen") { s ->
                when {
                    s == "styles" -> StylesScreen(from = styles ?: "settings", onBack = { styles = null })
                    s == "settings" -> SettingsScreen(onOpenStyles = { styles = "settings" })
                    s.startsWith("project:") -> ProjectScreen(onOpenStyles = { styles = "culling" })
                    else -> ProjectHome(onOpenStyles = { styles = "culling" })
                }
            }
        }
        val showBar = styles == null && (tab == Tab.Settings || project == null)
        AnimatedVisibility(showBar, modifier = Modifier.align(Alignment.BottomCenter), enter = fadeIn() + slideInVertically { it }, exit = fadeOut() + slideOutVertically { it }) {
            FloatingTabBar(backdrop, tab) { next -> if (next == Tab.Cull && tab == Tab.Cull && project != null) Projects.setActive(null); tab = next }
        }
        ToastHost(bottom = if (showBar) 80.dp else 24.dp)
        // 转到后台的模型下载：顶部一粒玻璃小胶囊，点一下把对话框叫回来
        val gate by com.fleetinggleam.android.core.Models.gate.collectAsState()
        AnimatedVisibility(gate?.hidden == true, modifier = Modifier.align(Alignment.TopCenter), enter = fadeIn() + slideInVertically { -it }, exit = fadeOut() + slideOutVertically { -it }) {
            val g = gate
            androidx.compose.runtime.CompositionLocalProvider(LocalBackdrop provides backdrop) {
                com.fleetinggleam.android.ui.components.GlassBox(Modifier.statusBarsPadding().padding(top = 6.dp), style = glassStyle(strong = true)) {
                    Row(Modifier.clip(CircleShape).clickable { com.fleetinggleam.android.core.Models.showGate() }.padding(horizontal = 14.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        val f = if (g != null && g.total > 0) g.done.toFloat() / g.total else 0f
                        androidx.compose.foundation.Canvas(Modifier.size(14.dp)) {
                            drawCircle(c.line, style = androidx.compose.ui.graphics.drawscope.Stroke(2.dp.toPx()))
                            drawArc(c.ink, -90f, 360f * f, false, style = androidx.compose.ui.graphics.drawscope.Stroke(2.dp.toPx(), cap = androidx.compose.ui.graphics.StrokeCap.Round))
                        }
                        Text(t("android.models.pill", "pct" to Math.round(f * 100)), style = Type.monoSmall.copy(color = c.ink, fontWeight = FontWeight.Medium), modifier = Modifier.padding(start = 8.dp))
                    }
                }
            }
        }
    }
    ModelGate()
    FolderPicker()
}

/**
 * 底部悬浮的液态玻璃标签栏（iOS 那种）：离屏幕边缘有距离的胶囊，边缘把后面的照片折射弯过来，带高光和投影。
 * 选中的页签是一粒墨色胶囊；手指按在栏上左右拖，胶囊像一滴水那样鼓起、变得通透、跟着手指走，松手吸到最近的页签。
 */
@Composable
private fun FloatingTabBar(backdrop: com.kyant.backdrop.Backdrop, current: Tab, onSelect: (Tab) -> Unit) {
    val c = LocalFg.current
    val density = androidx.compose.ui.platform.LocalDensity.current
    val itemW = 74.dp
    val itemPx = with(density) { itemW.toPx() }
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    // 胶囊的位置（像素）与"被拿起来"的程度
    val x = androidx.compose.runtime.remember { androidx.compose.animation.core.Animatable(current.ordinal * itemPx) }
    val lift = androidx.compose.runtime.remember { androidx.compose.animation.core.Animatable(0f) }
    var dragging by androidx.compose.runtime.remember { mutableStateOf(false) }
    androidx.compose.runtime.LaunchedEffect(current, itemPx) { if (!dragging) x.animateTo(current.ordinal * itemPx, Motion.glide()) }
    val barSrc = androidx.compose.runtime.remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    val press = com.fleetinggleam.android.ui.components.rememberPress(barSrc)
    Box(Modifier.navigationBarsPadding().padding(bottom = 10.dp)) {
        Box(
            Modifier
                .glass(backdrop, RoundedCornerShape(50), glassStyle(), press = { maxOf(press(), lift.value * 0.6f) })
                .padding(4.dp)
                .pointerInput(itemPx) {
                    detectHorizontalDragGestures(
                        onDragStart = { dragging = true; scope.launch { lift.animateTo(1f, androidx.compose.animation.core.spring(0.6f, 420f)) } },
                        onDragEnd = {
                            val i = Math.round(x.value / itemPx).coerceIn(0, Tab.entries.size - 1)
                            dragging = false
                            scope.launch { lift.animateTo(0f, androidx.compose.animation.core.spring(0.5f, 300f)) }
                            scope.launch { x.animateTo(i * itemPx, Motion.glide()) }
                            if (Tab.entries[i] != current) onSelect(Tab.entries[i])
                        },
                        onDragCancel = { dragging = false; scope.launch { lift.animateTo(0f); x.animateTo(current.ordinal * itemPx, Motion.glide()) } },
                    ) { change, dx ->
                        change.consume()
                        scope.launch { x.snapTo((x.value + dx).coerceIn(0f, itemPx * (Tab.entries.size - 1))) }
                    }
                },
        ) {
            // 墨色胶囊：拿起来时放大一点、变得通透（下面的折射透出来）
            Box(
                Modifier
                    .offset { androidx.compose.ui.unit.IntOffset(x.value.toInt(), 0) }
                    .graphicsLayer { val s = 1f + 0.12f * lift.value; scaleX = s; scaleY = s }
                    .width(itemW).height(52.dp).clip(CircleShape)
                    // iOS 26 的选中态：一块更亮 / 更深一点的玻璃，不是实心墨色
                    .background((if (c.dark) Color.White.copy(alpha = 0.14f) else Color.Black.copy(alpha = 0.065f)).copy(alpha = (if (c.dark) 0.14f else 0.065f) * (1f + 0.6f * lift.value))),
            )
            Row(horizontalArrangement = Arrangement.spacedBy(0.dp)) {
                Tab.entries.forEachIndexed { i, tab ->
                    // 文字颜色跟着胶囊走：胶囊盖住多少，就反白多少
                    // 胶囊盖住多少，图标和字就从灰过渡到墨色多少
                    val cover = (1f - kotlin.math.abs(x.value / itemPx - i)).coerceIn(0f, 1f)
                    val fg = androidx.compose.ui.graphics.lerp(c.ash, c.ink, cover)
                    // iOS 的样子：图标在上、文字在下
                    androidx.compose.foundation.layout.Column(
                        Modifier.width(itemW).height(52.dp).clip(CircleShape)
                            .clickable(interactionSource = barSrc, indication = null) { onSelect(tab) },
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.Center,
                    ) {
                        FgIcon(tab.icon, size = 22.dp, stroke = 1.5f, color = fg)
                        Text(t(tab.label), style = Type.monoSmall.copy(fontFamily = com.fleetinggleam.android.ui.theme.Geist, color = fg, fontWeight = FontWeight.SemiBold, fontSize = androidx.compose.ui.unit.TextUnit(10f, androidx.compose.ui.unit.TextUnitType.Sp), lineHeight = androidx.compose.ui.unit.TextUnit(12f, androidx.compose.ui.unit.TextUnitType.Sp)), modifier = Modifier.padding(top = 2.dp), maxLines = 1)
                    }
                }
            }
        }
    }
}
