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
import androidx.compose.animation.core.tween
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.draw.blur
import com.fleetinggleam.android.core.duration

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
    val probeProg by Session.probeProg.collectAsState()
    val paused by Session.paused.collectAsState()
    val settings by Settings.state.collectAsState()
    val decisions by Session.decisions.collectAsState()
    val status by Projects.status.collectAsState()
    val scope = rememberCoroutineScope()
    var settingsOpen by remember { mutableStateOf(false) }
    var renaming by remember { mutableStateOf(false) }

    // 选片界面的状态：换项目整个换一份
    val cull = remember(project.id) { CullState() }
    /** 分析中点开大图用的（和分析完的那一套分开：分析完换成正式的照片墙）。 */
    val scanCull = remember(project.id) { CullState() }
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
                add(Triple(t("organize.button"), phase == Phase.Done) { organize = true })
                add(Triple(t("cull.reeval"), canReeval) { Session.reevaluate() })
                add(Triple(t("pset.settings"), !scanning) { settingsOpen = true })
                add(Triple(t("styles.title"), true, onOpenStyles))
                add(Triple(t("bin.rename"), true) { renaming = true })
            }, dot = canReeval)
        }
        OrganizeDialogs(project, organize) { organize = false }
        Box(Modifier.fillMaxWidth().height(1.dp).background(c.line))

        // ---- 横幅（和准备页同一套语言，都在页边距里的圆角卡）：分析中是一张暗色封面卡，正在看的那张照片糊在底下、白字压上去；
        //      暂停和分析完的一行总结是白卡
        AnimatedVisibility(scanning || celebrate || paused != Pause.None, enter = expandVertically() + fadeIn(), exit = shrinkVertically() + fadeOut()) {
            Box(Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 4.dp)) {
                if (scanning && paused == Pause.None) ScanningCard(progress) { Session.stopScan() }
                else Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(c.panel).padding(16.dp)) {
                    if (paused != Pause.None) {
                        Text(t(if (paused == Pause.Hot) "android.paused.hot" else "android.paused.battery"), style = Type.body.copy(color = c.ink))
                    } else {
                        val counts = decisions.values.groupingBy { it.state }.eachCount()
                        Text(t("cull.doneTitle"), style = Type.bodyStrong.copy(color = c.ink))
                        Text(t("cull.doneBody", "keep" to (counts["keep"] ?: 0), "review" to (counts["review"] ?: 0), "reject" to (counts["reject"] ?: 0)), style = Type.monoSmall.copy(color = c.ash), modifier = Modifier.padding(top = 2.dp))
                    }
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
                        phase == Phase.Probing && probeProg != null && probeProg!!.total > 0 -> t("cull.probing") + "  ${probeProg!!.done} / ${probeProg!!.total}"
                        else -> t(if (phase == Phase.Probing) "cull.probing" else "cull.loading")
                    }
                    Text(text, style = Type.small.copy(color = c.ash), modifier = Modifier.padding(16.dp))
                }
                when (phase) {
                    Phase.Ready -> ReadyStage(project, uncached, bytes, eta, files.filter { !it.cached }, previews, onStart = start, onAddFolder = addFolder, onRemoveFolder = { Session.removeFolder(it) }, done = files.size - uncached)
                    Phase.Scanning -> ScanningWall(scanCull)
                    Phase.Done -> Culling(project, cull, data, onOpenStyles)
                    else -> {}
                }
            }
        }
    }

    // 大图盖在整页上面（连工具栏一起）
    if (phase == Phase.Done && cull.viewer && cull.selected != null) Viewer(cull, data, decisions) { path, st, order -> decideIn(cull, data, path, st, order) }
    // 分析中：已经出结果的照片可以点开放大看（只看，改判等分析完）
    if (phase == Phase.Scanning && scanCull.viewer && scanCull.selected != null) ScanPeek(scanCull)

    // 要分析这个项目，可另一个项目还在分析：同一时间只分析一个，问一句
    val other by Session.conflict.collectAsState()
    other?.let { o ->
        FgDialog(true, { Session.conflict.value = null }, t("android.cull.busyTitle", "name" to o.name), footer = {
            FgButton(t("organize.cancel"), { Session.conflict.value = null })
            FgButton(t("android.cull.busyStop"), { Session.stopOtherAndStart() }, primary = true)
        }) { Text(t("android.cull.busyBody", "name" to o.name), style = Type.body.copy(color = c.ink)) }
    }
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

/** 分析中：像一张接触印样——和"网格"浏览方式同一套方格，每张照片在它的格子里一张张显影。 */
@Composable
private fun ScanningWall(cull: CullState) {
    val files by Session.files.collectAsState()
    Session.tick.collectAsState().value
    val items = remember(files) { files.map { WallItem(it.path, it.width.toFloat() / it.height.coerceAtLeast(1)) } }
    val state = rememberLazyListState()
    // 分析完的那几张可以点开放大（格子位置记下来，大图从格子里飞出来）
    val open: (String) -> Unit = remember(cull) { { path -> cull.select(path, byTap = true); cull.viewer = true } }
    Wall(items, "grid", state, PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 40.dp)) { i, w, h ->
        val path = items[i].key
        val done = Session.results[path]
        Box(Modifier.onGloballyPositioned { cull.coords[path] = it }) {
            Tile(path, w, h, done, Session.stages[path], "", selected = false, reveal = false, dim = false, manual = false, showScore = false, index = i, interactive = done != null, dock = false, onSelect = open, onOpen = open, onDecide = { _, _ -> })
        }
    }
}

/** 分析中看大图：只看已经分析完的那些（打开那一刻的样子），可以放大、左右翻；改判等分析完。 */
@Composable
private fun ScanPeek(cull: CullState) {
    val shown = remember { Session.files.value.mapNotNull { Session.results[it.path] } }
    if (shown.isEmpty()) { cull.viewer = false; return }
    val data = remember(shown) { CullData(shown, emptyMap(), emptyList(), emptyList(), shown.associateBy { it.path }, null) }
    Viewer(cull, data, emptyMap(), readOnly = true) { _, _, _ -> }
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
