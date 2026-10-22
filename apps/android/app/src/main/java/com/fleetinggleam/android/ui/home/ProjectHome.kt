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
import com.fleetinggleam.android.ui.components.anchor
import androidx.compose.foundation.layout.offset
import androidx.compose.ui.layout.layout
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
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
    androidx.activity.compose.BackHandler(view == "bin") { view = "projects" }

    if (store.projects.isEmpty()) {
        EmptyState()
        return
    }
    val list = if (view == "projects") live else bin
    // 全局搜索（桌面端一样：搜索模型在时才出现）：所有项目的照片，按项目分组，点一张打开那个项目并选中它
    val search = remember { com.fleetinggleam.android.ui.project.CullState() }
    val index by com.fleetinggleam.android.core.Search.index.collectAsState()
    val allFolders = remember(live) { live.filter { it.search != false }.flatMap { it.folders } }
    LaunchedEffect(allFolders, search.searchRev) { com.fleetinggleam.android.core.Search.keep(allFolders, search.searchRev) }
    LaunchedEffect(search.query, allFolders) {
        val q = search.query.trim()
        if (q.isEmpty() || allFolders.isEmpty()) { search.rawHits = null; return@LaunchedEffect }
        delay(250)
        search.rawHits = runCatching { com.fleetinggleam.android.core.Search.photos(allFolders, q) }.getOrDefault(emptyList())
    }
    val searching = view == "projects" && search.query.trim().isNotEmpty()
    val groups = remember(search.rawHits, live) {
        val by = LinkedHashMap<String, MutableList<com.fleetinggleam.android.core.Hit>>()
        for (h in search.rawHits ?: emptyList()) {
            val p = live.firstOrNull { pr -> pr.folders.any { f -> h.path.startsWith(f.trimEnd('/') + "/") } } ?: continue
            by.getOrPut(p.id) { mutableListOf() }.add(h)
        }
        live.filter { by.containsKey(it.id) }.map { it to by[it.id]!! }
    }
    LazyVerticalGrid(
        columns = GridCells.Adaptive(160.dp),
        modifier = Modifier.fillMaxSize().statusBarsPadding(),
        contentPadding = PagePadding,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalArrangement = Arrangement.spacedBy(18.dp),
    ) {
        item(span = { GridItemSpan(maxLineSpan) }) {
            Column(Modifier.padding(bottom = 4.dp)) {
                if (view == "projects") {
                    // 根页：大标题"项目"，右上 ⋯（回收站、风格、手动新建）与 ＋（新建）
                    Row(Modifier.fillMaxWidth().padding(top = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(t("home.title"), style = Type.largeTitle.copy(color = c.ink), modifier = Modifier.weight(1f))
                        com.fleetinggleam.android.ui.components.MenuButton(t("home.title"), listOf(
                            Triple(t("home.newDetailed").trimEnd('→', ' '), true) { scope.launch { Actions.pickFolder()?.let { detailed = Triple(it, baseName(it), "") } } },
                            Triple(t("nav.styles"), true, onOpenStyles),
                            Triple(t("bin.tab") + if (bin.isNotEmpty()) " · ${bin.size}" else "", bin.isNotEmpty()) { view = "bin" },
                        ))
                        Box(
                            Modifier.padding(start = 2.dp).size(36.dp).clip(CircleShape).background(c.ink).clickable { scope.launch { Actions.newProjectFromDialog() } }
                                .semantics { contentDescription = t("home.new") },
                            contentAlignment = Alignment.Center,
                        ) { FgIcon(Icons.PLUS, size = 18.dp, box = 18f, stroke = 1.7f, color = c.onInk) }
                    }
                    if (index.available && allFolders.isNotEmpty())
                        com.fleetinggleam.android.ui.project.SearchField(search, Modifier.padding(top = 12.dp), placeholder = t("search.globalPlaceholder"), autoFocus = false)
                } else {
                    // 回收站：推进页
                    com.fleetinggleam.android.ui.components.NavBar(t("bin.tab"), t("home.title"), { view = "projects" }, Modifier.layout { m, cs -> val pl = m.measure(cs.copy(maxWidth = cs.maxWidth + 32.dp.roundToPx(), minWidth = 0)); layout(cs.maxWidth, pl.height) { pl.placeRelative(-16.dp.roundToPx(), 0) } }) {
                        if (bin.isNotEmpty()) FgButton(if (armed == "all") t("bin.emptyConfirm") else t("bin.emptyAll"), { if (armed == "all") { Projects.emptyTrash(); armed = null } else armed = "all" }, small = true, on = armed == "all")
                    }
                    Text(t("bin.hint", "days" to Projects.TRASH_DAYS), style = Type.small.copy(color = c.ash), modifier = Modifier.padding(top = 4.dp, bottom = 4.dp))
                }
                if (status.saveError != null) Text(t("cull.saveFailed", "error" to status.saveError), style = Type.small.copy(color = c.ink), modifier = Modifier.padding(top = 12.dp))
                if (status.loadError == "corrupt") Text(t("cull.loadCorrupt"), style = Type.small.copy(color = c.ink), modifier = Modifier.padding(top = 12.dp))
            }
        }
        if (searching) {
            if (search.rawHits != null && groups.isEmpty()) item(span = { GridItemSpan(maxLineSpan) }) {
                Column(Modifier.padding(vertical = 24.dp)) {
                    Text(t("search.none", "q" to search.query.trim()), style = Type.bodyStrong.copy(color = c.ink))
                    Text(t("search.noneHint"), style = Type.rowDesc.copy(color = c.ash), modifier = Modifier.padding(top = 4.dp))
                }
            }
            groups.forEach { (p, hs) ->
                item(span = { GridItemSpan(maxLineSpan) }, key = "sg-${p.id}") {
                    Column(Modifier.animateItem()) {
                        Row(verticalAlignment = Alignment.Bottom, modifier = Modifier.padding(top = 8.dp, bottom = 8.dp)) {
                            Text(p.name, style = Type.heading.copy(color = c.ink))
                            Text(t("search.results", "count" to hs.size), style = Type.monoSmall.copy(color = c.ash), modifier = Modifier.padding(start = 8.dp, bottom = 2.dp))
                        }
                        androidx.compose.foundation.layout.FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                            hs.take(60).forEach { h ->
                                coil3.compose.AsyncImage(java.io.File(h.thumb), h.name, contentScale = androidx.compose.ui.layout.ContentScale.Crop,
                                    modifier = Modifier.height(96.dp).width((96f * h.width / h.height.coerceAtLeast(1)).coerceIn(56f, 200f).dp)
                                        .clip(androidx.compose.foundation.shape.RoundedCornerShape(8.dp))
                                        .clickable { com.fleetinggleam.android.core.Search.setPendingSelect(h.path); search.query = ""; Projects.setActive(p.id) })
                            }
                        }
                    }
                }
            }
            return@LazyVerticalGrid
        }
        // 最近的那个项目做成一张通栏的大卡（封面为主，底部一条渐变上写名字、张数和"继续挑选"），其余两列
        val hero = if (view == "projects" && live.size >= 2) live.maxByOrNull { it.updated } else null
        if (hero != null) {
            item(span = { GridItemSpan(maxLineSpan) }, key = "hero") { HeroCard(hero, onOpen = { Projects.setActive(hero.id) }, modifier = Modifier.animateItem()) }
            item(span = { GridItemSpan(maxLineSpan) }, key = "all-title") {
                Text(t("android.home.allProjects"), style = Type.overline.copy(color = c.ash), modifier = Modifier.padding(start = 2.dp, top = 4.dp))
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
    val haptic = androidx.compose.ui.platform.LocalHapticFeedback.current
    val overlay = com.fleetinggleam.android.ui.components.LocalOverlay.current
    val anchor = com.fleetinggleam.android.ui.components.rememberAnchor()
    // 像相册里的相簿：封面是主角，下面一行名字、一行张数；没有卡片底和描边。长按出菜单（重命名、移到回收站）
    Column(
        modifier
            .riseIn(index + 1)
            .pressScale(src, 0.97f)
            .then(Modifier.anchor(anchor))
            .combinedClickable(interactionSource = src, indication = null, enabled = !inBin, onClick = onOpen, onLongClick = {
                haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.LongPress)
                overlay.showMenu(anchor.value.let { androidx.compose.ui.geometry.Rect(it.left, it.top, it.right, it.top + it.width * 0.55f) }, listOf(
                    com.fleetinggleam.android.ui.components.MenuItem(t("bin.rename"), action = onRename),
                    com.fleetinggleam.android.ui.components.MenuItem(t("bin.toBin"), action = onTrash),
                ), alignEnd = false)
            }),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Box {
            Cover(p.cover.orEmpty().take(4))
        }
        Column(Modifier.padding(horizontal = 2.dp), verticalArrangement = Arrangement.spacedBy(1.dp)) {
            Text(p.name, style = Type.rowTitle.copy(color = c.ink, fontWeight = FontWeight.SemiBold), maxLines = 1, overflow = Ellipsis)
            Text(
                if (inBin) t("bin.daysLeft", "n" to Projects.daysLeft(p), "count" to Projects.daysLeft(p))
                else t("home.photos", "count" to p.count) + if (p.folders.size > 1) " · ${t("home.folders", "count" to p.folders.size)}" else "",
                style = Type.monoSmall.copy(color = c.ash), maxLines = 1,
            )
        }
        if (inBin) Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            FgButton(t("bin.restore"), onRestore, small = true)
            FgButton(if (armed) t("bin.purgeConfirm") else t("bin.purge"), onPurge, small = true, on = armed)
        }
    }
}

/** 最近的项目：通栏大卡，封面铺满，底部一条深色渐变托着名字、张数、上次时间和"继续挑选"。 */
@Composable
private fun HeroCard(p: Project, onOpen: () -> Unit, modifier: Modifier) {
    val c = LocalFg.current
    val src = rememberSource()
    Box(
        modifier.riseIn(1).pressScale(src, 0.985f).fillMaxWidth().aspectRatio(16f / 10f).clip(RoundedCornerShape(20.dp)).background(c.surface)
            .clickable(interactionSource = src, indication = null, onClick = onOpen),
    ) {
        val cover = p.cover.orEmpty()
        if (cover.isEmpty()) Text(t("home.empty"), style = Type.small.copy(color = c.ash), modifier = Modifier.align(Alignment.Center))
        else Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
            AsyncImage(File(cover[0]), null, Modifier.weight(1.6f).fillMaxSize(), contentScale = ContentScale.Crop)
            if (cover.size > 1) Column(Modifier.weight(1f).fillMaxSize(), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                cover.drop(1).take(2).forEach { AsyncImage(File(it), null, Modifier.weight(1f).fillMaxWidth(), contentScale = ContentScale.Crop) }
            }
        }
        Box(Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(120.dp).background(androidx.compose.ui.graphics.Brush.verticalGradient(listOf(Color.Transparent, Color(0xCC000000)))))
        Row(Modifier.align(Alignment.BottomStart).fillMaxWidth().padding(16.dp), verticalAlignment = Alignment.Bottom) {
            Column(Modifier.weight(1f)) {
                Text(t("android.home.recent"), style = Type.monoSmall.copy(color = Color.White.copy(alpha = 0.7f)))
                Text(p.name, style = Type.title.copy(color = Color.White), maxLines = 1, overflow = Ellipsis)
                Text(
                    listOf(t("home.photos", "count" to p.count), p.overrides.size.takeIf { it > 0 }?.let { t("android.home.marked", "count" to it) }, p.updated.takeIf { it > 0 }?.let { if (System.currentTimeMillis() - it < 60_000) t("android.home.justNow") else android.text.format.DateUtils.getRelativeTimeSpanString(it, System.currentTimeMillis(), android.text.format.DateUtils.MINUTE_IN_MILLIS).toString() }).filterNotNull().joinToString(" · "),
                    style = Type.monoSmall.copy(color = Color.White.copy(alpha = 0.85f)), maxLines = 1, overflow = Ellipsis,
                )
            }
            Text(
                t("android.home.continue"), style = Type.label.copy(color = Color(0xFF111113), fontWeight = FontWeight.SemiBold),
                modifier = Modifier.padding(start = 12.dp).clip(CircleShape).background(Color.White).padding(horizontal = 14.dp, vertical = 8.dp),
            )
        }
    }
}

/** 封面：最多四张，1 张整幅，2 张左右，3 张时第一张占左边两格，4 张田字（桌面端 .pcover）。 */
@Composable
private fun Cover(cover: List<String>) {
    val c = LocalFg.current
    val shape = RoundedCornerShape(14.dp)
    Box(Modifier.fillMaxWidth().aspectRatio(1f).clip(shape).background(c.surface), contentAlignment = Alignment.Center) {
        if (cover.isEmpty()) { Text(t("home.empty"), style = Type.small.copy(color = c.ash)); return@Box }
        val img: @Composable (String, Modifier) -> Unit = { path, m -> AsyncImage(File(path), null, m, contentScale = ContentScale.Crop) }
        val gap = 2.dp
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
