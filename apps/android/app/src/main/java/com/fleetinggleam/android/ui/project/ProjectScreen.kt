package com.fleetinggleam.android.ui.project

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.fleetinggleam.android.core.Pause
import com.fleetinggleam.android.core.Phase
import com.fleetinggleam.android.core.Pickers
import com.fleetinggleam.android.core.Projects
import com.fleetinggleam.android.core.Session
import com.fleetinggleam.android.core.Settings
import com.fleetinggleam.android.core.fmtBytes
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.Actions
import com.fleetinggleam.android.ui.components.FgButton
import com.fleetinggleam.android.ui.components.FgDialog
import com.fleetinggleam.android.ui.components.FgField
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.components.IconBtn
import com.fleetinggleam.android.ui.components.Icons
import com.fleetinggleam.android.ui.components.LiveBar
import com.fleetinggleam.android.ui.components.Ticker
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.launch

/**
 * 一个项目（桌面端 Culling.tsx 的手机版）：顶部工具栏，分析时的横幅，按阶段显示——读文件夹、开始分析前的灯箱、正在显影的照片墙、分析好的照片墙。
 * 选片界面（筛选、单张 / 对比、检查器、大图、搜索、场景、整理）在 Culling.kt。
 */
@Composable
fun ProjectScreen(onOpenStyles: () -> Unit) {
    val c = LocalFg.current
    val ctx = LocalContext.current
    val store by Projects.store.collectAsState()
    val project = store.projects.firstOrNull { it.id == store.activeId && it.trashed == null } ?: return
    val phase by Session.phase.collectAsState()
    val files by Session.files.collectAsState()
    val bytes by Session.bytes.collectAsState()
    val uncached by Session.uncached.collectAsState()
    val photos by Session.photos.collectAsState()
    val progress by Session.progress.collectAsState()
    val error by Session.error.collectAsState()
    val nasOff by Session.nasOff.collectAsState()
    val eta by Session.etaSeconds.collectAsState()
    val previews by Session.previews.collectAsState()
    val celebrate by Session.celebrate.collectAsState()
    val dirty by Session.dirty.collectAsState()
    val nasProg by Session.nasProg.collectAsState()
    val peopleProg by Session.peopleProg.collectAsState()
    val mediaProg by Session.mediaProg.collectAsState()
    val paused by Session.paused.collectAsState()
    val settings by Settings.state.collectAsState()
    val decisions by Session.decisions.collectAsState()
    Session.tick.collectAsState().value // 分析中每帧刷新一次
    val status by Projects.status.collectAsState()
    val scope = rememberCoroutineScope()
    var settingsOpen by remember { mutableStateOf(false) }
    var renaming by remember { mutableStateOf(false) }

    // 选片界面的状态：换项目整个换一份
    val cull = remember(project.id) { CullState() }
    val sceneText by com.fleetinggleam.android.core.Scenes.texts.collectAsState()
    val data = rememberCullData(cull, photos, decisions, project.sort ?: "group", sceneText)
    SearchEffects(project, cull, phase == Phase.Done)

    val addFolder: () -> Unit = { scope.launch { Actions.pickFolder()?.let { Session.addFolder(it) } } }
    val start: () -> Unit = { scope.launch { Pickers.notificationPermission(ctx); Session.startScan() } }
    val scanning = phase == Phase.Scanning

    Box(Modifier.fillMaxSize()) {
    Column(Modifier.fillMaxSize().statusBarsPadding()) {
        // ---- 顶栏（iOS 导航栏）：‹ 项目 · 名字（点一下改名）· 搜索 · ⋯（项目设置、重新评估、风格）。"整理"在底部玻璃条上。
        var organize by remember { mutableStateOf(false) }
        com.fleetinggleam.android.ui.components.NavBar(project.name, t("home.title"), { Projects.setActive(null) }, onTitle = { renaming = true }) {
            if (phase == Phase.Done && project.search != false) IconBtn(Icons.SEARCH, t("search.label"), { cull.searchOpen = !cull.searchOpen; if (!cull.searchOpen) cull.query = "" }, on = cull.searchOpen, plain = true)
            val canReeval = phase == Phase.Done && settings.learn && project.learn != false && dirty > 0
            com.fleetinggleam.android.ui.components.MenuButton(t("pset.settings"), buildList {
                add(Triple(t("pset.settings"), !scanning) { settingsOpen = true })
                add(Triple(t("cull.reeval"), canReeval) { Session.reevaluate() })
                add(Triple(t("styles.title"), true, onOpenStyles))
                add(Triple(t("bin.rename"), true) { renaming = true })
            }, dot = canReeval)
        }
        OrganizeDialogs(project, organize) { organize = false }
        Box(Modifier.fillMaxWidth().height(1.dp).background(c.line))

        // ---- 横幅：分析中（进度、预计、此刻在做哪张、停止），分析完的一行总结
        AnimatedVisibility(scanning || celebrate || paused != Pause.None, enter = expandVertically() + fadeIn(), exit = shrinkVertically() + fadeOut()) {
            Column(Modifier.fillMaxWidth().background(c.panel).padding(horizontal = 16.dp, vertical = 12.dp)) {
                if (paused != Pause.None) {
                    Text(t(if (paused == Pause.Hot) "android.paused.hot" else "android.paused.battery"), style = Type.small.copy(color = c.ink))
                } else if (scanning) {
                    val elapsed = (System.currentTimeMillis() - Session.startedAt) / 1000f
                    val left = if (progress.done >= 4) maxOf(1, Math.round(elapsed / progress.done * (progress.total - progress.done))) else null
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(t("cull.analyzing"), style = Type.bodyStrong.copy(color = c.ink))
                        Row(Modifier.weight(1f).padding(start = 10.dp), verticalAlignment = Alignment.Bottom) {
                            Ticker(progress.done, Type.mono.copy(color = c.ash))
                            Text(" / ${progress.total}" + (left?.let { " · " + t("cull.eta", "s" to it) } ?: ""), style = Type.mono.copy(color = c.ash), maxLines = 1)
                        }
                        FgButton(t("cull.stop"), { Session.stopScan() }, small = true)
                    }
                    val last = Session.lastStage
                    Text(last?.let { "${it.first.substringAfterLast('/')} · ${t("cull.stage.${it.second}")}" } ?: t("cull.loadingModel"), style = Type.monoSmall.copy(color = c.ash), maxLines = 1, overflow = Ellipsis, modifier = Modifier.padding(top = 2.dp))
                    LiveBar(if (progress.total > 0) progress.done.toFloat() / progress.total else 0f, Modifier.padding(top = 8.dp))
                } else {
                    val counts = decisions.values.groupingBy { it.state }.eachCount()
                    Text(t("cull.doneTitle"), style = Type.bodyStrong.copy(color = c.ink))
                    Text(t("cull.doneBody", "keep" to (counts["keep"] ?: 0), "review" to (counts["review"] ?: 0), "reject" to (counts["reject"] ?: 0)), style = Type.monoSmall.copy(color = c.ash))
                }
            }
        }

        Box(Modifier.weight(1f).fillMaxWidth()) {
            Column(Modifier.fillMaxSize()) {
                val msgs = buildList {
                    if (error != null && error != "cull.nasOffline" && error != "cull.nasDisabled") add(t(error!!).let { if (it == error) error!! else it })
                    if (status.saveError != null) add(t("cull.saveFailed", "error" to status.saveError))
                }
                msgs.forEach { Text(it, style = Type.small.copy(color = c.ink), modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) }
                if (error == "cull.nasOffline" || error == "cull.nasDisabled") Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(t(error!!, "name" to nasOff), style = Type.small.copy(color = c.ink), modifier = Modifier.weight(1f))
                    FgButton(t("cull.nasRetry"), { Session.reload() }, small = true)
                }
                if (phase == Phase.Probing || phase == Phase.Loading) {
                    val text = when {
                        nasProg != null -> nasProg!!.let { p -> if (p.bytesTotal > 0) t("cull.fetchingNas", "share" to p.share, "done" to p.done, "total" to p.total, "size" to fmtBytes(p.bytesTotal)) else t("android.mirrorSync", "share" to p.share, "done" to p.done, "total" to p.total) }
                        phase == Phase.Loading && mediaProg != null && mediaProg!!.done < mediaProg!!.total -> t("cull.preparingMedia", "done" to mediaProg!!.done, "total" to mediaProg!!.total)
                        phase == Phase.Loading && peopleProg != null && peopleProg!!.done < peopleProg!!.total -> t("cull.recognising", "done" to peopleProg!!.done, "total" to peopleProg!!.total)
                        else -> t(if (phase == Phase.Probing) "cull.probing" else "cull.loading")
                    }
                    Text(text, style = Type.small.copy(color = c.ash), modifier = Modifier.padding(16.dp))
                }
                when (phase) {
                    Phase.Ready -> ReadyStage(project, uncached, bytes, eta, files.filter { !it.cached }, previews, onStart = start, onAddFolder = addFolder, onRemoveFolder = { Session.removeFolder(it) })
                    Phase.Scanning -> ScanningWall()
                    Phase.Done -> Culling(project, cull, data, onOpenStyles, onOrganize = { organize = true })
                    else -> {}
                }
            }
        }
    }

    // 大图盖在整页上面（连工具栏一起）
    if (phase == Phase.Done && cull.viewer && cull.selected != null) Viewer(cull, data, decisions) { path, st -> decideIn(cull, data, path, st) }
    }

    ProjectSettings(settingsOpen, project, onClose = { settingsOpen = false }, onAddFolder = addFolder, onRemoveFolder = { Session.removeFolder(it) },
        onTrash = { settingsOpen = false; Projects.trash(project.id) }, onForceRescan = { settingsOpen = false; Session.forceRescan() })
    if (renaming) {
        var name by remember { mutableStateOf(project.name) }
        FgDialog(true, { renaming = false }, t("pset.name"), footer = {
            FgButton(t("organize.cancel"), { renaming = false })
            FgButton(t("organize.done"), { name.trim().takeIf { it.isNotEmpty() }?.let { v -> Projects.update(project.id) { it.copy(name = v) } }; renaming = false }, primary = true)
        }) { FgField(name, { name = it }) }
    }
}

/** 分析中：每张照片都已经在它最终的位置上（真实的宽高比来自文件头），一张张显影。 */
@Composable
private fun ScanningWall() {
    val files by Session.files.collectAsState()
    Session.tick.collectAsState().value
    val items = remember(files) { files.map { WallItem(it.path, it.width.toFloat() / it.height.coerceAtLeast(1)) } }
    val state = rememberLazyListState()
    Wall(items, "justified", state, PaddingValues(start = 12.dp, end = 12.dp, top = 12.dp, bottom = 40.dp)) { i, w, h ->
        val path = items[i].key
        Tile(path, w, h, Session.results[path], Session.stages[path], "", selected = false, reveal = false, dim = false, manual = false, showScore = false, index = i, interactive = false, onSelect = {}, onOpen = {}, onDecide = { _, _ -> })
    }
}
