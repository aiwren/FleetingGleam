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
import com.kyant.backdrop.backdrops.layerBackdrop
import androidx.compose.foundation.layout.height
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
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.launch
import androidx.compose.runtime.LaunchedEffect

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
    // 这里只收变得少的；分析 / 读取中一秒几十次的（进度、tick、各种读取进度）下沉到用它们的组件里收，
    // 不然每个事件整页重组（审查第二批）。确认页那几样（文件、大小、预计时间、预览）也只在确认页收
    val phase by Session.phase.collectAsState()
    val photos by Session.photos.collectAsState()
    val error by Session.error.collectAsState()
    val nasOff by Session.nasOff.collectAsState()
    val offline by Session.offline.collectAsState()
    val nasFailure by Session.nasFailure.collectAsState()
    val celebrate by Session.celebrate.collectAsState()
    val dirty by Session.dirty.collectAsState()
    val undoN by Session.undoCount.collectAsState()
    val paused by Session.paused.collectAsState()
    val settings by Settings.state.collectAsState()
    val decisions by Session.decisions.collectAsState()
    val status by Projects.status.collectAsState()
    val scope = rememberCoroutineScope()
    var settingsOpen by remember { mutableStateOf(false) }
    var renaming by remember { mutableStateOf(false) }

    // 选片界面的状态：换项目整个换一份；页签、筛选、选中、场景页离开再回来还在（外面按项目给了 SaveableStateProvider）
    val cull = rememberSaveable(project.id, saver = CullState.Saver) { CullState() }
    val sceneText by com.fleetinggleam.android.core.Scenes.texts.collectAsState()
    val data = rememberCullData(cull, photos, decisions, project.sort ?: "group", sceneText, project.stars)
    SearchEffects(project, cull, phase == Phase.Done)

    val addFolder: () -> Unit = { scope.launch { Actions.pickFolder()?.let { Session.addFolder(it) } } }
    val start: () -> Unit = { scope.launch { Pickers.notificationPermission(ctx); Session.startScan() } }
    val scanning = phase == Phase.Scanning

    val actions = rememberCullActions(cull, data)
    // 分析中的页签数据只在分析阶段算；是个 State，只有照片墙和页签条读它
    val scanData = if (scanning) rememberScanData(cull) else null
    val density = androidx.compose.ui.platform.LocalDensity.current
    val topPad = with(density) { cull.chromeHeight.toDp() }
    val glassOn = com.fleetinggleam.android.ui.components.LocalGlassOn.current
    // 这一页自己的 backdrop：内容录进去，顶栏、页签、底栏这些玻璃件从上面折射它（作者 2026-10-23：全局液态玻璃）
    val backdrop = com.kyant.backdrop.backdrops.rememberLayerBackdrop()
    var organize by remember { mutableStateOf(false) }

    Box(Modifier.fillMaxSize()) {
    // ---- 内容层：顶部从玻璃顶栏下面开始（各阶段自己按 topPad 留出）
    Box(Modifier.fillMaxSize().then(if (glassOn) Modifier.layerBackdrop(backdrop) else Modifier)) {
        Column(Modifier.fillMaxSize()) {
            val msgs = buildList {
                if (error != null && error != "cull.nasOffline" && error != "cull.nasDisabled" && error != "cull.safExpired") add(t(error!!).let { if (it == error) error!! else it })
                if (status.saveError != null) add(t("cull.saveFailed", "error" to status.saveError))
            }
            val plainTop = phase != Phase.Scanning && phase != Phase.Done
            if (plainTop) androidx.compose.foundation.layout.Spacer(Modifier.height(topPad))
            msgs.forEach { Text(it, style = Type.small.copy(color = c.ink), modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) }
            // 连不上分开说：密码不对 → 去设置；没权限；连不上 → 重试（审查第 33 条）
            if (error == "cull.nasOffline" || error == "cull.nasDisabled") Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                val why = if (error == "cull.nasOffline") nasFailure else "net"
                val text = when (why) { "auth" -> t("android.nas.authError", "name" to nasOff); "denied" -> t("android.nas.deniedError", "name" to nasOff); else -> t(error!!, "name" to nasOff) }
                Text(text, style = Type.small.copy(color = c.ink), modifier = Modifier.weight(1f))
                if (why == "auth") FgButton(t("android.nas.toSettings"), { com.fleetinggleam.android.core.Nav.openSettings("nas") }, small = true)
                else FgButton(t("cull.nasRetry"), { Session.reload() }, small = true)
            }
            // 系统文件夹的读权限失效：重新在系统选择器里选一次原来那个文件夹
            if (error == "cull.safExpired") Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(t("cull.safExpired", "name" to nasOff), style = Type.small.copy(color = c.ink), modifier = Modifier.weight(1f))
                FgButton(t("cull.safReauth"), {
                    scope.launch {
                        val uri = Pickers.openTree() ?: return@launch
                        when (Session.reauthorize(uri)) {
                            com.fleetinggleam.android.core.Media.Reauth.Ok -> {}
                            com.fleetinggleam.android.core.Media.Reauth.WrongFolder -> com.fleetinggleam.android.ui.components.Toasts.show(t("cull.safWrongFolder", "name" to nasOff))
                            com.fleetinggleam.android.core.Media.Reauth.NotGranted -> com.fleetinggleam.android.ui.components.Toasts.show(t("cull.safNotGranted", "name" to nasOff))
                        }
                    }
                }, small = true)
            }
            // 读取中：居中一行灰字（设计审查 A5 附注）
            if (phase == Phase.Probing || phase == Phase.Loading) LoadingLine(phase)
            // 分析完 / 暂停的一句总结：页边距里的白卡（分析完的那张 3 秒后自己收起）
            val doneTop = phase == Phase.Done && (celebrate || paused != Pause.None)
            Box(Modifier.weight(1f).fillMaxWidth()) {
                when (phase) {
                    Phase.Ready -> ReadyHost(project, paused, onStart = start, onAddFolder = addFolder)
                    // 分析中就是选片界面：页签 + 照片墙，照片一出结果就落进对应的页签（Scanning.kt）
                    Phase.Scanning -> if (scanData != null) ScanningStage(cull, scanData, { Session.stopScan() }, topPad = topPad, bottomPad = 92.dp)
                    Phase.Done -> CullingContent(project, cull, data, actions, topPad = topPad)
                    else -> {}
                }
                androidx.compose.animation.AnimatedVisibility(visible = doneTop, modifier = Modifier.align(Alignment.TopCenter).padding(top = topPad), enter = expandVertically() + fadeIn(), exit = shrinkVertically() + fadeOut()) {
                    Box(Modifier.padding(start = 16.dp, end = 16.dp, top = 8.dp)) {
                        Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(c.panel).padding(16.dp)) {
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
            }
        }
    }

    // ---- 浮层（玻璃，在 backdrop 之外）：顶栏 + 页签 + 窄胶囊；分析完时的底栏与提示
    androidx.compose.runtime.CompositionLocalProvider(com.fleetinggleam.android.ui.components.LocalBackdrop provides backdrop) {
        com.fleetinggleam.android.ui.components.GlassBar(Modifier.align(Alignment.TopCenter).onGloballyPositioned { cull.chromeHeight = it.size.height }) {
            Column(Modifier.fillMaxWidth().statusBarsPadding()) {
                // 顶栏（iOS 导航栏）：‹ 项目 · 名字（点一下改名）· 搜索 · ⋯；场景页时是 ‹ 名字 · 场景
                val scene = phase == Phase.Done && cull.sceneView
                com.fleetinggleam.android.ui.components.NavBar(
                    if (scene) t("cull.scenes") else project.name, if (scene) project.name else t("home.title"),
                    { if (scene) cull.sceneView = false else Projects.setActive(null) }, onTitle = if (scene) null else ({ renaming = true }),
                    subtitle = if (phase == Phase.Done && !scene && data.total > 0) t("cull.handled", "done" to data.processed, "total" to data.total) else null,
                    subtitleDone = data.total > 0 && data.processed >= data.total,
                ) {
                    // 撤销：有可撤销的判定 / 打星时出现（以前核心里有撤销栈，界面上没有入口——走查 2026-11-05）。撤销后选中被撤销的那张，墙会滚到它
                    if (phase == Phase.Done && !scene && undoN > 0) IconBtn(Icons.UNDO, t("android.cull.undo"), { Session.undo()?.let { cull.select(it) } }, plain = true)
                    if (phase == Phase.Done && !scene && project.search != false) IconBtn(Icons.SEARCH, t("search.label"), { cull.searchOpen = !cull.searchOpen; if (!cull.searchOpen) cull.query = "" }, on = cull.searchOpen, plain = true)
                    // 开着"学习我的喜好"就随时能点（作者 2026-10-23：删掉项目再新建同一个相册后，菜单一直是灰的，以为坏了）；
                    // 小红点只在这次打开后又有了新标注时出现
                    val canReeval = phase == Phase.Done && settings.learn && project.learn != false
                    com.fleetinggleam.android.ui.components.MenuButton(t("pset.settings"), buildList {
                        add(Triple(t("organize.button"), phase == Phase.Done) { organize = true })
                        add(Triple(t("cull.reeval"), canReeval) { Session.reevaluate() })
                        add(Triple(t("pset.settings"), !scanning) { settingsOpen = true })
                        add(Triple(t("styles.title"), true, onOpenStyles))
                        add(Triple(t("bin.rename"), true) { renaming = true })
                    }, dot = canReeval && dirty > 0)
                }
                if (phase == Phase.Done && cull.searchOpen && !scene) Box(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 8.dp)) { SearchField(cull) }
                if (phase == Phase.Scanning && scanData != null) ScanStatusStrip(cull, scanData)
                if (phase == Phase.Done && !scene) StatusStrip(cull, data)
                // NAS 没连上、照片是上次同步的：顶栏里一行灰字加"重试"（审查第 30 条）
                if (offline && error == null && phase != Phase.Scanning && !scene) Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 12.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(t(if (nasFailure == "auth") "android.nas.authOffline" else "android.cull.offline", "name" to nasOff), style = Type.rowDesc.copy(color = c.ash), modifier = Modifier.weight(1f))
                    if (nasFailure == "auth") FgButton(t("android.nas.toSettings"), { com.fleetinggleam.android.core.Nav.openSettings("nas") }, small = true)
                    else FgButton(t("cull.nasRetry"), { Session.reload() }, small = true)
                }
                AnimatedVisibility(phase == Phase.Scanning && cull.scanCardGone, enter = expandVertically() + fadeIn(), exit = shrinkVertically() + fadeOut()) {
                    ScanPill { scope.launch { cull.scanBackToTop?.invoke() } }
                }
            }
        }
        if (phase == Phase.Done) CullingChrome(project, cull, data, actions)
    }
    OrganizeDialogs(project, organize) { organize = false }

    // 大图盖在整页上面（连工具栏一起）
    if (phase == Phase.Done && cull.viewer && cull.selected != null) Viewer(cull, data, decisions) { path, st, order -> decideIn(cull, data, path, st, order) }
    // 分析中：已经出结果的照片可以点开放大看（只看，改判等分析完）；判出来的照片飞进页签
    if (phase == Phase.Scanning && cull.viewer && cull.selected != null) ScanPeek(cull)
    if (phase == Phase.Scanning) JudgeFlights(cull)

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

/** 开始分析前的确认页：文件、大小、预计时间、预览只在这里收（进项目读取时它们一直在变，别让整页跟着重组）。 */
@Composable
private fun ReadyHost(project: com.fleetinggleam.android.core.Project, paused: Pause, onStart: () -> Unit, onAddFolder: () -> Unit) {
    val files by Session.files.collectAsState()
    val bytes by Session.bytes.collectAsState()
    val uncached by Session.uncached.collectAsState()
    val eta by Session.etaSeconds.collectAsState()
    val previews by Session.previews.collectAsState()
    val todo = remember(files) { files.filter { !it.cached } }
    ReadyStage(project, uncached, bytes, eta, todo, previews, onStart = onStart, onAddFolder = onAddFolder, onRemoveFolder = { Session.removeFolder(it) }, done = files.size - uncached,
        pausedNote = if (paused != Pause.None && Session.pausedId == project.id) t(if (paused == Pause.Hot) "android.paused.hot" else "android.paused.battery") else null)
}

/** 读取中那一行灰字：NAS / 网盘复制、读文件夹、准备实况、认人物的进度只在这里收。 */
@Composable
private fun LoadingLine(phase: Phase) {
    val c = LocalFg.current
    val nasProg by Session.nasProg.collectAsState()
    val peopleProg by Session.peopleProg.collectAsState()
    val mediaProg by Session.mediaProg.collectAsState()
    val probeProg by Session.probeProg.collectAsState()
    val text = when {
        nasProg != null -> nasProg!!.let { p -> if (p.bytesTotal > 0) t("cull.fetchingNas", "share" to p.share, "done" to p.done, "total" to p.total, "size" to fmtBytes(p.bytesTotal)) else t("android.mirrorSync", "share" to p.share, "done" to p.done, "total" to p.total) }
        phase == Phase.Loading && mediaProg != null && mediaProg!!.done < mediaProg!!.total -> t("cull.preparingMedia", "done" to mediaProg!!.done, "total" to mediaProg!!.total)
        phase == Phase.Loading && peopleProg != null && peopleProg!!.done < peopleProg!!.total -> t("cull.recognising", "done" to peopleProg!!.done, "total" to peopleProg!!.total)
        phase == Phase.Probing && probeProg != null && probeProg!!.total > 0 -> t("cull.probing") + "  ${probeProg!!.done} / ${probeProg!!.total}"
        else -> t(if (phase == Phase.Probing) "cull.probing" else "cull.loading")
    }
    Text(text, style = Type.small.copy(color = c.ash), modifier = Modifier.fillMaxWidth().padding(24.dp), textAlign = androidx.compose.ui.text.style.TextAlign.Center)
}
