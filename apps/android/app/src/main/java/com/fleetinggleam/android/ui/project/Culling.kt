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
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
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
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.node.GlobalPositionAwareModifierNode
import androidx.compose.ui.node.ModifierNodeElement
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.fleetinggleam.android.ui.theme.Motion
import androidx.compose.animation.core.tween
import androidx.compose.animation.togetherWith
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
import com.fleetinggleam.android.ui.components.glassStyle
import com.fleetinggleam.android.ui.components.rememberPress
import com.fleetinggleam.android.ui.components.riseIn
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.intOrNull
import java.io.File
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.animation.core.spring
import com.fleetinggleam.android.ui.components.Ticker
import androidx.compose.animation.core.Animatable
import androidx.compose.ui.layout.positionInParent
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.foundation.gestures.detectHorizontalDragGestures

// ------------------------------------------------------------------------------------------------ 状态

/** 四种浏览方式（桌面端 LAYOUTS），图标是桌面端的同一组形状。 */
enum class ViewStyle(val id: String, val icon: String) {
    Justified("justified", "M1.5 2h6v5h-6zM9 2h5.5v5H9zM1.5 9h4v5h-4zM7 9h7.5v5H7z"),
    Grid("grid", "M1.5 1.5h5v5h-5zM9.5 1.5h5v5h-5zM1.5 9.5h5v5h-5zM9.5 9.5h5v5h-5z"),
    Single("single", "M1.5 2h13v8h-13zM4 13.5h1.5M7.3 13.5h1.5M10.5 13.5H12"),
    Compare("compare", "M1.5 2.5h5.5v11H1.5zM9 2.5h5.5v11H9z");
    companion object { fun of(id: String) = entries.firstOrNull { it.id == id } ?: Justified }
}


/**
 * 一个项目的选片界面的状态（桌面端 Culling.tsx 里那一串 useState）：筛选、选中、大图、搜索、场景页、浏览方式。
 * 换项目时整个换一份。
 */
@Stable
class CullState {
    var mode by mutableStateOf("all")
    var sceneFilter by mutableStateOf<Int?>(null)
    var personFilter by mutableStateOf<Int?>(null)
    /** 手动星级筛选：只看 n 星及以上（null = 不筛）。 */
    var starFilter by mutableStateOf<Int?>(null)
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
    /**
     * 每张照片的格子的布局坐标：打开大图时照片从它的格子里飞出来。只记坐标对象，位置在用到时才算（滚动时每帧不算矩形）。
     * 经 [trackRect] 登记，格子离开组合就注销（以前只增不减，滚过的每个格子都被攥着）。
     */
    val coords = HashMap<String, androidx.compose.ui.layout.LayoutCoordinates>()
    fun rect(path: String): Rect? = coords[path]?.takeIf { it.isAttached }?.boundsInRoot()
    /** 照片墙的缩放档位（网格、瀑布各一个，记住）。 */
    var gridZoom by androidx.compose.runtime.mutableIntStateOf(UiPrefs.int("zoom.grid", ZOOM_GRID_DEFAULT))
    var justifyZoom by androidx.compose.runtime.mutableIntStateOf(UiPrefs.int("zoom.justify", ZOOM_JUSTIFY_DEFAULT))
    fun zoomBy(grid: Boolean, d: Int) {
        if (grid) { gridZoom = (gridZoom + d).coerceIn(0, GRID_COLS.size - 1); UiPrefs.put("zoom.grid", gridZoom) }
        else { justifyZoom = (justifyZoom + d).coerceIn(0, JUSTIFY_HEIGHTS.size - 1); UiPrefs.put("zoom.justify", justifyZoom) }
    }
    /** 对比页的快捷取舍（留下第 i 张、同屏其余标淘汰、跳下一组）和同屏几张：底部浮动操作栏上显示成"留第 1 张 / 留第 2 张"。 */
    var compareKeep by mutableStateOf<((Int) -> Unit)?>(null)
    var compareCount by androidx.compose.runtime.mutableIntStateOf(0)
    /** 状态条上每个页签的坐标：分析中照片"飞进"页签的动效要用。 */
    val tabCoords = HashMap<String, androidx.compose.ui.layout.LayoutCoordinates>()
    fun tabRect(id: String): Rect? = tabCoords[id]?.takeIf { it.isAttached }?.boundsInRoot()

    /** 浮在内容上的玻璃顶栏 + 页签的总高度（像素）：内容从这么高的地方开始，滚动时从它下面过。 */
    var chromeHeight by mutableIntStateOf(0)
    /** 分析中：暗色进度卡滚出视野了（页签下浮出窄胶囊）。 */
    var scanCardGone by mutableStateOf(false)
    /** 分析中：点窄胶囊回到墙顶。 */
    var scanBackToTop: (suspend () -> Unit)? = null
    fun pickLayout(v: ViewStyle) { layout = v; UiPrefs.put("layout", v.id) }
    fun bumpHint(n: Int = 1) { hintUses = if (n == 99) 99 else maxOf(hintUses, hintUses + 1); UiPrefs.put("hint.dock", hintUses) }
    fun select(path: String?, byTap: Boolean = false) { selectedByTap = byTap; selected = path }
    val filtered: Boolean get() = sceneFilter != null || personFilter != null || starFilter != null

    companion object {
        /**
         * 离开项目再回来时记得的那几样（审查第二批：以前一切归零）：页签、场景 / 人物筛选、选中的那张、是不是在场景页。
         * 项目页外面（AppShell）按项目给了一个 SaveableStateProvider，rememberSaveable 才能活过"离开项目"；进程被杀后重开也还在。
         */
        val Saver: androidx.compose.runtime.saveable.Saver<CullState, Any> = androidx.compose.runtime.saveable.mapSaver(
            save = { s ->
                buildMap {
                    put("mode", s.mode); put("sceneView", s.sceneView)
                    s.sceneFilter?.let { put("scene", it) }; s.personFilter?.let { put("person", it) }; s.starFilter?.let { put("stars", it) }; s.selected?.let { put("selected", it) }
                }
            },
            restore = { m ->
                CullState().apply {
                    mode = m["mode"] as? String ?: "all"; sceneView = m["sceneView"] as? Boolean ?: false
                    sceneFilter = m["scene"] as? Int; personFilter = m["person"] as? Int; starFilter = m["stars"] as? Int; selected = m["selected"] as? String
                }
            },
        )
    }
}

/** 把这一块的布局坐标登记进 `map[key]`（[CullState.coords] / [CullState.tabCoords]）；离开组合时注销自己登记的那份。 */
fun Modifier.trackRect(map: MutableMap<String, LayoutCoordinates>, key: String): Modifier = this then TrackRectElement(map, key)

private class TrackRectElement(val map: MutableMap<String, LayoutCoordinates>, val key: String) : ModifierNodeElement<TrackRectNode>() {
    override fun create() = TrackRectNode(map, key)
    override fun update(node: TrackRectNode) {
        if (node.map === map && node.key == key) return
        node.drop()
        node.map = map; node.key = key
    }
    // 按身份比 map（HashMap 的 equals 会逐项比内容）
    override fun equals(other: Any?) = other is TrackRectElement && other.map === map && other.key == key
    override fun hashCode() = System.identityHashCode(map) * 31 + key.hashCode()
}

private class TrackRectNode(var map: MutableMap<String, LayoutCoordinates>, var key: String) : Modifier.Node(), GlobalPositionAwareModifierNode {
    private var mine: LayoutCoordinates? = null
    override fun onGloballyPositioned(coordinates: LayoutCoordinates) { mine = coordinates; map[key] = coordinates }
    override fun onDetach() = drop()
    /** 只删自己登记的：同一张照片可能同时在墙上和单张 / 对比里登记，后来的那份不能被先走的删掉。 */
    fun drop() { if (mine != null && map[key] === mine) map.remove(key); mine = null }
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
    /** `starN[n]`：别的筛选都算上，n 星及以上有几张；`starX[n]`：正好 n 星有几张（面板只列真打过的星数）。下标 1..5。 */
    val starN: IntArray = IntArray(6),
    val starX: IntArray = IntArray(6),
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
fun rememberCullData(cull: CullState, photos: List<Photo>, decisions: Map<String, Decision>, sortBy: String, sceneText: Map<Int, String>, stars: Map<String, Int> = emptyMap()): CullData {
    val mode = cull.mode; val sceneF = cull.sceneFilter; val personF = cull.personFilter; val starF = cull.starFilter; val selected = cull.selected
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
    val facets = remember(photos, decisions, mode, sceneF, personF, starF, stars) {
        fun st(p: Photo) = decisions[p.path]?.state
        val inScene = { p: Photo -> sceneF == null || p.scene == sceneF }
        val inPerson = { p: Photo -> personF == null || p.people.contains(personF) }
        val inStar = { p: Photo -> starF == null || (stars[p.path] ?: 0) >= starF }
        val inMode = { p: Photo -> mode == "all" || st(p) == mode }
        val status = HashMap<String, Int>().apply { put("all", 0); put("keep", 0); put("review", 0); put("reject", 0) }
        val sceneN = HashMap<Int, Int>(); val personN = HashMap<Int, Int>()
        val starN = IntArray(6); val starX = IntArray(6)
        for (p in photos) {
            if (inScene(p) && inPerson(p) && inStar(p)) { status["all"] = status["all"]!! + 1; st(p)?.let { status[it] = (status[it] ?: 0) + 1 } }
            if (inMode(p) && inPerson(p) && inStar(p)) sceneN[p.scene] = (sceneN[p.scene] ?: 0) + 1
            if (inMode(p) && inScene(p) && inStar(p)) p.people.filter { it >= 0 }.toSet().forEach { personN[it] = (personN[it] ?: 0) + 1 }
            if (inMode(p) && inScene(p) && inPerson(p)) { val n = stars[p.path] ?: 0; if (n in 1..5) { starX[n]++; for (k in 1..n) starN[k]++ } }
        }
        Facets(status, sceneN, personN, starN, starX)
    }
    val scenes = remember(scenes0, facets, sceneText) { scenes0.map { (id, cover, total) -> SceneItem(id, cover, total, facets.sceneN[id] ?: 0, sceneText[id] ?: "") } }
    val people = remember(people0, facets) { people0.map { it.copy(count = facets.personN[it.person] ?: 0) } }
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
    val matchesView = { p: Photo -> (mode == "all" || decisions[p.path]?.state == mode) && (sceneF == null || p.scene == sceneF) && (personF == null || p.people.contains(personF)) && (starF == null || (stars[p.path] ?: 0) >= starF) }
    // 选中的那张一直留在眼前，直到你移开——哪怕刚做的改动让它不再符合这个视图。
    // 只有它真的不再符合时才"带着"；其余情况是 null，这样点选一张不会重算整个列表（也就不会重排整面墙）。
    val carried = remember(selected, byPath, decisions, mode, sceneF, personF, starF, stars) { selected?.let { byPath[it] }?.takeIf { !matchesView(it) }?.path }
    val shownRaw = remember(order, decisions, mode, sceneF, personF, starF, stars, carried, hits) {
        val base = order.filter { matchesView(it) || it.path == carried }
        if (hits == null) base else {
            val rank = hits.withIndex().associate { (i, h) -> h.path to i }
            base.filter { it.path in rank }.sortedBy { rank[it.path] }
        }
    }
    // 同样的照片、同样的顺序就是同一个列表：改判会换一张取舍表，但视图里的照片没变时，下游（排版、每一格的位置）都不重做
    val shownRef = remember { mutableStateOf(shownRaw) }
    if (shownRef.value !== shownRaw && (shownRef.value.size != shownRaw.size || shownRef.value.indices.any { shownRef.value[it] !== shownRaw[it] })) shownRef.value = shownRaw
    val shown = shownRef.value
    return remember(shown, facets, scenes, people, byPath, hits) { CullData(shown, facets.status, scenes, people, byPath, hits, facets.starN, facets.starX) }
}

/** 筛选面板里每一栏的计数（见 rememberCullData）。 */
private class Facets(val status: Map<String, Int>, val sceneN: Map<Int, Int>, val personN: Map<Int, Int>, val starN: IntArray, val starX: IntArray)

/** 一条理由的文字（桌面端 reasonText）：参数原样带进去，`stateKey` 先翻译成状态的名字。 */
fun reasonText(r: Reason): String {
    val params = r.p?.entries?.map { (k, v) ->
        val prim = v as? JsonPrimitive
        k to (prim?.let { p -> p.intOrNull ?: p.doubleOrNull ?: p.contentOrNull })
    } ?: emptyList()
    val state = r.p?.str("stateKey")?.let { t(it) }
    return t(r.k, *(params + ("state" to state)).toTypedArray())
}

/**
 * 改判，并处理筛选视图里的细节（桌面端 decide）：改了之后不再属于这个视图的那张，选中挪到它的邻居上。
 * `order` 是眼前这一串照片的顺序：大图里传它自己打开时定下的那一串，邻居就在翻页器里找（往后找第一张还在这个页签里的，
 * 没有再往前找），不会选中一张翻页器里没有的照片——以前从实时列表找，划回去时选中和翻页对不上，来回跳着闪。
 */
fun decideIn(cull: CullState, data: CullData, path: String, st: String?, order: List<Photo> = data.shown) {
    Session.decide(path, st)
    cull.bumpHint()
    if (st != null && cull.mode != "all" && st != cull.mode) {
        val i = order.indexOfFirst { it.path == path }
        val inView = data.shown.mapTo(HashSet()) { it.path }
        val ok = { p: Photo -> p.path != path && p.path in inView }
        val nb = order.drop(i + 1).firstOrNull(ok) ?: order.take(i.coerceAtLeast(0)).lastOrNull(ok)
            ?: order.getOrNull(i + 1) ?: order.getOrNull(i - 1)
        cull.select(nb?.path?.takeIf { it != path })
    }
}

// ------------------------------------------------------------------------------------------------ 选片界面

/** 选片界面上的动作（改判、打开大图、点选格子）：内容层和浮层共用一份，交给格子的回调不随重组换新对象。 */
class CullActions(val decide: (String, String?) -> Unit, val open: (String) -> Unit, val selectTile: (String) -> Unit)

@Composable
fun rememberCullActions(cull: CullState, data: CullData): CullActions {
    val haptic = LocalHapticFeedback.current
    val dataRef = androidx.compose.runtime.rememberUpdatedState(data)
    return remember(cull) {
        val decide: (String, String?) -> Unit = { path, st -> haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove); decideIn(cull, dataRef.value, path, st) }
        val open: (String) -> Unit = { path -> cull.select(path, byTap = true); cull.viewer = true }
        // 格子单击直接进大图，跟手机相册一样（作者 2026-10-26："点一下就放到大图就行了 不用双击"）
        val selectTile: (String) -> Unit = open
        CullActions(decide, open, selectTile)
    }
}

/**
 * 分析好的项目的内容层（录进项目页的 backdrop，浮层从上面折射它）：照片墙 / 单张 / 对比 / 场景页。
 * 状态页签在项目页的玻璃顶栏里（ProjectScreen），底部的玻璃工具栏在 `CullingChrome`。
 * 内容顶部留出 `cull.chromeHeight`，底部留出底栏的高度。
 */
@Composable
fun CullingContent(project: Project, cull: CullState, data: CullData, actions: CullActions, topPad: Dp) {
    val photos by Session.photos.collectAsState()
    val decisions by Session.decisions.collectAsState()
    val celebrate by Session.celebrate.collectAsState()
    val describing by Scenes.describing.collectAsState()
    val packs by Models.packs.collectAsState()
    val scope = rememberCoroutineScope()
    val sortBy = project.sort ?: "group"
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

    androidx.activity.compose.BackHandler(cull.sceneView) { cull.sceneView = false }
    androidx.activity.compose.BackHandler(!cull.sceneView && cull.searchOpen) { cull.searchOpen = false; cull.query = "" }
    // 操作提示在时，墙底部多留 56dp，免得和最后一行照片重叠（设计审查 A6.9）
    val hintOn = cull.hintUses < 4 && !cull.sceneView
    val bottomPad = if (hintOn) 148.dp else 92.dp
    when {
        cull.sceneView -> SceneGallery(project, data, cull, describing, canDescribe = !scenePack && data.scenes.any { it.text.isEmpty() },
            onDescribe = { scope.launch { if (Models.ensurePack("scenetext") != Answer.Cancelled) Scenes.describe(photos, true) } }, topPad = topPad, bottomPad = bottomPad)
        cull.layout == ViewStyle.Single && data.shown.isNotEmpty() -> Box(Modifier.fillMaxSize().padding(top = topPad)) {
            SinglePager(data.shown, decisions, cull, onDecide = actions.decide, onOpen = actions.open, bottomPad = bottomPad)
        }
        cull.layout == ViewStyle.Compare && data.shown.isNotEmpty() -> Box(Modifier.fillMaxSize().padding(top = topPad)) {
            // 人物筛选下的照片池：只在照片或筛选变了时重算（以前每次重组都 filter 一遍，CompareView 里的 groupBy 跟着重算）
            val pool = remember(photos, cull.personFilter) { cull.personFilter?.let { who -> photos.filter { it.people.contains(who) } } ?: photos }
            CompareView(data.shown, pool, if (cull.sceneFilter != null) "scene" else "group",
                cull, decisions, onDecide = actions.decide, onOpen = actions.open, bottomPad = bottomPad)
        }
        else -> {
            // 页签固定在上面的玻璃里（米家那种）；墙上左右滑切换页签，内容朝滑的方向滑入
            val items = remember(data.shown) { data.shown.map { WallItem(it.path, it.ratio) } }
            // 选中换了（不是点出来的：撤销、全局搜索、单张里换的）：把它滚到眼前
            val reveal = cull.selected.takeIf { !cull.selectedByTap }
            androidx.compose.animation.AnimatedContent(
                targetState = cull.mode, label = "mode", transitionSpec = modeTransition,
                modifier = Modifier.fillMaxSize().swipeModes(cull),
            ) { _ ->
                val state = rememberLazyListState()
                Wall(items, if (cull.layout == ViewStyle.Grid) "grid" else "justified", state, PaddingValues(start = 16.dp, end = 16.dp, top = topPad + 4.dp, bottom = bottomPad),
                    header = { if (data.shown.isEmpty()) EmptyView(cull, data) }, reveal = reveal, thumbOf = { data.shown.getOrNull(it)?.thumb },
                    zoom = if (cull.layout == ViewStyle.Grid) cull.gridZoom else cull.justifyZoom, onZoom = { d -> cull.zoomBy(cull.layout == ViewStyle.Grid, d) }) { i, w, h ->
                    val p = data.shown[i]
                    val d = decisions[p.path]
                    val st = d?.state ?: ""
                    Box(Modifier.trackRect(cull.coords, p.path)) {
                        Tile(p.path, w, h, p, null, st, selected = cull.selected == p.path, reveal = celebrate, dim = st == "reject" && cull.mode == "all", manual = d?.manual == true,
                            showScore = sortBy == "score", index = i, interactive = true, dock = false, stars = project.stars[p.path] ?: 0,
                            onSelect = actions.selectTile, onOpen = actions.open, onDecide = actions.decide)
                    }
                }
            }
        }
    }
}

/** 分析好的项目的浮层（在 backdrop 之外，玻璃）：操作提示、底部工具栏，以及筛选 / 浏览方式弹层。 */
@Composable
fun CullingChrome(project: Project, cull: CullState, data: CullData, actions: CullActions) {
    val c = LocalFg.current
    val decisions by Session.decisions.collectAsState()
    val sortBy = project.sort ?: "group"
    val hintOn = cull.hintUses < 4 && !cull.sceneView
    Box(Modifier.fillMaxSize()) {
        // 操作提示：前几次改判之后就不再出现
        AnimatedVisibility(hintOn, modifier = Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(bottom = 74.dp, start = 16.dp, end = 16.dp),
            enter = fadeIn(Motion.base()) + slideInVertically(Motion.base()) { it / 2 }, exit = fadeOut(Motion.fast())) {
            GlassBox(shape = RoundedCornerShape(14.dp), style = glassStyle(strong = true)) {
                Row(Modifier.height(44.dp).padding(start = 16.dp, end = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(t("android.cull.hint"), style = Type.rowDesc.copy(color = c.ink), modifier = Modifier.weight(1f, fill = false), maxLines = 2)
                    Text(t("cull.hintDismiss"), style = Type.label.copy(color = c.ink, fontWeight = FontWeight.SemiBold), modifier = Modifier.clip(RoundedCornerShape(10.dp)).clickable { cull.bumpHint(99) }.padding(horizontal = 12.dp, vertical = 8.dp))
                }
            }
        }
        // 底栏：没选照片时只有浏览方式；选中一张就变成它的快速操作（保留 / 淘汰）。场景页有自己的导航，底栏隐藏（A11.3）
        val sel = cull.selected?.let { data.byPath[it] }?.takeIf { cull.layout != ViewStyle.Compare }
        if (!cull.sceneView) CullBar(cull, sel, sel?.let { decisions[it.path] }, Modifier.align(Alignment.BottomCenter), onDecide = actions.decide, onOpen = actions.open)
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

/** 页签的顺序（左右滑动按它走）。 */
internal val MODES = listOf("all", "keep", "review", "reject")

/** 页签切换：内容朝滑的方向滑入（分析中和分析完同一套）。 */
internal val modeTransition: androidx.compose.animation.AnimatedContentTransitionScope<String>.() -> androidx.compose.animation.ContentTransform = {
    val dir = if (MODES.indexOf(targetState) >= MODES.indexOf(initialState)) 1 else -1
    (androidx.compose.animation.slideInHorizontally(Motion.base()) { it / 5 * dir } + fadeIn(Motion.base()))
        .togetherWith(androidx.compose.animation.slideOutHorizontally(Motion.base()) { -it / 5 * dir } + fadeOut(Motion.fast()))
        .using(androidx.compose.animation.SizeTransform(clip = false))
}

/** 墙上左右滑切换页签（米家那种）。 */
@Composable
internal fun Modifier.swipeModes(cull: CullState): Modifier {
    val haptic = LocalHapticFeedback.current
    val dragPx = with(LocalDensity.current) { 64.dp.toPx() }
    return pointerInput(Unit) {
        var dx = 0f
        detectHorizontalDragGestures(
            onDragStart = { dx = 0f },
            onDragEnd = {
                if (kotlin.math.abs(dx) > dragPx) {
                    val n = MODES.indexOf(cull.mode) + if (dx < 0) 1 else -1
                    if (n in MODES.indices) { haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove); cull.mode = MODES[n] }
                }
            },
        ) { ch, d -> dx += d; ch.consume() }
    }
}

/**
 * 状态页签（米家首页那种文字页签）：全部 / 保留 / 待复查 / 淘汰，选中的黑、其余灰，下面一道会滑的短线；每个后面一个小数字是"现在点它墙上会有几张"。
 * 右端 ≡ 打开人物与场景筛选（有筛选时带个点）；正在用的人物 / 场景筛选显示在下面一行，点 × 去掉。固定在照片墙上方，不随墙滚走。
 */
@Composable
fun StatusStrip(cull: CullState, data: CullData, padded: Boolean = true) {
    val c = LocalFg.current
    val density = LocalDensity.current
    // 每个页签文字在行里的位置和宽度（不含后面的数字），给下划线用
    val spans = remember { mutableStateMapOf<String, Pair<Float, Float>>() }
    val target = spans[cull.mode]
    val lineX by animateFloatAsState(target?.first ?: 0f, Motion.glide(), label = "tabX")
    val lineW by animateFloatAsState(target?.second ?: 0f, Motion.glide(), label = "tabW")
    val hasFilters = data.people.isNotEmpty() || data.scenes.size > 1 || data.starN[1] > 0 || cull.starFilter != null
    val filtering = cull.personFilter != null || cull.sceneFilter != null || cull.starFilter != null
    Column(Modifier.fillMaxWidth().padding(horizontal = if (padded) 16.dp else 4.dp)) {
        Box(Modifier.fillMaxWidth()) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Row(Modifier.weight(1f).horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically) {
                    MODES.forEachIndexed { i, id ->
                        val on = cull.mode == id
                        var rowX by remember { mutableStateOf(0f) }
                        Row(
                            Modifier.padding(end = 18.dp).onGloballyPositioned { rowX = it.positionInParent().x }
                                .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) { cull.mode = id }
                                .padding(top = 8.dp, bottom = 12.dp),
                            verticalAlignment = Alignment.Bottom,
                        ) {
                            // 15sp：选中半粗墨色、未选常规灰（设计审查 A6.2）；下划线只有文字那么宽
                            Text(t("state.$id"), style = Type.tab.copy(color = if (on) c.ink else c.ash, fontWeight = if (on) FontWeight.SemiBold else FontWeight.Normal),
                                modifier = Modifier.onGloballyPositioned { spans[id] = (rowX + it.positionInParent().x) to it.size.width.toFloat() })
                            // 数字像计数器一样一位位滚（分析中会一直变）；颜色跟随页签。分析中的光点飞到它的左侧
                            Box(Modifier.padding(start = 4.dp, bottom = 2.dp).trackRect(cull.tabCoords, id)) { Ticker(data.status[id] ?: 0, Type.monoSmall.copy(color = if (on) c.ink else c.ash)) }
                        }
                    }
                }
                if (hasFilters) Box(Modifier.size(40.dp).clip(CircleShape).clickable { cull.filterSheet = true }, contentAlignment = Alignment.Center) {
                    FgIcon("M2 4h12M4 8h8M6 12h4", size = 18.dp, stroke = 1.6f, color = c.ink)
                    // "有筛选"的点是墨色：琥珀只给保留（R1 / A6.3）
                    if (filtering) Box(Modifier.align(Alignment.TopEnd).padding(top = 8.dp, end = 7.dp).size(6.dp).clip(CircleShape).background(c.ink))
                }
            }
            // 下划线：跟着选中的页签滑过去
            if (target != null) Box(
                Modifier.align(Alignment.BottomStart).offset { androidx.compose.ui.unit.IntOffset(lineX.toInt(), 0) }
                    .width(with(density) { lineW.toDp() }).height(2.dp).clip(RoundedCornerShape(1.dp)).background(c.ink),
            )
        }
        // 正在用的人物 / 场景筛选
        if (filtering) Row(Modifier.padding(bottom = 8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
            cull.personFilter?.let { id ->
                data.people.firstOrNull { it.person == id }?.let { p ->
                    FilterChip(onClear = { cull.personFilter = null }) { FaceAvatar(p.photo, p.box, 22.dp); Text(t("cull.people"), style = Type.rowDesc.copy(color = c.ink), modifier = Modifier.padding(start = 6.dp)) }
                }
            }
            cull.starFilter?.let { n ->
                FilterChip(onClear = { cull.starFilter = null }) { Text(t("stars.atLeast", "count" to n), style = Type.rowDesc.copy(color = c.ink), modifier = Modifier.padding(start = 5.dp)) }
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
private fun CullBar(cull: CullState, sel: Photo?, d: Decision?, modifier: Modifier, onDecide: (String, String?) -> Unit, onOpen: (String) -> Unit) {
    val c = LocalFg.current
    Box(modifier.navigationBarsPadding().padding(bottom = 10.dp)) {
        GlassBox(shape = RoundedCornerShape(50)) {
            // 内容切换时胶囊的宽度跟着变
            // 只有"有没有选中照片"和"是不是场景页"变了才换内容；改判、换一张选中的都在原地更新（之前把判定也当键，点一下保留整条会闪一下）
            androidx.compose.animation.AnimatedContent(
                targetState = Pair(cull.sceneView, sel != null), label = "bar",
                transitionSpec = { (androidx.compose.animation.fadeIn(Motion.fast()) togetherWith androidx.compose.animation.fadeOut(Motion.fast())).using(androidx.compose.animation.SizeTransform(clip = false) { _, _ -> Motion.glide() }) },
            ) { (scene, hasSel) ->
                val path = if (hasSel) sel?.path else null
                val state = d?.state
                Row(Modifier.padding(4.dp), verticalAlignment = Alignment.CenterVertically) {
                    when {
                        scene -> BarItem("M10 3L5 8l5 5", t("cull.backToPhotos")) { cull.sceneView = false }
                        path != null -> {
                            // 没有"放大"项：再点一次选中的格子、或长按，就是放大（设计审查 A6.7）
                            val manual = d?.manual == true
                            BarItem("star", t(if (state == "keep" && manual) "mark.clear" else "state.keep"), on = state == "keep", gold = true) { onDecide(path, if (state == "keep" && manual) null else "keep") }
                            BarItem("M3 3l6 6M9 3L3 9", t(if (state == "reject" && manual) "mark.clear" else "state.reject"), on = state == "reject", box = 12f) { onDecide(path, if (state == "reject" && manual) null else "reject") }
                            Box(Modifier.width(1.dp).height(24.dp).background(c.line))
                            BarItem(cull.layout.icon, t("cull.layout.${cull.layout.id}")) { cull.viewSheet = true }
                        }
                        else -> {
                            // 对比页：快捷取舍放在浮动栏里、浏览方式左边（作者 2026-10-23：这里还宽，内容区就不用再占一行）
                            val keep = cull.compareKeep
                            if (cull.layout == ViewStyle.Compare && keep != null && cull.compareCount > 0) {
                                for (i in 0 until cull.compareCount) BarItem("star", t("android.cmp.keepNth", "n" to i + 1), width = 76.dp) { keep(i) }
                                Box(Modifier.width(1.dp).height(24.dp).background(c.line))
                            }
                            BarItem(cull.layout.icon, t("cull.layout.${cull.layout.id}")) { cull.viewSheet = true }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun BarItem(icon: String, label: String, enabled: Boolean = true, dot: Boolean = false, on: Boolean = false, gold: Boolean = false, box: Float = 16f, width: androidx.compose.ui.unit.Dp = 60.dp, onClick: () -> Unit) {
    val c = LocalFg.current
    val src = remember { MutableInteractionSource() }
    val press = rememberPress(src)
    // 选中态：保留是琥珀色（只有"保留"用它），淘汰是墨色；颜色平滑过渡，不闪
    val bg by androidx.compose.animation.animateColorAsState(if (!on) Color.Transparent else if (gold) c.gleam else c.ink, Motion.fast(), label = "barBg")
    val fg by androidx.compose.animation.animateColorAsState(if (!on) c.ink else if (gold) c.gleamInk else c.onInk, Motion.fast(), label = "barFg")
    Column(
        Modifier.width(width).height(46.dp).clip(RoundedCornerShape(23.dp)).background(bg)
            .graphicsLayer { val s = 1f + 0.08f * press(); scaleX = s; scaleY = s; alpha = if (enabled) 1f else 0.35f }
            .clickable(interactionSource = src, indication = null, enabled = enabled, onClick = onClick),
        horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center,
    ) {
        Box {
            if (icon == "star") Star(true, 18.dp, color = if (on) fg else c.gleam, glow = false) else FgIcon(icon, size = 18.dp, box = box, color = fg)
            if (dot) Box(Modifier.align(Alignment.TopEnd).offset(4.dp, (-2).dp).size(7.dp).clip(CircleShape).background(c.ink))
        }
        Text(label, style = Type.caption.copy(color = fg), maxLines = 1, overflow = Ellipsis, modifier = Modifier.padding(top = 1.dp))
    }
}

// ------------------------------------------------------------------------------------------------ 筛选与浏览方式

/** 筛选（桌面端 FilterPanel 的手机版，放在底部弹出的面板里）：状态、人物、场景，三者叠加。 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun FilterSheet(cull: CullState, data: CullData) {
    if (!cull.filterSheet) return
    val c = LocalFg.current
    var allPeople by remember { mutableStateOf(false) }
    com.fleetinggleam.android.ui.components.GlassSheet(true, { cull.filterSheet = false }) {
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(horizontal = 16.dp).padding(bottom = 20.dp)) {
            Text(t("cull.filters"), style = Type.navTitle.copy(color = c.ink))
            SheetTitle(t("cull.status"))
            Column(Modifier.clip(RoundedCornerShape(14.dp)).background(c.panel)) {
                listOf("all", "keep", "review", "reject").forEachIndexed { i, id ->
                    if (i > 0) Box(Modifier.padding(start = 16.dp).fillMaxWidth().height(0.5.dp).background(c.line))
                    Row(Modifier.fillMaxWidth().clickable { cull.mode = id }.heightIn(min = 52.dp).padding(horizontal = 16.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.width(20.dp), contentAlignment = Alignment.Center) { StateIcon(id, c.ink, 14.dp) }
                        Text(t("state.$id"), style = Type.rowTitle.copy(color = c.ink), modifier = Modifier.weight(1f).padding(start = 10.dp))
                        Text("${data.status[id] ?: 0}", style = Type.mono.copy(color = c.ash))
                        com.fleetinggleam.android.ui.components.FgRadio(cull.mode == id, Modifier.padding(start = 12.dp))
                    }
                }
            }
            // 手动星级：n 星及以上，只列真打过的星数；没打过星的项目不显示这一栏
            if (data.starN[1] > 0 || cull.starFilter != null) {
                SheetTitle(t("stars.label"), clear = if (cull.starFilter != null) ({ cull.starFilter = null }) else null)
                Column(Modifier.clip(RoundedCornerShape(14.dp)).background(c.panel)) {
                    (5 downTo 1).filter { data.starX[it] > 0 || cull.starFilter == it }.forEachIndexed { i, n ->
                        if (i > 0) Box(Modifier.padding(start = 16.dp).fillMaxWidth().height(0.5.dp).background(c.line))
                        val on = cull.starFilter == n
                        Row(Modifier.fillMaxWidth().clickable { cull.starFilter = if (on) null else n }.heightIn(min = 52.dp).padding(horizontal = 16.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text("★".repeat(n), style = Type.rowDesc.copy(color = c.ink), modifier = Modifier.width(76.dp))
                            Text(t("stars.atLeast", "count" to n), style = Type.rowTitle.copy(color = c.ink), modifier = Modifier.weight(1f))
                            Text("${data.starN[n]}", style = Type.mono.copy(color = c.ash))
                            com.fleetinggleam.android.ui.components.FgRadio(on, Modifier.padding(start = 12.dp))
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
                                Box(Modifier.clip(CircleShape).border(2.dp, if (on) c.ink else Color.Transparent, CircleShape).padding(2.dp).clip(CircleShape).background(c.canvas).padding(2.dp)) { FaceAvatar(p.photo, p.box, 44.dp) }
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
        Text(text, style = Type.overline.copy(color = c.ash), modifier = Modifier.weight(1f))
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
    com.fleetinggleam.android.ui.components.GlassSheet(true, { cull.viewSheet = false }) {
        Column(Modifier.navigationBarsPadding().padding(horizontal = 16.dp).padding(bottom = 20.dp)) {
            Text(t("cull.viewMenu"), style = Type.navTitle.copy(color = c.ink))
            SheetTitle(t("cull.layoutAria"))
            // 弹层里不做入场（R6）；选中 = surface2 底 + 1.5dp 墨色描边，不是整块实心（A6.13）
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                ViewStyle.entries.forEach { v ->
                    val on = cull.layout == v
                    Column(
                        Modifier.weight(1f).clip(RoundedCornerShape(12.dp)).background(if (on) c.surface2 else c.panel).border(1.5.dp, if (on) c.ink else Color.Transparent, RoundedCornerShape(12.dp))
                            .clickable { cull.pickLayout(v); cull.viewSheet = false }.padding(vertical = 14.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        FgIcon(v.icon, size = 22.dp, color = c.ink, stroke = 1.3f)
                        Text(t("cull.layout.${v.id}"), style = Type.rowDesc.copy(color = c.ink, fontWeight = if (on) FontWeight.SemiBold else FontWeight.Medium), modifier = Modifier.padding(top = 6.dp), maxLines = 1)
                    }
                }
            }
            SheetTitle(t("cull.sortAria"))
            Column(Modifier.clip(RoundedCornerShape(14.dp)).background(c.panel)) {
                listOf("group", "order", "time", "score").forEachIndexed { i, id ->
                    if (i > 0) Box(Modifier.padding(start = 16.dp).fillMaxWidth().height(0.5.dp).background(c.line))
                    Row(Modifier.fillMaxWidth().clickable { onSort(id); cull.viewSheet = false }.heightIn(min = 52.dp).padding(horizontal = 16.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(t("cull.sort.$id"), style = Type.rowTitle.copy(color = c.ink), modifier = Modifier.weight(1f))
                        if (sortBy == id) FgIcon("M3 8.5l3.2 3L13 4.5", size = 16.dp, stroke = 1.8f, color = c.ink)
                    }
                }
            }
        }
    }
}
