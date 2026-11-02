package com.fleetinggleam.android.ui.project

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import com.fleetinggleam.android.core.Core
import com.fleetinggleam.android.core.ExportResult
import com.fleetinggleam.android.core.Fg
import com.fleetinggleam.android.core.Media
import com.fleetinggleam.android.core.Organize
import com.fleetinggleam.android.core.Pickers
import com.fleetinggleam.android.core.Progress
import com.fleetinggleam.android.core.Project
import com.fleetinggleam.android.core.Session
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgButton
import com.fleetinggleam.android.ui.components.FgDialog
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.components.Icons
import com.fleetinggleam.android.ui.components.LiveBar
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

/** 导出到哪里：相册里的「惊鸿/项目名」，或系统里选的文件夹（能换成路径就用路径，换不出就按 Uri 逐张写）。 */
private sealed interface Dest {
    data class Path(val path: String, val label: String) : Dest
    data class Tree(val uri: android.net.Uri, val label: String) : Dest
}

private sealed interface Dlg {
    data object Menu : Dlg
    data class Export(val dest: Dest) : Dlg
    data object Trash : Dlg
    data object Marks : Dlg
}

/** 整理（桌面端 Organize.tsx）：从底部玻璃条的"整理"打开——导出保留的、把淘汰的移走。`open` 为真时显示菜单。 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OrganizeDialogs(project: Project, open: Boolean, onDismissMenu: () -> Unit) {
    val c = LocalFg.current
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val photos by Session.photos.collectAsState()
    val decisions by Session.decisions.collectAsState()
    var dlg by remember { mutableStateOf<Dlg?>(null) }
    var phase by remember { mutableStateOf("confirm") } // confirm | running | done
    var progress by remember { mutableStateOf(Progress()) }
    var result by remember { mutableStateOf<Triple<Int, Int, List<String>>?>(null) }
    var exportErr by remember { mutableStateOf<String?>(null) }
    var exportRes by remember { mutableStateOf<ExportResult?>(null) }
    var stopping by remember { mutableStateOf(false) }
    val nasShares = com.fleetinggleam.android.core.Nas.shares.collectAsState().value.filter { it.enabled }
    // 项目所在的 NAS 没连上：导出到 NAS、移走淘汰的（进 NAS 的回收文件夹）做不了，先置灰（审查第 30 条）
    val offline by Session.offline.collectAsState()
    var scopeAll by remember { mutableStateOf(false) }
    val keep = photos.filter { decisions[it.path]?.state == "keep" }
    val rejectAll = photos.filter { decisions[it.path]?.state == "reject" }
    val rejectManual = rejectAll.filter { decisions[it.path]?.manual == true }
    val trashList = if (scopeAll) rejectAll else rejectManual
    // 保留的照片里有带同名 RAW 的：导出时可以把 RAW 一起带上
    val rawCount = keep.count { it.companions.isNotEmpty() }
    var withRaw by remember { mutableStateOf(true) }
    // 写回 Lightroom / Bridge：有判定的照片都写（NAS 与网盘文件夹里的只是副本，跳过）；先数一数会写几个旁车、几个 JPEG、几张写不了
    val stars = project.stars
    val decidedAll = photos.filter { decisions.containsKey(it.path) }
    val decided = decidedAll.filter { !Organize.isCopy(ctx, it.path) }
    val copies = decidedAll.size - decided.size
    val markCounts = decided.flatMap { listOf(it.path) + it.companions }.groupingBy { Organize.carrierOf(it) }.eachCount()
    var markStars by remember { mutableStateOf(true) }
    var markLabels by remember { mutableStateOf(true) }
    var markRes by remember { mutableStateOf<com.fleetinggleam.android.core.MarkResult?>(null) }
    var markErr by remember { mutableStateOf<String?>(null) }
    var markUndone by remember { mutableStateOf<Pair<Int, Int>?>(null) }
    var undoing by remember { mutableStateOf(false) }

    fun open(d: Dlg) { result = null; exportErr = null; exportRes = null; markRes = null; markErr = null; markUndone = null; undoing = false; stopping = false; phase = "confirm"; progress = Progress(); dlg = d; if (d != Dlg.Menu) onDismissMenu() }
    fun close() { val reload = phase == "done"; dlg = null; onDismissMenu(); if (reload) Session.reload() }

    androidx.compose.runtime.LaunchedEffect(open) { if (open) open(Dlg.Menu) }

    // ---- 两件事
    if (dlg == Dlg.Menu) {
        com.fleetinggleam.android.ui.components.GlassSheet(true, { dlg = null; onDismissMenu() }) {
            Column(Modifier.navigationBarsPadding().padding(horizontal = 16.dp).padding(bottom = 16.dp)) {
                // 弹层标题一律左对齐（设计审查 A12.1）
                Text(t("organize.button"), style = Type.navTitle.copy(color = c.ink), modifier = Modifier.fillMaxWidth().padding(bottom = 12.dp))
                Column(Modifier.clip(RoundedCornerShape(14.dp)).background(c.panel)) {
                OrgItem(Icons.ALBUM, t("android.organize.toAlbum"), if (keep.isEmpty()) t("organize.none") else t("android.organize.toAlbumDesc", "count" to keep.size, "where" to "${t("brand.name")}/${project.name}"), keep.isNotEmpty(), first = true) {
                    open(Dlg.Export(Dest.Path(Organize.albumDir(project.name).absolutePath, "${t("brand.name")}/${project.name}")))
                }
                OrgItem(Icons.FOLDER, t("android.organize.toFolder"), if (keep.isEmpty()) t("organize.none") else t("organize.exportDesc", "count" to keep.size), keep.isNotEmpty()) {
                    dlg = null; onDismissMenu()
                    scope.launch {
                        val uri = Pickers.openTree() ?: return@launch
                        val path = Media.treeToPath(uri)
                        open(Dlg.Export(if (path != null && File(path).canWrite()) Dest.Path(path, path) else Dest.Tree(uri, androidx.documentfile.provider.DocumentFile.fromTreeUri(ctx, uri)?.name ?: uri.toString())))
                    }
                }
                // 导出到 NAS：选择器只列 NAS；选中的是那个文件夹的镜像路径，核心库认出来就直接传到 NAS 上
                if (nasShares.isNotEmpty()) OrgItem(Icons.NAS, t("organize.exportNasItem"), if (offline) t("android.organize.offline") else if (keep.isEmpty()) t("organize.none") else t("organize.exportNasDesc", "count" to keep.size), keep.isNotEmpty() && !offline) {
                    dlg = null; onDismissMenu()
                    scope.launch {
                        val path = com.fleetinggleam.android.core.Nas.pick(nasShares.singleOrNull()?.id, source = "nas", title = t("organize.exportNasPick"), choose = t("organize.exportHere")) ?: return@launch
                        open(Dlg.Export(Dest.Path(path, com.fleetinggleam.android.core.Nas.label(path))))
                    }
                }
                OrgItem(Icons.MARKS, t("organize.marksItem"), if (decidedAll.isEmpty()) t("organize.none") else t("organize.marksDesc", "count" to decided.size), decided.isNotEmpty()) {
                    open(Dlg.Marks)
                }
                OrgItem(Icons.TRASH, t("organize.trashItem"), if (offline) t("android.organize.offline") else if (rejectAll.isEmpty()) t("organize.none") else t("organize.trashDesc", "count" to rejectAll.size), rejectAll.isNotEmpty() && !offline, box = 14f) {
                    scopeAll = rejectManual.isEmpty(); open(Dlg.Trash)
                }
                }
            }
        }
    }

    // ---- 导出
    (dlg as? Dlg.Export)?.let { d ->
        DisposableEffect(Unit) {
            val off = Core.listen("export-progress") { raw -> runCatching { Fg.json.decodeFromString<Progress>(raw) }.getOrNull()?.let { progress = it } }
            onDispose { off() }
        }
        val label = when (val x = d.dest) { is Dest.Path -> x.label; is Dest.Tree -> x.label }
        FgDialog(true, { if (phase != "running") close() }, t("organize.exportTitle"), locked = phase == "running", footer = {
            if (phase == "done") FgButton(t("organize.done"), { close() }, primary = true, small = true)
            else if (phase == "running") {
                // 导出中可以停：核心那边在当前这张写完后停下，网盘那边同样（审查第 35 条）
                FgButton(if (stopping) t("android.organize.stopping") else t("android.organize.stop"), {
                    stopping = true
                    when (d.dest) {
                        is Dest.Path -> scope.launch { runCatching { Core.call("export_cancel") } }
                        is Dest.Tree -> Organize.treeCancel = true
                    }
                }, small = true, enabled = !stopping)
                FgButton(t("organize.exporting", "done" to progress.done, "total" to progress.total), {}, primary = true, small = true, enabled = false)
            } else {
                FgButton(t("organize.cancel"), { close() }, small = true)
                FgButton(t("organize.exportGo"), {
                    val raw = rawCount > 0 && withRaw
                    phase = "running"; progress = Progress(0, keep.size); stopping = false
                    scope.launch {
                        val paths = keep.map { it.path }
                        val r = runCatching {
                            when (val x = d.dest) {
                                is Dest.Path -> Organize.exportTo(ctx, paths, x.path, withRaw = raw)
                                // 网盘等只能按 Uri 逐张写的：同名 RAW 在这边展开
                                is Dest.Tree -> withContext(Dispatchers.IO) { Organize.exportToTree(ctx, if (raw) keep.flatMap { listOf(it.path) + it.companions } else paths, x.uri) { a, b -> progress = Progress(a, b) } }
                            }
                        }.getOrElse { exportErr = it.message; ExportResult(0, 0, label) }
                        exportRes = r; phase = "done"
                    }
                }, primary = true, small = true)
            }
        }) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (phase == "done") {
                    val r = exportRes
                    // 没开始就失败了（空间不够之类）：按错误前缀说清楚
                    if (exportErr != null) Text(com.fleetinggleam.android.core.Errors.explain(exportErr) ?: t("organize.exportFailed", "error" to exportErr), style = Type.body.copy(color = c.ink))
                    else if (r != null) {
                        Text(if (r.cancelled) t("android.organize.exportCancelled", "count" to r.copied) else t("organize.exported", "count" to r.copied), style = Type.body.copy(color = c.ink))
                        if (r.skipped > 0) Text(t("android.organize.exportedSame", "n" to r.skipped), style = Type.rowDesc.copy(color = c.ash))
                        // 没导出的：同一个原因的归在一起，各举一张
                        if (r.failed.isNotEmpty()) {
                            Text(t("android.organize.exportFailedN", "n" to r.failed.size), style = Type.rowDesc.copy(color = c.ink))
                            r.failed.groupBy { com.fleetinggleam.android.core.Errors.explain(it.error) ?: com.fleetinggleam.android.core.Errors.detail(it.error).ifEmpty { "?" } }.forEach { (why, list) ->
                                Text("· $why（${list.size}）${File(list[0].path).name}" + if (list.size > 1) " …" else "", style = Type.rowDesc.copy(color = c.ash), maxLines = 2, overflow = Ellipsis)
                            }
                        }
                    }
                    Text(label, style = Type.monoSmall.copy(color = c.ash))
                } else {
                    Text(t("organize.exportBody", "count" to keep.size), style = Type.body.copy(color = c.ink))
                    // 路径只是一行等宽灰字；"原图不会被改动"在整理菜单里已经说过一次（A12.2）
                    Text(label, style = Type.mono.copy(color = c.ash))
                    if (rawCount > 0) SwitchLine(t("organize.withRaw", "count" to rawCount), withRaw, enabled = phase != "running") { withRaw = it }
                    if (phase == "running") LiveBar(if (progress.total > 0) progress.done.toFloat() / progress.total else 0f, Modifier.padding(top = 4.dp))
                }
            }
        }
    }

    // ---- 写回 Lightroom / Bridge
    if (dlg == Dlg.Marks) {
        val busy = phase == "running" || undoing
        FgDialog(true, { if (!busy) close() }, t("organize.marksTitle"), locked = busy, footer = {
            if (phase == "done") {
                val r = markRes
                if (r != null && r.undo.isNotEmpty() && markUndone == null) FgButton(if (undoing) t("organize.marksUndoing") else t("organize.marksUndo"), {
                    undoing = true
                    scope.launch {
                        markUndone = runCatching { Organize.undoMarks(ctx, r.undo) }.map { it.restored.size to it.failed.size }.getOrElse { 0 to r.undo.size }
                        undoing = false
                    }
                }, small = true, enabled = !undoing)
                FgButton(t("organize.done"), { close() }, primary = true, small = true, enabled = !undoing)
            } else {
                FgButton(t("organize.cancel"), { close() }, enabled = phase != "running", small = true)
                FgButton(if (phase == "running") t("organize.marksWriting") else t("organize.marksGo"), {
                    phase = "running"
                    scope.launch {
                        val items = decided.map { Organize.MarkItem(it.path, decisions[it.path]!!.state, it.score, stars[it.path]) }
                        runCatching { Organize.writeMarks(ctx, items, markStars, markLabels) }.onSuccess { markRes = it }.onFailure { markErr = it.message ?: "?" }
                        phase = "done"
                    }
                }, primary = true, small = true,
                    enabled = phase != "running" && decided.isNotEmpty() && (markStars || markLabels) && (markCounts["jpeg"] ?: 0) + (markCounts["sidecar"] ?: 0) > 0)
            }
        }) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                val undone = markUndone
                val r = markRes
                if (phase == "done" && undone != null) {
                    Text(t("organize.marksUndone", "count" to undone.first), style = Type.body.copy(color = c.ink))
                    if (undone.second > 0) Text(t("organize.marksUndoFailed", "count" to undone.second, "n" to undone.second), style = Type.rowDesc.copy(color = c.ash))
                } else if (phase == "done" && markErr != null) {
                    Text(t("organize.marksFailed", "error" to (com.fleetinggleam.android.core.Errors.explain(markErr) ?: markErr)), style = Type.body.copy(color = c.ink))
                } else if (phase == "done" && r != null) {
                    Text(t("organize.marksWritten", "sidecars" to r.sidecars, "embedded" to r.embedded), style = Type.body.copy(color = c.ink))
                    if (r.skipped.isNotEmpty()) Text(t("organize.marksSkipped", "count" to r.skipped.size, "n" to r.skipped.size), style = Type.rowDesc.copy(color = c.ash))
                    // 没写成的：同一个原因的归在一起，各举一张（相册里别的应用的照片，系统不让改，会在这里）
                    if (r.failed.isNotEmpty()) {
                        Text(t("android.organize.marksFailedN", "n" to r.failed.size), style = Type.rowDesc.copy(color = c.ink))
                        r.failed.groupBy { com.fleetinggleam.android.core.Errors.explain(it.error) ?: com.fleetinggleam.android.core.Errors.detail(it.error).ifEmpty { it.error.ifEmpty { "?" } } }.forEach { (why, list) ->
                            Text("· $why（${list.size}）${File(list[0].path).name}" + if (list.size > 1) " …" else "", style = Type.rowDesc.copy(color = c.ash), maxLines = 2, overflow = Ellipsis)
                        }
                    }
                    // 相册里别的应用（相机等）存的照片，系统只许它自己改：说清楚原因和办法
                    if (r.failed.any { "os error 1" in it.error || "os error 13" in it.error || it.error.startsWith("denied") }) Text(t("android.organize.marksDenied"), style = Type.rowDesc.copy(color = c.ash))
                    if (r.sidecars + r.embedded > 0) Text(t("organize.marksNext"), style = Type.rowDesc.copy(color = c.ash))
                } else {
                    Text(t("organize.marksBody", "count" to decided.size), style = Type.body.copy(color = c.ink))
                    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        markCounts["sidecar"]?.let { Text("· " + t("organize.marksSidecars", "count" to it, "n" to it), style = Type.rowDesc.copy(color = c.ink)) }
                        markCounts["jpeg"]?.let { Text("· " + t("organize.marksEmbedded", "count" to it, "n" to it), style = Type.rowDesc.copy(color = c.ink)) }
                        markCounts["none"]?.let { Text("· " + t("organize.marksUnsupported", "count" to it, "n" to it), style = Type.rowDesc.copy(color = c.ash)) }
                        if (copies > 0) Text("· " + t("android.organize.marksCopies", "n" to copies), style = Type.rowDesc.copy(color = c.ash))
                    }
                    SwitchLine(t("organize.marksStars"), markStars, enabled = phase != "running") { markStars = it }
                    SwitchLine(t("organize.marksLabels"), markLabels, enabled = phase != "running") { markLabels = it }
                    Text(t("organize.marksSafe"), style = Type.rowDesc.copy(color = c.ash))
                }
            }
        }
    }

    // ---- 移走淘汰的
    if (dlg == Dlg.Trash) {
        FgDialog(true, { if (phase != "running") close() }, t("organize.trashTitle"), locked = phase == "running", footer = {
            if (phase == "done") FgButton(t("organize.done"), { close() }, primary = true, small = true)
            else {
                FgButton(t("organize.cancel"), { close() }, enabled = phase != "running", small = true)
                FgButton(if (phase == "running") t("organize.trashing") else t("organize.trashGo", "count" to trashList.size), {
                    phase = "running"
                    scope.launch {
                        val (out, gone) = runCatching { withContext(Dispatchers.IO) { Organize.trash(ctx, trashList.map { it.path }) { s -> withContext(Dispatchers.Main) { Pickers.confirm(s) } } } }
                            .getOrElse { Organize.TrashOutcome(0, trashList.size, emptyList()) to emptyList() }
                        Organize.forget(project.id, gone)
                        if (out.cancelled && out.moved == 0) { phase = "confirm"; return@launch }
                        result = Triple(out.moved, out.failed, out.bins); phase = "done"
                    }
                }, primary = true, small = true, enabled = phase != "running" && trashList.isNotEmpty())
            }
        }) {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                if (phase == "done") {
                    val r = result
                    Text(t("organize.trashed", "count" to (r?.first ?: 0)), style = Type.body.copy(color = c.ink))
                    if ((r?.second ?: 0) > 0) Text(t("organize.trashFailed", "n" to r!!.second), style = Type.rowDesc.copy(color = c.ash))
                    if (!r?.third.isNullOrEmpty()) Text(t("organize.trashedToShare", "where" to r!!.third.joinToString(" · ")), style = Type.rowDesc.copy(color = c.ash))
                    else Text(t("organize.trashRecover"), style = Type.rowDesc.copy(color = c.ash))
                } else {
                    Text(t("organize.trashBody"), style = Type.body.copy(color = c.ink))
                    Radio(t("organize.scopeManual", "count" to rejectManual.size), !scopeAll, enabled = rejectManual.isNotEmpty()) { scopeAll = false }
                    Radio(t("organize.scopeAll", "count" to rejectAll.size), scopeAll) { scopeAll = true }
                    if (scopeAll && rejectAll.size > rejectManual.size) Text(t("organize.warnAll", "n" to rejectAll.size - rejectManual.size), style = Type.rowDesc.copy(color = c.ink, fontWeight = FontWeight.Medium))
                    Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically) {
                        trashList.take(8).forEach { p -> AsyncImage(File(p.thumb), null, Modifier.size(44.dp).clip(RoundedCornerShape(6.dp)), contentScale = ContentScale.Crop) }
                        if (trashList.size > 8) Text("+${trashList.size - 8}", style = Type.mono.copy(color = c.ash), modifier = Modifier.padding(start = 4.dp))
                    }
                }
            }
        }
    }
}

@Composable
private fun OrgItem(icon: String, title: String, desc: String, enabled: Boolean, box: Float = 16f, first: Boolean = false, onClick: () -> Unit) {
    val c = LocalFg.current
    Column {
        if (!first) Box(Modifier.padding(start = 56.dp).fillMaxWidth().height(0.5.dp).background(c.line))
        Row(
            Modifier.fillMaxWidth().graphicsLayer { alpha = if (enabled) 1f else 0.4f }.clickable(enabled = enabled, onClick = onClick).padding(horizontal = 16.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            com.fleetinggleam.android.ui.components.IconBox(icon, box = box)
            Column(Modifier.weight(1f).padding(horizontal = 12.dp)) {
                Text(title, style = Type.rowTitle.copy(color = c.ink), maxLines = 1, overflow = Ellipsis)
                Text(desc, style = Type.rowDesc.copy(color = c.ash), maxLines = 2, overflow = Ellipsis)
            }
            FgIcon(Icons.CHEVRON, size = 14.dp, stroke = 1.6f, color = c.ash)
        }
    }
}

/** 对话框里的一行开关：左边一句话，右边开关；整行可点。 */
@Composable
private fun SwitchLine(text: String, on: Boolean, enabled: Boolean = true, onChange: (Boolean) -> Unit) {
    val c = LocalFg.current
    Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).clickable(enabled = enabled) { onChange(!on) }.padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(text, style = Type.rowDesc.copy(color = c.ink), modifier = Modifier.weight(1f).padding(end = 12.dp))
        com.fleetinggleam.android.ui.components.FgSwitch(on, onChange, text, enabled = enabled)
    }
}

@Composable
private fun Radio(text: String, on: Boolean, enabled: Boolean = true, onClick: () -> Unit) {
    val c = LocalFg.current
    Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).graphicsLayer { alpha = if (enabled) 1f else 0.45f }.clickable(enabled = enabled, onClick = onClick).padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        com.fleetinggleam.android.ui.components.FgRadio(on)
        Text(text, style = Type.rowTitle.copy(color = c.ink), modifier = Modifier.padding(start = 10.dp))
    }
}
