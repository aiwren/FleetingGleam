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
import dev.chrisbanes.haze.HazeState
import dev.chrisbanes.haze.hazeEffect
import dev.chrisbanes.haze.hazeSource
import dev.chrisbanes.haze.materials.ExperimentalHazeMaterialsApi
import dev.chrisbanes.haze.materials.HazeMaterials
import dev.chrisbanes.haze.rememberHazeState

/** 底部的两个页签（作者 2026-10-21：只有"照片"和"设置"；NAS、模型、备份都在设置里）。 */
private enum class Tab(val label: String, val icon: String) { Cull("nav.cull", Icons.CULL), Settings("settings.title", Icons.SETTINGS) }

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
    val haze = rememberHazeState()

    BackHandler(enabled = styles != null) { styles = null }
    BackHandler(enabled = styles == null && tab == Tab.Cull && project != null) { Projects.setActive(null) }
    BackHandler(enabled = styles == null && tab == Tab.Settings) { tab = Tab.Cull }

    Box(Modifier.fillMaxSize().background(c.canvas)) {
        if (!status.loaded) return@Box
        Box(Modifier.fillMaxSize().hazeSource(haze)) {
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
            FloatingTabBar(haze, tab) { next -> if (next == Tab.Cull && tab == Tab.Cull && project != null) Projects.setActive(null); tab = next }
        }
        ToastHost(bottom = if (showBar) 104.dp else 24.dp)
    }
    ModelGate()
    FolderPicker()
}

/**
 * 底部悬浮的液态玻璃标签栏：离屏幕边缘有距离的胶囊，对后面的内容实时模糊并稍稍着色（Android 12+；更早的退为半透明），
 * 一圈上亮下暗的高光描边给它厚度，选中的页签是一粒墨色胶囊、在两个页签之间滑过去。
 */
@OptIn(ExperimentalHazeMaterialsApi::class)
@Composable
private fun FloatingTabBar(haze: HazeState, current: Tab, onSelect: (Tab) -> Unit) {
    val c = LocalFg.current
    val shape = RoundedCornerShape(32.dp)
    val rim = Brush.verticalGradient(listOf(Color.White.copy(alpha = if (c.dark) 0.35f else 0.9f), Color.White.copy(alpha = 0.08f), Color.Black.copy(alpha = if (c.dark) 0.35f else 0.08f)))
    val itemW = 116.dp
    Box(Modifier.navigationBarsPadding().padding(bottom = 14.dp)) {
        Box(
            Modifier
                .shadow(18.dp, shape, ambientColor = Color.Black.copy(alpha = 0.25f), spotColor = Color.Black.copy(alpha = 0.3f))
                .clip(shape)
                .hazeEffect(state = haze, style = HazeMaterials.ultraThin(c.panel)) { blurRadius = 28.dp; noiseFactor = 0.02f }
                .border(1.dp, rim, shape)
                .padding(6.dp),
        ) {
            val x by animateDpAsState(itemW * current.ordinal, Motion.glide(), label = "tabpill")
            Box(Modifier.offset(x = x).width(itemW).height(48.dp).clip(CircleShape).background(c.ink))
            Row(horizontalArrangement = Arrangement.spacedBy(0.dp)) {
                Tab.entries.forEach { tab ->
                    val sel = tab == current
                    Row(
                        Modifier.width(itemW).height(48.dp).clip(CircleShape).clickable { onSelect(tab) },
                        horizontalArrangement = Arrangement.Center,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        FgIcon(tab.icon, size = 18.dp, color = if (sel) c.onInk else c.ink)
                        Text(t(tab.label), style = Type.label.copy(color = if (sel) c.onInk else c.ink, fontWeight = FontWeight.SemiBold), modifier = Modifier.padding(start = 8.dp), maxLines = 1)
                    }
                }
            }
        }
    }
}
