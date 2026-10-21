package com.fleetinggleam.android.ui.home

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import com.fleetinggleam.android.core.Projects
import com.fleetinggleam.android.core.Project
import com.fleetinggleam.android.core.Styles
import com.fleetinggleam.android.core.baseName
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.Actions
import com.fleetinggleam.android.ui.components.FgButton
import com.fleetinggleam.android.ui.components.FgDialog
import com.fleetinggleam.android.ui.components.FgField
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.components.Icons
import com.fleetinggleam.android.ui.components.PagePadding
import com.fleetinggleam.android.ui.components.Segmented
import com.fleetinggleam.android.ui.components.Toasts
import com.fleetinggleam.android.ui.components.pressScale
import com.fleetinggleam.android.ui.components.rememberSource
import com.fleetinggleam.android.ui.components.riseIn
import com.fleetinggleam.android.ui.components.StylePicker
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.io.File

/** The module's landing page: every project as a card, plus a recycle bin for the ones you delete (the Mac's ProjectHome.tsx). */
@Composable
fun ProjectHome(onOpenStyles: () -> Unit) {
    val c = LocalFg.current
    val store by Projects.store.collectAsState()
    val status by Projects.status.collectAsState()
    val scope = rememberCoroutineScope()
    val now = System.currentTimeMillis()
    val live = store.projects.filter { it.trashed == null }
    val bin = store.projects.filter { it.trashed != null && now - it.trashed < Projects.TRASH_DAYS * 86_400_000L }
    var view by remember { mutableStateOf("projects") }
    var armed by remember { mutableStateOf<String?>(null) }
    var ask by remember { mutableStateOf<Project?>(null) }
    var renaming by remember { mutableStateOf<Project?>(null) }
    var detailed by remember { mutableStateOf<Triple<String, String, String>?>(null) }
    LaunchedEffect(armed) { if (armed != null) { delay(3000); armed = null } }
    LaunchedEffect(view, bin.size) { if (view == "bin" && bin.isEmpty()) view = "projects" }

    if (store.projects.isEmpty()) {
        EmptyState()
        return
    }
    val list = if (view == "projects") live else bin
    LazyVerticalGrid(
        columns = GridCells.Adaptive(160.dp),
        modifier = Modifier.fillMaxSize().statusBarsPadding(),
        contentPadding = PagePadding,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item(span = { GridItemSpan(maxLineSpan) }) {
            Column(Modifier.riseIn(0, scaleFrom = 1f).padding(top = 12.dp, bottom = 8.dp)) {
                Text(t(if (view == "projects") "home.title" else "bin.tab"), style = Type.title.copy(color = c.ink))
                Text(if (view == "projects") t("home.subtitle") else t("bin.hint", "days" to Projects.TRASH_DAYS), style = Type.small.copy(color = c.ash), modifier = Modifier.padding(top = 4.dp))
                Row(Modifier.padding(top = 16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Segmented(
                        listOf("projects" to t("home.title"), "bin" to t("bin.tab") + if (bin.isNotEmpty()) " · ${bin.size}" else ""),
                        view, { if (it != "bin" || bin.isNotEmpty()) view = it },
                    )
                    Spacer(Modifier.weight(1f))
                    if (view == "bin") FgButton(if (armed == "all") t("bin.emptyConfirm") else t("bin.emptyAll"), { if (armed == "all") { Projects.emptyTrash(); armed = null } else armed = "all" }, small = true, on = armed == "all")
                    else FgButton(t("nav.styles"), onOpenStyles, small = true)
                }
                if (status.saveError != null) Text(t("cull.saveFailed", "error" to status.saveError), style = Type.small.copy(color = c.ink), modifier = Modifier.padding(top = 12.dp))
                if (status.loadError == "corrupt") Text(t("cull.loadCorrupt"), style = Type.small.copy(color = c.ink), modifier = Modifier.padding(top = 12.dp))
            }
        }
        items(list, key = { it.id }) { p ->
            val i = list.indexOf(p)
            ProjectCard(
                p, inBin = view == "bin", index = i, armed = armed == p.id,
                onOpen = { Projects.setActive(p.id) },
                onTrash = { ask = p },
                onRename = { renaming = p },
                onRestore = { Projects.restore(p.id) },
                onPurge = { if (armed == p.id) { Projects.purge(p.id); armed = null } else armed = p.id },
                modifier = Modifier.animateItem(),
            )
        }
        if (view == "projects") item(key = "new") {
            NewCard(
                index = list.size,
                onQuick = { scope.launch { Actions.newProjectFromDialog() } },
                onDetailed = { scope.launch { Actions.pickFolder()?.let { detailed = Triple(it, baseName(it), "") } } },
                modifier = Modifier.animateItem(),
            )
        }
        if (view == "bin" && bin.isEmpty()) item(span = { GridItemSpan(maxLineSpan) }) { Text(t("bin.empty"), style = Type.body.copy(color = c.ash)) }
    }

    FgDialog(ask != null, { ask = null }, t("bin.confirmTitle"), footer = {
        FgButton(t("organize.cancel"), { ask = null })
        FgButton(t("bin.toBin"), {
            val p = ask ?: return@FgButton
            Projects.trash(p.id)
            Toasts.show("${t("bin.moved")} · ${p.name}", t("bin.undo")) { Projects.restore(p.id) }
            ask = null
        }, on = true)
    }) { Text(t("bin.confirmBody", "name" to (ask?.name ?: ""), "days" to Projects.TRASH_DAYS), style = Type.body.copy(color = c.ink)) }

    renaming?.let { r ->
        var name by remember(r.id) { mutableStateOf(r.name) }
        FgDialog(true, { renaming = null }, t("bin.rename"), footer = {
            FgButton(t("organize.cancel"), { renaming = null })
            FgButton(t("organize.done"), { name.trim().takeIf { it.isNotEmpty() }?.let { v -> Projects.update(r.id) { it.copy(name = v) } }; renaming = null }, primary = true)
        }) { FgField(name, { name = it }, label = t("pset.name"), onDone = { name.trim().takeIf { it.isNotEmpty() }?.let { v -> Projects.update(r.id) { it.copy(name = v) } }; renaming = null }) }
    }

    detailed?.let { (dir, name0, style0) ->
        val styles by Styles.list.collectAsState()
        var name by remember(dir) { mutableStateOf(name0) }
        var style by remember(dir) { mutableStateOf(style0) }
        FgDialog(true, { detailed = null }, t("home.new"), footer = {
            FgButton(t("organize.cancel"), { detailed = null })
            FgButton(t("home.new"), {
                val p = Projects.create(name.trim().ifEmpty { baseName(dir) }, listOf(dir))
                if (style.isNotEmpty()) Projects.update(p.id) { it.copy(style = style) }
                detailed = null
            }, primary = true)
        }) {
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                FgField(name, { name = it }, label = t("pset.name"), placeholder = t("home.namePlaceholder"))
                Column {
                    Text(t("pset.folders"), style = Type.small.copy(color = c.ash))
                    FolderLine(dir, Modifier.padding(top = 6.dp))
                    FgButton(t("styles.chooseFolder"), { scope.launch { Actions.pickFolder()?.let { detailed = Triple(it, baseName(it), style) } } }, small = true, modifier = Modifier.padding(top = 8.dp))
                }
                if (styles.size > 1) StylePicker(style, styles) { style = it }
            }
        }
    }
}

/** 一个文件夹：名字粗体，完整路径等宽小字。 */
@Composable
fun FolderLine(path: String, modifier: Modifier = Modifier) {
    val c = LocalFg.current
    Column(modifier) {
        Text(baseName(path), style = Type.bodyStrong.copy(color = c.ink), maxLines = 1, overflow = Ellipsis)
        Text(path, style = Type.monoSmall.copy(color = c.ash), maxLines = 2, overflow = Ellipsis)
    }
}

@Composable
private fun ProjectCard(p: Project, inBin: Boolean, index: Int, armed: Boolean, onOpen: () -> Unit, onTrash: () -> Unit, onRename: () -> Unit, onRestore: () -> Unit, onPurge: () -> Unit, modifier: Modifier) {
    val c = LocalFg.current
    val src = rememberSource()
    val shape = RoundedCornerShape(16.dp)
    Column(
        modifier
            .riseIn(index + 1)
            .pressScale(src)
            .clip(shape)
            .background(c.panel)
            .border(1.dp, c.line, shape)
            .combinedClickable(interactionSource = src, indication = null, enabled = !inBin, onClick = onOpen, onLongClick = onRename)
            .padding(start = 8.dp, end = 8.dp, top = 8.dp, bottom = 12.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Box {
            Cover(p.cover.orEmpty().take(4))
            if (!inBin) {
                Box(
                    Modifier.align(Alignment.TopEnd).padding(6.dp).size(30.dp).clip(CircleShape).background(Color(0x8C101014)).clickable(onClick = onTrash),
                    contentAlignment = Alignment.Center,
                ) { FgIcon(Icons.TRASH, size = 15.dp, box = 14f, color = Color.White) }
            }
        }
        Column(Modifier.padding(horizontal = 6.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(p.name, style = Type.bodyStrong.copy(color = c.ink, fontWeight = FontWeight.SemiBold), maxLines = 1, overflow = Ellipsis)
            Text(
                if (inBin) t("bin.daysLeft", "n" to Projects.daysLeft(p), "count" to Projects.daysLeft(p))
                else "${t("home.photos", "count" to p.count)} · ${t("home.folders", "count" to p.folders.size)}",
                style = Type.monoSmall.copy(color = c.ash), maxLines = 1,
            )
        }
        if (inBin) Row(Modifier.padding(horizontal = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FgButton(t("bin.restore"), onRestore, small = true)
            FgButton(if (armed) t("bin.purgeConfirm") else t("bin.purge"), onPurge, small = true, on = armed)
        }
    }
}

/** 封面：最多四张，1 张整幅，2 张左右，3 张时第一张占左边两格，4 张田字（桌面端 .pcover）。 */
@Composable
private fun Cover(cover: List<String>) {
    val c = LocalFg.current
    val shape = RoundedCornerShape(10.dp)
    Box(Modifier.fillMaxWidth().aspectRatio(3f / 2f).clip(shape).background(c.surface), contentAlignment = Alignment.Center) {
        if (cover.isEmpty()) { Text(t("home.empty"), style = Type.small.copy(color = c.ash)); return@Box }
        val img: @Composable (String, Modifier) -> Unit = { path, m -> AsyncImage(File(path), null, m, contentScale = ContentScale.Crop) }
        val gap = 3.dp
        when (cover.size) {
            1 -> img(cover[0], Modifier.fillMaxSize())
            2 -> Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.spacedBy(gap)) { cover.forEach { img(it, Modifier.weight(1f).fillMaxSize()) } }
            3 -> Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.spacedBy(gap)) {
                img(cover[0], Modifier.weight(1f).fillMaxSize())
                Column(Modifier.weight(1f).fillMaxSize(), verticalArrangement = Arrangement.spacedBy(gap)) { img(cover[1], Modifier.weight(1f).fillMaxWidth()); img(cover[2], Modifier.weight(1f).fillMaxWidth()) }
            }
            else -> Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.spacedBy(gap)) {
                for (r in 0..1) Row(Modifier.weight(1f).fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(gap)) { for (k in 0..1) img(cover[r * 2 + k], Modifier.weight(1f).fillMaxSize()) }
            }
        }
    }
}

/** 新建项目：虚线框，一个大加号"新建项目"，下面一个"手动创建 →"（先选文件夹，再定名字和风格）。 */
@Composable
private fun NewCard(index: Int, onQuick: () -> Unit, onDetailed: () -> Unit, modifier: Modifier) {
    val c = LocalFg.current
    val src = rememberSource()
    Column(
        modifier.riseIn(index + 1).pressScale(src).fillMaxWidth().aspectRatio(3f / 2.55f)
            .drawBehind {
                drawRoundRect(c.line.copy(alpha = 0.6f), cornerRadius = CornerRadius(16.dp.toPx()), style = Stroke(1.5.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(10f, 8f))))
            },
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Column(Modifier.clip(RoundedCornerShape(12.dp)).clickable(interactionSource = src, indication = null, onClick = onQuick).padding(12.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Box(Modifier.size(44.dp).clip(CircleShape).background(c.ink), contentAlignment = Alignment.Center) { FgIcon(Icons.PLUS, size = 20.dp, box = 18f, stroke = 1.6f, color = c.onInk) }
            Text(t("home.new"), style = Type.bodyStrong.copy(color = c.ink), modifier = Modifier.padding(top = 10.dp))
        }
        Text(t("home.newDetailed"), style = Type.small.copy(color = c.ash), modifier = Modifier.clip(RoundedCornerShape(8.dp)).clickable(onClick = onDetailed).padding(horizontal = 10.dp, vertical = 6.dp))
    }
}

/** 第一次打开：品牌名、一句话、三步做什么、新建项目（桌面端没有项目时的 .empty）。 */
@Composable
private fun EmptyState() {
    val c = LocalFg.current
    val scope = rememberCoroutineScope()
    Column(Modifier.fillMaxSize().statusBarsPadding().padding(horizontal = 28.dp).padding(top = 72.dp)) {
        Text(t("brand.name"), style = Type.display.copy(color = c.ink), modifier = Modifier.riseIn(0, scaleFrom = 1f))
        if (!com.fleetinggleam.android.core.I18n.lang.value.startsWith("en")) Text(t("brand.latin"), style = Type.monoSmall.copy(color = c.ash), modifier = Modifier.padding(top = 2.dp))
        Text(t("brand.tagline"), style = Type.heading.copy(color = c.ash, fontWeight = FontWeight.Normal), modifier = Modifier.padding(top = 10.dp).riseIn(1, scaleFrom = 1f))
        Text(t("cull.emptyBody", "brand" to t("brand.name")), style = Type.body.copy(color = c.ash), modifier = Modifier.padding(top = 20.dp).riseIn(2, scaleFrom = 1f))
        Column(Modifier.padding(top = 20.dp)) {
            com.fleetinggleam.android.core.I18n.list("cull.steps").forEachIndexed { i, s ->
                Column(Modifier.riseIn(3 + i, scaleFrom = 1f)) {
                    Box(Modifier.fillMaxWidth().height(1.dp).background(c.line))
                    Row(Modifier.padding(vertical = 12.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                        Text("0${i + 1}", style = Type.mono.copy(color = c.ash))
                        Text(s, style = Type.body.copy(color = c.ink))
                    }
                }
            }
        }
        Spacer(Modifier.height(24.dp))
        FgButton(t("cull.newProjectBtn"), { scope.launch { Actions.newProjectFromDialog() } }, primary = true, modifier = Modifier.riseIn(7, scaleFrom = 1f))
        Text(t("nav.privacy"), style = Type.small.copy(color = c.ash), modifier = Modifier.padding(top = 28.dp).width(320.dp))
    }
}
