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
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.rememberScrollState
import com.fleetinggleam.android.core.Session
import com.kyant.backdrop.backdrops.layerBackdrop

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

    // 一个项目都没有（全在回收站里也算）：起始屏；回收站还有东西就在最下面给个入口
    if (live.isEmpty() && view == "projects") {
        StartScreen(binCount = bin.size, onBin = { view = "bin" })
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
    // 这一页自己的 backdrop：网格录进去，右上角的玻璃按钮从上面折射它（全局液态玻璃）
    val glassOn = com.fleetinggleam.android.ui.components.LocalGlassOn.current
    val backdrop = com.kyant.backdrop.backdrops.rememberLayerBackdrop()
    Box(Modifier.fillMaxSize()) {
    LazyVerticalGrid(
        columns = GridCells.Adaptive(160.dp),
        modifier = Modifier.fillMaxSize().then(if (glassOn) Modifier.layerBackdrop(backdrop) else Modifier).statusBarsPadding(),
        contentPadding = PagePadding,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        item(span = { GridItemSpan(maxLineSpan) }) {
            Column(Modifier.padding(bottom = 4.dp)) {
                if (view == "projects") {
                    // 根页：大标题"项目"；右上的 ⋯ 与 ＋ 是浮在网格上的玻璃圆钮（画在下面的浮层里）
                    Row(Modifier.fillMaxWidth().padding(top = 10.dp).height(36.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(t("home.title"), style = Type.largeTitle.copy(color = c.ink), modifier = Modifier.weight(1f))
                    }
                    if (index.available && allFolders.isNotEmpty())
                        com.fleetinggleam.android.ui.project.SearchField(search, Modifier.padding(top = 12.dp), placeholder = t("search.globalPlaceholder"), autoFocus = false)
                } else {
                    // 回收站：推进页
                    com.fleetinggleam.android.ui.components.NavBar(t("bin.tab"), t("home.title"), { view = "projects" }, inset = 16.dp) {
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
        // 最近的那个项目做成一张通栏的大卡（封面为主，底部一条渐变上写名字、张数和"继续挑选"），其余两列；只有一个项目时就只有大卡
        val hero = if (view == "projects" && live.isNotEmpty()) live.maxByOrNull { it.updated } else null
        if (hero != null) {
            item(span = { GridItemSpan(maxLineSpan) }, key = "hero") { HeroCard(hero, onOpen = { Projects.setActive(hero.id) }, onTrash = { ask = hero }, onRename = { renaming = hero }, modifier = Modifier.animateItem()) }
            if (live.size >= 2) item(span = { GridItemSpan(maxLineSpan) }, key = "all-title") {
                Text(t("android.home.allProjects"), style = Type.overline.copy(color = c.ash), modifier = Modifier.padding(start = 2.dp, top = 8.dp))
            }
        }
        items(if (hero != null && live.size < 2) emptyList() else list, key = { it.id }) { p ->
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
    // ---- 浮层：右上角 ⋯（手动新建、风格、回收站）与 ＋（新建），玻璃圆钮，和大标题同一行
    if (view == "projects") androidx.compose.runtime.CompositionLocalProvider(com.fleetinggleam.android.ui.components.LocalBackdrop provides backdrop) {
        Row(Modifier.align(Alignment.TopEnd).statusBarsPadding().padding(top = 18.dp, end = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            com.fleetinggleam.android.ui.components.MenuButton(t("home.title"), listOf(
                Triple(t("home.newDetailed").trimEnd('→', '…', ' '), true) { scope.launch { Actions.pickFolder()?.let { detailed = Triple(it, baseName(it), "") } } },
                Triple(t("nav.styles"), true, onOpenStyles),
                Triple(t("bin.tab") + if (bin.isNotEmpty()) " · ${bin.size}" else "", bin.isNotEmpty()) { view = "bin" },
            ), glass = true, size = 36.dp)
            com.fleetinggleam.android.ui.components.GlassIconBtn(Icons.PLUS, t("home.new"), { scope.launch { Actions.newProjectFromDialog() } }, box = 18f, size = 36.dp)
        }
    }
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
            // 可见的入口（重命名、移到回收站）：长按还在，但没人知道要长按
            if (!inBin) CardMenuButton(Modifier.align(Alignment.TopEnd).padding(6.dp)) {
                overlay.showMenu(anchor.value.let { androidx.compose.ui.geometry.Rect(it.left, it.top, it.right, it.top + it.width * 0.55f) }, listOf(
                    com.fleetinggleam.android.ui.components.MenuItem(t("bin.rename"), action = onRename),
                    com.fleetinggleam.android.ui.components.MenuItem(t("bin.toBin"), action = onTrash),
                ), alignEnd = true)
            }
        }
        Column(Modifier.padding(horizontal = 2.dp), verticalArrangement = Arrangement.spacedBy(1.dp)) {
            Text(p.name, style = Type.rowTitle.copy(color = c.ink, fontWeight = FontWeight.SemiBold), maxLines = 1, overflow = Ellipsis)
            val bg = AnalysingLine(p)
            if (bg != null) Text(bg, style = Type.monoSmall.copy(color = c.ink), maxLines = 1)
            else Text(
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

/** 封面右上角的"⋯"：压在照片上的圆钮的统一规格——26dp、黑 30% 底、1dp 白 20% 边、白 85% 的图标（设计审查 A2.5 / A8.4）。 */
@Composable
private fun CardMenuButton(modifier: Modifier, onClick: () -> Unit) {
    Box(
        modifier.size(26.dp).clip(androidx.compose.foundation.shape.CircleShape).background(Color(0x4D000000)).border(1.dp, Color(0x33FFFFFF), androidx.compose.foundation.shape.CircleShape)
            .clickable(interactionSource = rememberSource(), indication = null, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        androidx.compose.foundation.Canvas(Modifier.size(14.dp)) {
            val r = 1.5.dp.toPx()
            for (k in 0..2) drawCircle(Color.White.copy(alpha = 0.85f), r, androidx.compose.ui.geometry.Offset(size.width * (0.2f + 0.3f * k), size.height / 2))
        }
    }
}

/** 这个项目正在后台分析：卡片上那一行写"分析中 n / 总数"（暂停了写暂停的原因）。 */
@Composable
private fun AnalysingLine(p: Project): String? {
    val run by Session.running.collectAsState()
    val prog by Session.progress.collectAsState()
    if (run?.id != p.id) return null
    return if (prog.total > 0) t("android.home.analysing", "done" to prog.done, "total" to prog.total) else t("cull.analyzing")
}

/** 最近的项目：通栏大卡，封面铺满，底部一条深色渐变托着名字、张数、上次时间和"继续挑选"。长按和小卡一样出菜单（重命名、移到回收站）。 */
@Composable
private fun HeroCard(p: Project, onOpen: () -> Unit, onTrash: () -> Unit, onRename: () -> Unit, modifier: Modifier) {
    val c = LocalFg.current
    val src = rememberSource()
    val haptic = androidx.compose.ui.platform.LocalHapticFeedback.current
    val overlay = com.fleetinggleam.android.ui.components.LocalOverlay.current
    val anchor = com.fleetinggleam.android.ui.components.rememberAnchor()
    Box(
        modifier.riseIn(1).pressScale(src, 0.985f).fillMaxWidth().aspectRatio(16f / 10f).clip(RoundedCornerShape(16.dp)).background(c.surface)
            .then(Modifier.anchor(anchor))
            .combinedClickable(interactionSource = src, indication = null, onClick = onOpen, onLongClick = {
                haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.LongPress)
                overlay.showMenu(anchor.value.let { androidx.compose.ui.geometry.Rect(it.left, it.top, it.right, it.top + it.height * 0.6f) }, listOf(
                    com.fleetinggleam.android.ui.components.MenuItem(t("bin.rename"), action = onRename),
                    com.fleetinggleam.android.ui.components.MenuItem(t("bin.toBin"), action = onTrash),
                ), alignEnd = false)
            }),
    ) {
        val cover = p.cover.orEmpty()
        if (cover.isEmpty()) Text(t("home.empty"), style = Type.small.copy(color = c.ash), modifier = Modifier.align(Alignment.Center))
        else Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
            AsyncImage(File(cover[0]), null, Modifier.weight(1.6f).fillMaxSize(), contentScale = ContentScale.Crop)
            if (cover.size > 1) Column(Modifier.weight(1f).fillMaxSize(), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                cover.drop(1).take(2).forEach { AsyncImage(File(it), null, Modifier.weight(1f).fillMaxWidth(), contentScale = ContentScale.Crop) }
            }
        }
        // 底部渐变 96dp、0 → 65% 黑：托住两行字就够，不压封面（设计审查 A2.4）
        Box(Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(96.dp).background(androidx.compose.ui.graphics.Brush.verticalGradient(listOf(Color.Transparent, Color(0xA6000000)))))
        CardMenuButton(Modifier.align(Alignment.TopEnd).padding(10.dp)) {
            overlay.showMenu(anchor.value.let { androidx.compose.ui.geometry.Rect(it.left, it.top, it.right, it.top + it.height * 0.6f) }, listOf(
                com.fleetinggleam.android.ui.components.MenuItem(t("bin.rename"), action = onRename),
                com.fleetinggleam.android.ui.components.MenuItem(t("bin.toBin"), action = onTrash),
            ), alignEnd = true)
        }
        // 只两行：名字、张数（位置已经说明"最近"；整张卡可点，不再放"继续挑选"胶囊——设计审查 A2.2 / A2.3）。分析中时上面多一行进度
        Column(Modifier.align(Alignment.BottomStart).fillMaxWidth().padding(16.dp)) {
            AnalysingLine(p)?.let { Text(it, style = Type.monoSmall.copy(color = Color.White.copy(alpha = 0.95f))) }
            Text(p.name, style = Type.title.copy(color = Color.White), maxLines = 1, overflow = Ellipsis)
            Text(
                listOf(t("home.photos", "count" to p.count), p.overrides.size.takeIf { it > 0 }?.let { t("android.home.marked", "count" to it) }, p.updated.takeIf { it > 0 }?.let { if (System.currentTimeMillis() - it < 60_000) t("android.home.justNow") else android.text.format.DateUtils.getRelativeTimeSpanString(it, System.currentTimeMillis(), android.text.format.DateUtils.MINUTE_IN_MILLIS).toString() }).filterNotNull().joinToString(" · "),
                style = Type.monoSmall.copy(color = Color.White.copy(alpha = 0.85f)), maxLines = 1, overflow = Ellipsis,
            )
        }
    }
}

/** 封面：最多四张，1 张整幅，2 张左右，3 张时第一张占左边两格，4 张田字（桌面端 .pcover）。 */
@Composable
private fun Cover(cover: List<String>) {
    val c = LocalFg.current
    val shape = RoundedCornerShape(10.dp)
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

/**
 * 一个项目都没有时的起始屏（作者 2026-10-22：不做滑动式引导页，空状态本身就是第一步）：插图、一句话、三条事实、整行的"从相册开始"，
 * 下面文字入口"选一个文件夹" / "连接 NAS"，最后"用示例照片先试试"——不想先给相册权限的人也能看一眼效果。建好第一个项目就直接进去。
 */
@Composable
private fun StartScreen(binCount: Int, onBin: () -> Unit) {
    val c = LocalFg.current
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val scope = rememberCoroutineScope()
    val facts = listOf(
        Triple("M8 1.5l5.5 2.3v3.4c0 3.3-2.3 6.1-5.5 7.3C4.8 13.3 2.5 10.5 2.5 7.2V3.8z", t("android.start.fact1"), 0),
        Triple(Icons.CULL, t("android.start.fact2"), 1),
        Triple("M2.5 8a5.5 5.5 0 109.2-4M11.5 1.5v3h-3M6.2 8.2l1.4 1.4 2.6-2.8", t("android.start.fact3"), 2),
    )
    // 三组入场（插画 / 文字 / 按钮），组间 60 ms（设计审查 A1.2）；文字入口合成一行（A1.3）；没有品牌名那行，标语直接点题（A1.4）
    val entry: @Composable (String, () -> Unit) -> Unit = { text, onClick ->
        Text(text, style = Type.label.copy(color = c.ink), modifier = Modifier.clip(CircleShape).clickable(onClick = onClick).padding(horizontal = 10.dp, vertical = 10.dp), maxLines = 1)
    }
    val dot: @Composable () -> Unit = { Text("·", style = Type.label.copy(color = c.ash)) }
    Column(
        Modifier.fillMaxSize().statusBarsPadding().verticalScroll(rememberScrollState()).padding(horizontal = 28.dp).padding(top = 32.dp, bottom = 150.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(Modifier.riseIn(0, scaleFrom = 0.94f)) { com.fleetinggleam.android.ui.picker.PhotoStack() }
        Column(Modifier.riseIn(2).fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
            Text(t("brand.tagline"), style = Type.title.copy(color = c.ink), textAlign = TextAlign.Center, modifier = Modifier.padding(top = 24.dp))
            Column(Modifier.padding(top = 24.dp).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                facts.forEach { (icon, text, _) ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        com.fleetinggleam.android.ui.components.IconBox(icon)
                        Text(text, style = Type.body.copy(color = c.ink), modifier = Modifier.padding(start = 12.dp))
                    }
                }
            }
        }
        Column(Modifier.riseIn(4).fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
            FgButton(t("android.start.album"), { scope.launch { Actions.newProjectFromDialog(source = "albums") } }, primary = true, modifier = Modifier.padding(top = 32.dp).fillMaxWidth())
            Row(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
                entry(t("android.start.folder")) { scope.launch { Actions.newProjectFromDialog(source = "folder") } }
                dot()
                entry(t("android.start.nas")) { com.fleetinggleam.android.core.Nav.openSettings("nas") }
                dot()
                entry(t("android.start.sample")) { scope.launch { com.fleetinggleam.android.core.Samples.open(ctx) } }
                if (binCount > 0) { dot(); entry(t("bin.tab") + " · $binCount", onBin) }
            }
            Text(t("nav.privacy"), style = Type.small.copy(color = c.ash), textAlign = TextAlign.Center, modifier = Modifier.padding(top = 24.dp))
        }
    }
}

