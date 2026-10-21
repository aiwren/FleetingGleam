package com.fleetinggleam.android.ui.project

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import com.fleetinggleam.android.core.Decision
import com.fleetinggleam.android.core.Hit
import com.fleetinggleam.android.core.Models
import com.fleetinggleam.android.core.Answer
import com.fleetinggleam.android.core.Photo
import com.fleetinggleam.android.core.Project
import com.fleetinggleam.android.core.Projects
import com.fleetinggleam.android.core.Reason
import com.fleetinggleam.android.core.Scenes
import com.fleetinggleam.android.core.Search
import com.fleetinggleam.android.core.Session
import com.fleetinggleam.android.core.Settings
import com.fleetinggleam.android.core.UiPrefs
import com.fleetinggleam.android.core.str
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.components.GlassBox
import com.fleetinggleam.android.ui.components.LocalBackdrop
import com.fleetinggleam.android.ui.components.glassStyle
import com.fleetinggleam.android.ui.components.rememberPress
import com.fleetinggleam.android.ui.components.riseIn
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import com.kyant.backdrop.backdrops.layerBackdrop
import com.kyant.backdrop.backdrops.rememberLayerBackdrop
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.intOrNull
import java.io.File

// ------------------------------------------------------------------------------------------------ 状态

/** 四种浏览方式（桌面端 LAYOUTS），图标是桌面端的同一组形状。 */
enum class ViewStyle(val id: String, val icon: String) {
    Justified("justified", "M1.5 2h6v5h-6zM9 2h5.5v5H9zM1.5 9h4v5h-4zM7 9h7.5v5H7z"),
    Grid("grid", "M1.5 1.5h5v5h-5zM9.5 1.5h5v5h-5zM1.5 9.5h5v5h-5zM9.5 9.5h5v5h-5z"),
    Single("single", "M1.5 2h13v8h-13zM4 13.5h1.5M7.3 13.5h1.5M10.5 13.5H12"),
    Compare("compare", "M1.5 2.5h5.5v11H1.5zM9 2.5h5.5v11H9z");
    companion object { fun of(id: String) = entries.firstOrNull { it.id == id } ?: Justified }
}

const val UNDO_ICON = "M5.5 3.5L2.5 6.5l3 3M2.5 6.5h7a3.8 3.8 0 010 7.6H8"

/**
 * 一个项目的选片界面的状态（桌面端 Culling.tsx 里那一串 useState）：筛选、选中、大图、搜索、场景页、浏览方式。
 * 换项目时整个换一份。
 */
@Stable
class CullState {
    var mode by mutableStateOf("all")
    var sceneFilter by mutableStateOf<Int?>(null)
    var personFilter by mutableStateOf<Int?>(null)
    var selected by mutableStateOf<String?>(null)
    /** 选中是点出来的（就在手指下，不用滚过去）。 */
    var selectedByTap = false
    var viewer by mutableStateOf(false)
    var searchOpen by mutableStateOf(false)
    var query by mutableStateOf("")
    var rawHits by mutableStateOf<List<Hit>?>(null)
    var searchRev by mutableIntStateOf(0)
    var sceneView by mutableStateOf(false)
    var filterSheet by mutableStateOf(false)
    var viewSheet by mutableStateOf(false)
    var layout by mutableStateOf(ViewStyle.of(UiPrefs.str("layout", "justified")))
        private set
    var hintUses by mutableIntStateOf(UiPrefs.int("hint.dock", 0))
        private set
    /** 每张照片在屏幕上的位置：打开大图时照片从它的格子里飞出来。 */
    val rects = HashMap<String, Rect>()

    fun pickLayout(v: ViewStyle) { layout = v; UiPrefs.put("layout", v.id) }
    fun bumpHint(n: Int = 1) { hintUses = if (n == 99) 99 else maxOf(hintUses, hintUses + 1); UiPrefs.put("hint.dock", hintUses) }
    fun select(path: String?, byTap: Boolean = false) { selectedByTap = byTap; selected = path }
    val filtered: Boolean get() = sceneFilter != null || personFilter != null
}

data class SceneItem(val scene: Int, val cover: Photo, val total: Int, val count: Int, val text: String)
data class PersonItem(val person: Int, val photo: Photo, val box: List<Float>, val total: Int, val count: Int)

/** 由照片、取舍与筛选推出来的一切（桌面端那一串 useMemo）。 */
class CullData(
    val shown: List<Photo>,
    val status: Map<String, Int>,
    val scenes: List<SceneItem>,
    val people: List<PersonItem>,
    val byPath: Map<String, Photo>,
    val hits: List<Hit>?,
) {
    fun index(path: String?): Int = if (path == null) -1 else shown.indexOfFirst { it.path == path }
}

/** 照片按项目的排序方式排好（和桌面端一样：按相似分组时，组保持在文件夹里的位置，组里最好的在前）。 */
fun ordered(photos: List<Photo>, sortBy: String): List<Photo> = when (sortBy) {
    "score" -> photos.sortedByDescending { it.score }
    "time" -> photos.sortedBy { it.takenAt ?: 0L }
    "group" -> {
        val first = HashMap<Int, Int>()
        photos.forEachIndexed { i, p -> first.putIfAbsent(p.group, i) }
        photos.sortedWith(compareBy<Photo> { first[it.group] }.thenByDescending { it.score })
    }
    else -> photos
}

@Composable
fun rememberCullData(cull: CullState, photos: List<Photo>, decisions: Map<String, Decision>, sortBy: String, sceneText: Map<Int, String>): CullData {
    val mode = cull.mode; val sceneF = cull.sceneFilter; val personF = cull.personFilter; val selected = cull.selected
    val rawHits = cull.rawHits; val query = cull.query
    val byPath = remember(photos) { photos.associateBy { it.path } }
    val order = remember(photos, sortBy) { ordered(photos, sortBy) }
    // 场景：后端做"兼顾不同场景"时已经分好的、相似的大簇；按大小排，同样大的保持出现的先后，封面是分最高的一张
    val scenes0 = remember(photos) {
        val by = LinkedHashMap<Int, Triple<Int, Photo, Int>>()
        for (p in photos) {
            val s = by[p.scene]
            by[p.scene] = if (s == null) Triple(p.scene, p, 1) else Triple(s.first, if (p.score > s.second.score) p else s.second, s.third + 1)
        }
        by.values.sortedByDescending { it.third }
    }
    // 人物：后端认出来的、反复出现的脸；头像是这个人最大的一张脸
    val people0 = remember(photos) {
        data class Acc(var photo: Photo, var box: List<Float>, var area: Float, val paths: MutableSet<String>)
        val by = LinkedHashMap<Int, Acc>()
        for (p in photos) p.people.forEachIndexed { fi, id ->
            val box = p.faceBoxes.getOrNull(fi)
            if (id < 0 || box == null || box.size < 4) return@forEachIndexed
            val area = box[2] * box[3]
            val e = by[id]
            if (e == null) by[id] = Acc(p, box, area, mutableSetOf(p.path))
            else { e.paths.add(p.path); if (area > e.area) { e.area = area; e.photo = p; e.box = box } }
        }
        by.entries.sortedWith(compareByDescending<Map.Entry<Int, Acc>> { it.value.paths.size }.thenBy { it.key }).map { (id, a) -> PersonItem(id, a.photo, a.box, a.paths.size, a.paths.size) }
    }
    // 筛选面板里的每个数：要是现在点它，墙上会有几张（另外两个筛选照旧）
    val facets = remember(photos, decisions, mode, sceneF, personF) {
        fun st(p: Photo) = decisions[p.path]?.state
        val inScene = { p: Photo -> sceneF == null || p.scene == sceneF }
        val inPerson = { p: Photo -> personF == null || p.people.contains(personF) }
        val inMode = { p: Photo -> mode == "all" || st(p) == mode }
        val status = HashMap<String, Int>().apply { put("all", 0); put("keep", 0); put("review", 0); put("reject", 0) }
        val sceneN = HashMap<Int, Int>(); val personN = HashMap<Int, Int>()
        for (p in photos) {
            if (inScene(p) && inPerson(p)) { status["all"] = status["all"]!! + 1; st(p)?.let { status[it] = (status[it] ?: 0) + 1 } }
            if (inMode(p) && inPerson(p)) sceneN[p.scene] = (sceneN[p.scene] ?: 0) + 1
            if (inMode(p) && inScene(p)) p.people.filter { it >= 0 }.toSet().forEach { personN[it] = (personN[it] ?: 0) + 1 }
        }
        Triple(status, sceneN, personN)
    }
    val scenes = remember(scenes0, facets, sceneText) { scenes0.map { (id, cover, total) -> SceneItem(id, cover, total, facets.second[id] ?: 0, sceneText[id] ?: "") } }
    val people = remember(people0, facets) { people0.map { it.copy(count = facets.third[it.person] ?: 0) } }
    // 场景的那句话也能搜到："花田"或"building"会找出描述成这样的场景里的每一张，排在文字模型找到的前面
    val hits = remember(rawHits, query, photos, sceneText) {
        val q = query.trim().lowercase()
        if (rawHits == null || q.isEmpty() || sceneText.isEmpty() || (q.length < 3 && q.all { it.code < 128 })) rawHits
        else {
            val tagged = photos.filter { sceneText[it.scene]?.lowercase()?.contains(q) == true }
            if (tagged.isEmpty()) rawHits else {
                val tagSet = tagged.map { it.path }.toSet()
                val rank = rawHits.map { it.path }.toSet()
                val first = rawHits.filter { it.path in tagSet }
                val extra = tagged.filter { it.path !in rank }.sortedByDescending { it.score }.map { Hit(it.path, it.name, it.thumb, it.width, it.height, it.score) }
                first + extra + rawHits.filter { it.path !in tagSet }
            }
        }
    }
    val shown = remember(order, decisions, mode, sceneF, personF, selected, hits) {
        fun matchesView(p: Photo) = (mode == "all" || decisions[p.path]?.state == mode) && (sceneF == null || p.scene == sceneF) && (personF == null || p.people.contains(personF))
        // 选中的那张一直留在眼前，直到你移开——哪怕刚做的改动让它不再符合这个视图
        val carried = selected?.let { byPath[it] }?.takeIf { !matchesView(it) }?.path
        val base = order.filter { matchesView(it) || it.path == carried }
        if (hits == null) base else {
            val rank = hits.withIndex().associate { (i, h) -> h.path to i }
            base.filter { it.path in rank }.sortedBy { rank[it.path] }
        }
    }
    return remember(shown, facets, scenes, people, byPath, hits) { CullData(shown, facets.first, scenes, people, byPath, hits) }
}

/** 一条理由的文字（桌面端 reasonText）：参数原样带进去，`stateKey` 先翻译成状态的名字。 */
fun reasonText(r: Reason): String {
    val params = r.p?.entries?.map { (k, v) ->
        val prim = v as? JsonPrimitive
        k to (prim?.let { p -> p.intOrNull ?: p.doubleOrNull ?: p.contentOrNull })
    } ?: emptyList()
    val state = r.p?.str("stateKey")?.let { t(it) }
    return t(r.k, *(params + ("state" to state)).toTypedArray())
}

/** 改判，并处理筛选视图里的细节（桌面端 decide）：改了之后不再属于这个视图的那张，选中挪到它的邻居上。 */
fun decideIn(cull: CullState, data: CullData, path: String, st: String?) {
    Session.decide(path, st)
    cull.bumpHint()
    if (st != null && cull.mode != "all" && st != cull.mode) {
        val i = data.shown.indexOfFirst { it.path == path }
        val nb = data.shown.getOrNull(i + 1) ?: data.shown.getOrNull(i - 1)
        cull.select(nb?.path?.takeIf { it != path })
    }
}

// ------------------------------------------------------------------------------------------------ 选片界面

/** 分析好的项目：状态条、照片墙 / 单张 / 对比 / 场景页，底部一条液态玻璃工具栏。 */
@Composable
fun Culling(project: Project, cull: CullState, data: CullData, onOpenStyles: () -> Unit, onOrganize: () -> Unit) {
    val c = LocalFg.current
    val photos by Session.photos.collectAsState()
    val decisions by Session.decisions.collectAsState()
    val celebrate by Session.celebrate.collectAsState()
    val undoN by Session.undoCount.collectAsState()
    val describing by Scenes.describing.collectAsState()
    val packs by Models.packs.collectAsState()
    val haptic = LocalHapticFeedback.current
    val scope = rememberCoroutineScope()
    val sortBy = project.sort ?: "group"
    val backdrop = rememberLayerBackdrop()
    val scenePack = packs.firstOrNull { it.id == "scenetext" }?.installed == true

    // 照片定下来之后给场景起名；装了描述模型时，叫不出名字的也写一句话
    LaunchedEffect(photos, scenePack) { Scenes.describe(photos, scenePack) }
    // 全局搜索里点的那张：项目打开后选中
    LaunchedEffect(photos) { Search.takePendingSelect()?.let { want -> if (photos.any { it.path == want }) cull.select(want) } }
    // 单张与对比总是显示一张：没选（或选的被筛掉了）就选第一张
    LaunchedEffect(cull.layout, data.shown) {
        if ((cull.layout == ViewStyle.Single || cull.layout == ViewStyle.Compare) && data.shown.isNotEmpty() && data.shown.none { it.path == cull.selected }) cull.select(data.shown[0].path)
    }
    // 打开大图或单张时，后台先准备前后几张
    LaunchedEffect(cull.selected, cull.viewer, cull.layout) {
        val i = data.index(cull.selected)
        if (i >= 0 && (cull.viewer || cull.layout == ViewStyle.Single)) com.fleetinggleam.android.core.Large.prefetch(listOf(1, -1, 2, -2, 3).mapNotNull { data.shown.getOrNull(i + it)?.path })
    }

    val decide: (String, String?) -> Unit = { path, st -> haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove); decideIn(cull, data, path, st) }
    val open: (String) -> Unit = { path -> cull.select(path, byTap = true); cull.viewer = true }

    androidx.activity.compose.BackHandler(cull.sceneView) { cull.sceneView = false }
    androidx.activity.compose.BackHandler(!cull.sceneView && cull.searchOpen) { cull.searchOpen = false; cull.query = "" }
    Box(Modifier.fillMaxSize()) {
        Box(Modifier.fillMaxSize().layerBackdrop(backdrop)) {
            val bottomPad = 92.dp
            when {
                cull.sceneView -> SceneGallery(project, data, cull, describing, canDescribe = !scenePack && data.scenes.any { it.text.isEmpty() },
                    onDescribe = { scope.launch { if (Models.ensurePack("scenetext") != Answer.Cancelled) Scenes.describe(photos, true) } }, bottomPad = bottomPad)
                cull.layout == ViewStyle.Single && data.shown.isNotEmpty() -> Column(Modifier.fillMaxSize()) {
                    StatusStrip(cull, data)
                    SinglePager(data.shown, decisions, cull, onDecide = decide, onOpen = open, bottomPad = bottomPad)
                }
                cull.layout == ViewStyle.Compare && data.shown.isNotEmpty() -> Column(Modifier.fillMaxSize()) {
                    StatusStrip(cull, data)
                    CompareView(data.shown, if (cull.personFilter == null) photos else photos.filter { it.people.contains(cull.personFilter) }, if (cull.sceneFilter != null) "scene" else "group",
                        cull, decisions, onDecide = decide, onOpen = open, bottomPad = bottomPad)
                }
                else -> {
                    val items = remember(data.shown) { data.shown.map { WallItem(it.path, it.ratio) } }
                    val state = rememberLazyListState()
                    // 选中换了（不是点出来的：撤销、全局搜索、单张里换的）：把它滚到眼前
                    val reveal = cull.selected.takeIf { !cull.selectedByTap }
                    Wall(items, if (cull.layout == ViewStyle.Grid) "grid" else "justified", state, PaddingValues(start = 12.dp, end = 12.dp, top = 4.dp, bottom = bottomPad),
                        header = {
                            Column {
                                StatusStrip(cull, data, padded = false)
                                if (data.shown.isEmpty()) EmptyView(cull, data)
                            }
                        }, reveal = reveal) { i, w, h ->
                        val p = data.shown[i]
                        val d = decisions[p.path]
                        val st = d?.state ?: ""
                        Box(Modifier.onGloballyPositioned { cull.rects[p.path] = it.boundsInRoot() }) {
                            Tile(p.path, w, h, p, null, st, selected = cull.selected == p.path, reveal = celebrate, dim = st == "reject" && cull.mode == "all", manual = d?.manual == true,
                                showScore = sortBy == "score", index = i, interactive = true,
                                onSelect = { path -> if (cull.selected == path) open(path) else cull.select(path, byTap = true) }, onOpen = open, onDecide = decide)
                        }
                    }
                }
            }
        }

        CompositionLocalProvider(LocalBackdrop provides backdrop) {
            // 操作提示：前几次改判之后就不再出现
            AnimatedVisibility(cull.hintUses < 4 && !cull.sceneView, modifier = Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(bottom = 74.dp, start = 16.dp, end = 16.dp),
                enter = fadeIn() + slideInVertically { it / 2 }, exit = fadeOut()) {
                GlassBox(shape = RoundedCornerShape(16.dp), style = glassStyle(strong = true)) {
                    Row(Modifier.padding(start = 16.dp, end = 6.dp, top = 6.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(t("android.cull.hint"), style = Type.rowDesc.copy(color = c.ink), modifier = Modifier.weight(1f, fill = false))
                        Text(t("cull.hintDismiss"), style = Type.label.copy(color = c.ink, fontWeight = FontWeight.SemiBold), modifier = Modifier.clip(RoundedCornerShape(10.dp)).clickable { cull.bumpHint(99) }.padding(horizontal = 12.dp, vertical = 10.dp))
                    }
                }
            }
            CullBar(cull, undoN, Modifier.align(Alignment.BottomCenter), onOrganize = onOrganize, onUndo = {
                Session.undo()?.let { cull.select(it) }
                haptic.performHapticFeedback(HapticFeedbackType.LongPress)
            })
        }
    }

    FilterSheet(cull, data)
    ViewSheet(cull, sortBy) { Projects.update(project.id) { p -> p.copy(sort = it) } }
}

/** 筛选后一张也没有：搜索时说没找到，否则说这个视图里没有照片。 */
@Composable
private fun EmptyView(cull: CullState, data: CullData) {
    val c = LocalFg.current
    Column(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 40.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        if (data.hits != null) {
            Text(t("search.none", "q" to cull.query.trim()), style = Type.bodyStrong.copy(color = c.ink))
            Text(t("search.noneHint"), style = Type.rowDesc.copy(color = c.ash), modifier = Modifier.padding(top = 6.dp))
        } else Text(t("cull.emptyView"), style = Type.rowDesc.copy(color = c.ash))
    }
}

/**
 * 状态条：全部 / 保留 / 待复查 / 淘汰，后面跟着正在用的人物与场景筛选（点 × 去掉）。数字是现在点它墙上会有几张。
 */
@Composable
fun StatusStrip(cull: CullState, data: CullData, padded: Boolean = true) {
    val c = LocalFg.current
    Row(
        Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = if (padded) 12.dp else 0.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically,
    ) {
        listOf("all", "keep", "review", "reject").forEach { id ->
            val on = cull.mode == id
            Row(
                Modifier.clip(CircleShape).background(if (on) c.ink else c.surface).border(1.dp, if (on) Color.Transparent else c.line, CircleShape)
                    .clickable { cull.mode = id }.padding(horizontal = 10.dp, vertical = 5.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                StateIcon(id, if (on) c.onInk else c.ink, 11.dp)
                Text(t("state.$id"), style = Type.rowDesc.copy(color = if (on) c.onInk else c.ink, fontWeight = FontWeight.Medium, fontSize = androidx.compose.ui.unit.TextUnit(12f, androidx.compose.ui.unit.TextUnitType.Sp)), modifier = Modifier.padding(start = 4.dp))
                Text("${data.status[id] ?: 0}", style = Type.monoSmall.copy(color = if (on) c.onInk.copy(alpha = 0.7f) else c.ash, fontSize = androidx.compose.ui.unit.TextUnit(11f, androidx.compose.ui.unit.TextUnitType.Sp)), modifier = Modifier.padding(start = 5.dp))
            }
        }
        // 人物与场景的筛选：状态已经在这一排上了，所以这里只放一个入口（项目里有人物或不止一个场景时才有）
        if ((data.people.isNotEmpty() || data.scenes.size > 1) && cull.personFilter == null && cull.sceneFilter == null) {
            Row(
                Modifier.clip(CircleShape).background(c.surface).border(1.dp, c.line, CircleShape).clickable { cull.filterSheet = true }.padding(horizontal = 10.dp, vertical = 5.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                FgIcon(com.fleetinggleam.android.ui.components.Icons.FILTERS, size = 12.dp, color = c.ink)
                Text(t(if (data.people.isNotEmpty()) "android.cull.peopleScenes" else "cull.scenes"), style = Type.rowDesc.copy(color = c.ink, fontWeight = FontWeight.Medium, fontSize = androidx.compose.ui.unit.TextUnit(12f, androidx.compose.ui.unit.TextUnitType.Sp)), modifier = Modifier.padding(start = 4.dp))
            }
        }
        cull.personFilter?.let { id ->
            data.people.firstOrNull { it.person == id }?.let { p ->
                FilterChip(onClear = { cull.personFilter = null }) { FaceAvatar(p.photo, p.box, 22.dp); Text(t("cull.people"), style = Type.rowDesc.copy(color = c.ink), modifier = Modifier.padding(start = 6.dp)) }
            }
        }
        cull.sceneFilter?.let { id ->
            data.scenes.firstOrNull { it.scene == id }?.let { s ->
                FilterChip(onClear = { cull.sceneFilter = null }) {
                    AsyncImage(File(s.cover.thumb), null, Modifier.size(22.dp).clip(RoundedCornerShape(6.dp)), contentScale = ContentScale.Crop)
                    Text(s.text.ifEmpty { t("cull.scenes") }, style = Type.rowDesc.copy(color = c.ink), maxLines = 1, overflow = Ellipsis, modifier = Modifier.padding(start = 6.dp).width(120.dp.coerceAtMost(if (s.text.isEmpty()) 60.dp else 120.dp)))
                }
            }
        }
    }
}

private fun Dp.coerceAtMost(v: Dp): Dp = if (this > v) v else this

@Composable
private fun FilterChip(onClear: () -> Unit, content: @Composable () -> Unit) {
    val c = LocalFg.current
    Row(Modifier.clip(CircleShape).background(c.surface).border(1.dp, c.ink.copy(alpha = 0.35f), CircleShape).clickable(onClick = onClear).padding(start = 5.dp, end = 10.dp, top = 4.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        content()
        FgIcon("M4 4l8 8M12 4l-8 8", size = 10.dp, stroke = 1.8f, color = c.ash, modifier = Modifier.padding(start = 6.dp))
    }
}

/** 保留 / 待复查 / 淘汰的小图标（和桌面端筛选面板一样：实心星、空心星、×、四格）。 */
@Composable
fun StateIcon(id: String, color: Color, size: Dp = 13.dp) {
    when (id) {
        "keep" -> Star(true, size, color = if (color == LocalFg.current.onInk) color else LocalFg.current.gleam, glow = false)
        "review" -> Star(false, size, color = color, glow = false)
        "reject" -> FgIcon("M3 3l6 6M9 3L3 9", size = size * 0.8f, box = 12f, stroke = 1.8f, color = color)
        else -> FgIcon("M1.5 1.5h4v4h-4zM6.5 1.5h4v4h-4zM1.5 6.5h4v4h-4zM6.5 6.5h4v4h-4z", size = size * 0.85f, box = 12f, stroke = 1.4f, color = color)
    }
}

/**
 * 一张脸的圆形裁切（桌面端 FaceAvatar）：从照片的缩略图里按人脸框取出来（缩略图保持照片的比例，框直接可用）。
 */
@Composable
fun FaceAvatar(photo: Photo, box: List<Float>, size: Dp) {
    val (x, y, w, h) = box
    val W = photo.width.toFloat(); val H = photo.height.toFloat()
    val side = maxOf(w * W, h * H) * 1.7f
    val k = size.value / side
    Box(Modifier.size(size).clip(CircleShape).background(LocalFg.current.surface)) {
        AsyncImage(
            File(photo.thumb), null, contentScale = ContentScale.FillBounds,
            modifier = Modifier.wrapContentSize(Alignment.TopStart, unbounded = true)
                .offset((size.value / 2 - (x + w / 2) * W * k).dp, (size.value / 2 - (y + h / 2) * H * k).dp)
                .size((W * k).dp, (H * k).dp),
        )
    }
}

// ------------------------------------------------------------------------------------------------ 底部工具栏

/** 底部的液态玻璃工具栏：筛选、浏览方式、撤销。 */
@Composable
private fun CullBar(cull: CullState, undoN: Int, modifier: Modifier, onOrganize: () -> Unit, onUndo: () -> Unit) {
    val c = LocalFg.current
    Box(modifier.navigationBarsPadding().padding(bottom = 10.dp)) {
        GlassBox(shape = RoundedCornerShape(50)) {
            Row(Modifier.padding(4.dp), verticalAlignment = Alignment.CenterVertically) {
                if (cull.sceneView) {
                    BarItem("M10 3L5 8l5 5", t("cull.backToPhotos")) { cull.sceneView = false }
                } else {
                    BarItem(cull.layout.icon, t("cull.layout.${cull.layout.id}")) { cull.viewSheet = true }
                    BarItem("M2.5 5.5h11M2.5 8h11M2.5 10.5h7", t("organize.button"), onClick = onOrganize)
                    BarItem(UNDO_ICON, t("android.cull.undo"), enabled = undoN > 0, onClick = onUndo)
                }
            }
        }
    }
}

@Composable
private fun BarItem(icon: String, label: String, enabled: Boolean = true, dot: Boolean = false, onClick: () -> Unit) {
    val c = LocalFg.current
    val src = remember { MutableInteractionSource() }
    val press = rememberPress(src)
    Column(
        Modifier.width(72.dp).height(46.dp).clip(RoundedCornerShape(23.dp))
            .graphicsLayer { val s = 1f + 0.08f * press(); scaleX = s; scaleY = s; alpha = if (enabled) 1f else 0.35f }
            .clickable(interactionSource = src, indication = null, enabled = enabled, onClick = onClick),
        horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center,
    ) {
        Box {
            FgIcon(icon, size = 18.dp, color = c.ink)
            if (dot) Box(Modifier.align(Alignment.TopEnd).offset(4.dp, (-2).dp).size(7.dp).clip(CircleShape).background(c.ink))
        }
        Text(label, style = Type.monoSmall.copy(fontFamily = com.fleetinggleam.android.ui.theme.Geist, color = c.ink, fontWeight = FontWeight.SemiBold, fontSize = androidx.compose.ui.unit.TextUnit(10f, androidx.compose.ui.unit.TextUnitType.Sp), lineHeight = androidx.compose.ui.unit.TextUnit(12f, androidx.compose.ui.unit.TextUnitType.Sp)), maxLines = 1, overflow = Ellipsis, modifier = Modifier.padding(top = 1.dp))
    }
}

// ------------------------------------------------------------------------------------------------ 筛选与浏览方式

/** 筛选（桌面端 FilterPanel 的手机版，放在底部弹出的面板里）：状态、人物、场景，三者叠加。 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun FilterSheet(cull: CullState, data: CullData) {
    if (!cull.filterSheet) return
    val c = LocalFg.current
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    var allPeople by remember { mutableStateOf(false) }
    ModalBottomSheet(onDismissRequest = { cull.filterSheet = false }, sheetState = sheet, containerColor = c.canvas) {
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(horizontal = 20.dp).padding(bottom = 20.dp)) {
            Text(t("cull.filters"), style = Type.heading.copy(color = c.ink))
            SheetTitle(t("cull.status"))
            Column(Modifier.clip(RoundedCornerShape(14.dp)).background(c.panel).border(1.dp, c.line, RoundedCornerShape(14.dp))) {
                listOf("all", "keep", "review", "reject").forEachIndexed { i, id ->
                    if (i > 0) Box(Modifier.fillMaxWidth().height(1.dp).background(c.line))
                    Row(Modifier.fillMaxWidth().clickable { cull.mode = id }.padding(horizontal = 16.dp, vertical = 13.dp), verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.width(20.dp), contentAlignment = Alignment.Center) { StateIcon(id, c.ink, 14.dp) }
                        Text(t("state.$id"), style = Type.rowTitle.copy(color = c.ink), modifier = Modifier.weight(1f).padding(start = 10.dp))
                        Text("${data.status[id] ?: 0}", style = Type.mono.copy(color = c.ash))
                        Box(Modifier.padding(start = 12.dp).size(18.dp).clip(CircleShape).border(1.5.dp, if (cull.mode == id) c.ink else c.line, CircleShape), contentAlignment = Alignment.Center) {
                            if (cull.mode == id) Box(Modifier.size(9.dp).clip(CircleShape).background(c.ink))
                        }
                    }
                }
            }
            val livePeople = data.people.filter { it.count > 0 || it.person == cull.personFilter }
            if (data.people.isNotEmpty()) {
                SheetTitle(t("cull.people"), clear = if (cull.personFilter != null) ({ cull.personFilter = null }) else null)
                if (livePeople.isEmpty()) Text(t("cull.noPeopleHere"), style = Type.rowDesc.copy(color = c.ash))
                else {
                    val shown = if (allPeople) livePeople else livePeople.take(10)
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        shown.forEach { p ->
                            val on = cull.personFilter == p.person
                            Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.clip(RoundedCornerShape(12.dp)).clickable { cull.personFilter = if (on) null else p.person }.padding(4.dp)) {
                                Box(Modifier.clip(CircleShape).border(2.dp, if (on) c.ink else Color.Transparent, CircleShape).padding(3.dp)) { FaceAvatar(p.photo, p.box, 48.dp) }
                                Text("${p.count}", style = Type.monoSmall.copy(color = c.ash), modifier = Modifier.padding(top = 3.dp))
                            }
                        }
                    }
                    if (livePeople.size > 10) Text(if (allPeople) t("cull.fewer") else t("cull.morePeople", "count" to livePeople.size), style = Type.rowDesc.copy(color = c.ink, fontWeight = FontWeight.Medium),
                        modifier = Modifier.padding(top = 8.dp).clip(RoundedCornerShape(8.dp)).clickable { allPeople = !allPeople }.padding(vertical = 6.dp))
                }
            }
            val liveScenes = data.scenes.filter { it.count > 0 || it.scene == cull.sceneFilter }
            if (liveScenes.size > 1) {
                SheetTitle(t("cull.scenes"), clear = if (cull.sceneFilter != null) ({ cull.sceneFilter = null }) else null)
                val inPanel = liveScenes.take(6).toMutableList()
                liveScenes.firstOrNull { it.scene == cull.sceneFilter }?.let { if (it !in inPanel) inPanel[inPanel.size - 1] = it }
                inPanel.chunked(2).forEach { row ->
                    Row(Modifier.padding(bottom = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        row.forEach { s -> SceneCard(s, cull.sceneFilter == s.scene, Modifier.weight(1f)) { cull.sceneFilter = if (cull.sceneFilter == s.scene) null else s.scene } }
                        if (row.size == 1) Spacer(Modifier.weight(1f))
                    }
                }
                if (liveScenes.size > inPanel.size) Text(t("cull.allScenes", "count" to liveScenes.size), style = Type.rowDesc.copy(color = c.ink, fontWeight = FontWeight.Medium),
                    modifier = Modifier.clip(RoundedCornerShape(8.dp)).clickable { cull.filterSheet = false; cull.sceneView = true }.padding(vertical = 6.dp))
            }
        }
    }
}

@Composable
private fun SheetTitle(text: String, clear: (() -> Unit)? = null) {
    val c = LocalFg.current
    Row(Modifier.fillMaxWidth().padding(top = 20.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(text.uppercase(), style = Type.overline.copy(color = c.ash), modifier = Modifier.weight(1f))
        if (clear != null) Text(t("cull.clearFilter"), style = Type.rowDesc.copy(color = c.ink, fontWeight = FontWeight.Medium), modifier = Modifier.clip(RoundedCornerShape(8.dp)).clickable(onClick = clear).padding(horizontal = 6.dp, vertical = 2.dp))
    }
}

/** 一个场景：封面、那句描述、张数。 */
@Composable
private fun SceneCard(s: SceneItem, on: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val c = LocalFg.current
    Box(modifier.aspectRatio(1.5f).clip(RoundedCornerShape(12.dp)).border(2.dp, if (on) c.ink else Color.Transparent, RoundedCornerShape(12.dp)).clickable(onClick = onClick)) {
        AsyncImage(File(s.cover.thumb), null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
        Box(Modifier.fillMaxSize().background(androidx.compose.ui.graphics.Brush.verticalGradient(0.45f to Color.Transparent, 1f to Color(0xB3000000))))
        if (s.text.isNotEmpty()) Text(s.text, style = Type.monoSmall.copy(color = Color.White), maxLines = 2, overflow = Ellipsis, modifier = Modifier.align(Alignment.BottomStart).padding(start = 8.dp, end = 34.dp, bottom = 7.dp))
        Text("${s.count}", style = Type.monoSmall.copy(color = Color.White), modifier = Modifier.align(Alignment.BottomEnd).padding(7.dp))
    }
}

/** 浏览方式与排序（桌面端 ViewMenu）。 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ViewSheet(cull: CullState, sortBy: String, onSort: (String) -> Unit) {
    if (!cull.viewSheet) return
    val c = LocalFg.current
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(onDismissRequest = { cull.viewSheet = false }, sheetState = sheet, containerColor = c.canvas) {
        Column(Modifier.navigationBarsPadding().padding(horizontal = 20.dp).padding(bottom = 20.dp)) {
            Text(t("cull.viewMenu"), style = Type.heading.copy(color = c.ink))
            SheetTitle(t("cull.layoutAria"))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                ViewStyle.entries.forEachIndexed { i, v ->
                    val on = cull.layout == v
                    Column(
                        Modifier.weight(1f).riseIn(i).clip(RoundedCornerShape(14.dp)).background(if (on) c.ink else c.panel).border(1.dp, if (on) Color.Transparent else c.line, RoundedCornerShape(14.dp))
                            .clickable { cull.pickLayout(v); cull.viewSheet = false }.padding(vertical = 14.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        FgIcon(v.icon, size = 22.dp, color = if (on) c.onInk else c.ink, stroke = 1.3f)
                        Text(t("cull.layout.${v.id}"), style = Type.rowDesc.copy(color = if (on) c.onInk else c.ink, fontWeight = FontWeight.Medium), modifier = Modifier.padding(top = 6.dp), maxLines = 1)
                    }
                }
            }
            SheetTitle(t("cull.sortAria"))
            Column(Modifier.clip(RoundedCornerShape(14.dp)).background(c.panel).border(1.dp, c.line, RoundedCornerShape(14.dp))) {
                listOf("group", "order", "time", "score").forEachIndexed { i, id ->
                    if (i > 0) Box(Modifier.fillMaxWidth().height(1.dp).background(c.line))
                    Row(Modifier.fillMaxWidth().clickable { onSort(id); cull.viewSheet = false }.padding(horizontal = 16.dp, vertical = 13.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(t("cull.sort.$id"), style = Type.rowTitle.copy(color = c.ink), modifier = Modifier.weight(1f))
                        if (sortBy == id) FgIcon("M3 8.5l3.2 3L13 4.5", size = 16.dp, stroke = 1.8f, color = c.ink)
                    }
                }
            }
        }
    }
}
