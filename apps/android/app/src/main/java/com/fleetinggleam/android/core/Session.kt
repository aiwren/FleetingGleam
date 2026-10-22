package com.fleetinggleam.android.core

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.MapSerializer
import kotlinx.serialization.builtins.serializer
import java.util.concurrent.ConcurrentHashMap

/** 一个项目在界面上的阶段（和桌面端 Culling.tsx 的 Phase 一样）。 */
enum class Phase { Idle, Probing, Ready, Scanning, Loading, Done }

/** 分析中一张照片的样子：走到哪一步，缩略图已经有了没有。 */
data class StageInfo(val stage: String, val pct: Int, val thumb: String?, val width: Int?, val height: Int?)

/** 分析为什么停了（手机才有：太热、电量太低），停了之后条件好转会自己接着分析。 */
enum class Pause { None, Hot, Battery }

/**
 * 打开着的那个项目（桌面端 Culling.tsx 里不画界面的那一半，原样搬过来）：
 * 进项目先连上它所在的 NAS、同步 NAS / 系统文件夹的镜像、探测（尺寸与哪些已经分析过），然后要么等用户点"开始分析"，要么直接从缓存打开；
 * 分析中收引擎的事件，一帧画一次；分析完问引擎取舍；手动改判先在本地改好，必要时再问引擎。整个进程一个，换项目就换一个。
 */
object Session {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private lateinit var app: Context

    val phase = MutableStateFlow(Phase.Idle)
    val files = MutableStateFlow<List<ProbeFile>>(emptyList())
    val bytes = MutableStateFlow(0L)
    val uncached = MutableStateFlow(0)
    val photos = MutableStateFlow<List<Photo>>(emptyList())
    val progress = MutableStateFlow(Progress())
    /** 出错时的文案键或消息（"cull.emptyFolder"、"cull.nasOffline"……）。 */
    val error = MutableStateFlow<String?>(null)
    val nasOff = MutableStateFlow("")
    val etaSeconds = MutableStateFlow<Int?>(null)
    /** 开始分析前给确认页用的快速缩略图。 */
    val previews = MutableStateFlow<Map<String, String>>(emptyMap())
    val decisions = MutableStateFlow<Map<String, Decision>>(emptyMap())
    val celebrate = MutableStateFlow(false)
    /** 学到的喜好变了（这个项目里改判过、或在别处学过）：工具栏上出现"重新评估"。 */
    val dirty = MutableStateFlow(0)
    val nasProg = MutableStateFlow<NasProgress?>(null)
    val peopleProg = MutableStateFlow<Progress?>(null)
    val mediaProg = MutableStateFlow<Progress?>(null)
    /** 读取文件夹（取每张的尺寸）的进度。 */
    val probeProg = MutableStateFlow<Progress?>(null)
    val paused = MutableStateFlow(Pause.None)
    /** 还能撤销几步（工具栏的撤销按钮跟着亮、灭）。 */
    val undoCount = MutableStateFlow(0)
    /** 分析中的实时数据：每帧最多刷新一次（`tick` 加一），几千个事件也不卡。 */
    val tick = MutableStateFlow(0)
    val results = ConcurrentHashMap<String, Photo>()
    val stages = ConcurrentHashMap<String, StageInfo>()
    @Volatile var lastStage: Pair<String, String>? = null; private set
    @Volatile var startedAt = 0L; private set

    @Volatile private var token = 0
    private var job: Job? = null
    private var decideJob: Job? = null
    private var currentKey = ""
    /** 手动改判的撤销栈（最近 50 次），换项目清空。 */
    private val undoStack = ArrayDeque<Pair<String, String?>>()
    /** 一次只改一张、而且没有上限和"每组一张"时，本地直接改，不再问引擎（见 `decide`）。 */
    private var localPatch: Triple<String, String, List<Photo>>? = null

    fun init(ctx: Context) {
        app = ctx.applicationContext
        listen()
        // 打开的项目、它的文件夹、选定的模型、清空缓存：任何一个变了都重新进项目
        scope.launch {
            Projects.store.map { s -> s.projects.firstOrNull { it.id == s.activeId && it.trashed == null }?.let { "${it.id}|${it.folders.joinToString("|")}" } ?: "" }
                .distinctUntilChanged().collect { enter() }
        }
        scope.launch { Settings.state.map { it.tierRev }.distinctUntilChanged().collect { if (currentKey.isNotEmpty()) enter() } }
        scope.launch { Settings.cacheRev.collect { if (it > 0 && currentKey.isNotEmpty()) enter() } }
        // 取舍跟着照片、保留上限、选片方式、"每组一张"和手动改判走
        scope.launch {
            Projects.store.map { s -> current()?.let { listOf(it.id, it.keepCap, it.pickMode, it.onePerGroup, it.overrides) } }
                .distinctUntilChanged().collect { redecide() }
        }
        scope.launch { photos.collect { redecide() } }
        // 在别处学了喜好（从文件夹学、别的风格）：提示可以重新评估
        scope.launch { var seen = Styles.revision.value; Styles.revision.collect { if (it != seen) { seen = it; dirty.value += 1 } } }
    }

    fun current(): Project? = Projects.store.value.let { s -> s.projects.firstOrNull { it.id == s.activeId && it.trashed == null } }

    // 分析中的事件一秒几十个；界面每帧最多刷新一次（攒 32 毫秒再发一个 tick），否则每张照片到达都重组整面墙
    @Volatile private var flushPending = false
    private fun flush() {
        if (flushPending) return
        flushPending = true
        scope.launch { delay(32); flushPending = false; tick.value += 1 }
    }

    private fun listen() {
        Core.listen("scan-progress") { raw -> if (phase.value == Phase.Scanning) runCatching { progress.value = Fg.json.decodeFromString<Progress>(raw) } }
        Core.listen("scan-info") { raw -> runCatching { Settings.setScanInfo(Fg.json.decodeFromString(raw)) } }
        Core.listen("scan-stage") { raw ->
            if (phase.value != Phase.Scanning) return@listen
            val p = runCatching { Fg.json.decodeFromString<StageEvent>(raw) }.getOrNull() ?: return@listen
            val old = stages[p.path]
            stages[p.path] = StageInfo(p.stage, p.pct, p.thumb ?: old?.thumb, p.width ?: old?.width, p.height ?: old?.height)
            lastStage = p.path to p.stage
            flush()
        }
        Core.listen("scan-photo") { raw ->
            if (phase.value != Phase.Scanning) return@listen
            val p = runCatching { Fg.json.decodeFromString<Photo>(raw) }.getOrNull() ?: return@listen
            results[p.path] = p
            stages.remove(p.path)
            flush()
        }
        // the photos that were already analysed arrive in batches
        Core.listen("scan-photos") { raw ->
            if (phase.value != Phase.Scanning) return@listen
            val list = runCatching { Fg.json.decodeFromString(ListSerializer(Photo.serializer()), raw) }.getOrNull() ?: return@listen
            for (p in list) { results[p.path] = p; stages.remove(p.path) }
            flush()
        }
        Core.listen("people-progress") { raw -> runCatching { peopleProg.value = Fg.json.decodeFromString<Progress>(raw) } }
        Core.listen("media-progress") { raw -> runCatching { mediaProg.value = Fg.json.decodeFromString<Progress>(raw) } }
        Core.listen("nas-progress") { raw -> runCatching { val p = Fg.json.decodeFromString<NasProgress>(raw); nasProg.value = if (p.done >= p.total) null else p } }
        Core.listen("probe-progress") { raw -> runCatching { val p = Fg.json.decodeFromString<Progress>(raw); probeProg.value = if (p.done >= p.total) null else p } }
    }

    /** Entering a project (or changing its folders): read sizes first, then either wait for the go-ahead (new photos) or open instantly from the cache. */
    fun enter() {
        val tok = ++token
        // whatever scan was running belonged to the project (or settings) just left; it stops at its next photo
        runCatching { Core.callBlocking("scan_cancel") }
        Scans.stopped()
        undoStack.clear(); undoCount.value = 0
        error.value = null; celebrate.value = false; dirty.value = 0; nasProg.value = null; peopleProg.value = null; mediaProg.value = null; paused.value = Pause.None
        decisions.value = emptyMap()
        val project = current()
        currentKey = project?.let { "${it.id}|${it.folders.joinToString("|")}" } ?: ""
        if (project == null || project.folders.isEmpty()) { phase.value = Phase.Idle; photos.value = emptyList(); files.value = emptyList(); return }
        probeProg.value = null
        phase.value = Phase.Probing
        job?.cancel()
        job = scope.launch {
            try {
                // a project on a network share: connect the share first, and say so when it cannot be reached
                val shares = runCatching { Core.get<List<Share>>("nas_ensure", args("dirs" to project.folders)) }.getOrDefault(emptyList())
                if (tok != token) return@launch
                val disabled = shares.filter { !it.enabled }
                if (disabled.isNotEmpty()) { photos.value = emptyList(); files.value = emptyList(); phase.value = Phase.Idle; nasOff.value = disabled.joinToString(", ") { it.name }; error.value = "cull.nasDisabled"; return@launch }
                val off = shares.filter { !it.connected }
                if (off.isNotEmpty()) { photos.value = emptyList(); files.value = emptyList(); phase.value = Phase.Idle; nasOff.value = off.joinToString(", ") { it.name }; error.value = "cull.nasOffline"; return@launch }
                // a folder picked through the system picker that has no path: bring its new photos into the mirror first
                Media.syncMirrors(app, project.folders, { done, total, name -> nasProg.value = if (done >= total) null else NasProgress(name, done, total) }, { tok != token })
                nasProg.value = null
                val probe = Core.get<Probe>("probe_folder", args("dirs" to project.folders, "compute" to "auto", "quality" to Strategy.currentTier()))
                if (tok != token) return@launch
                files.value = probe.files; bytes.value = probe.bytes; uncached.value = probe.uncached
                if (probe.files.isEmpty()) { photos.value = emptyList(); phase.value = Phase.Idle; error.value = "cull.emptyFolder"; return@launch }
                if (project.count != probe.files.size) Projects.update(project.id) { it.copy(count = probe.files.size) }
                if (probe.uncached > 0) {
                    phase.value = Phase.Ready
                    prepareReady(tok)
                } else runScan(project, probe.files.size, quiet = true, tok = tok)
            } catch (e: Exception) {
                if (tok == token && e.message != "cancelled") { error.value = e.message; phase.value = Phase.Idle }
            }
        }
    }

    /** 确认页：预计要多久（应用会用的模型、这台手机的核数），和前几张照片的快速缩略图。 */
    private suspend fun prepareReady(tok: Int) {
        runCatching {
            val tier = Strategy.predictTier()
            val m = Strategy.machine()
            if (tok == token) etaSeconds.value = Strategy.estimateSeconds(tier, uncached.value, m.cores)
        }
        val paths = files.value.filter { !it.cached }.take(8).map { it.path }
        if (paths.isEmpty()) return
        runCatching {
            val list = Core.get<List<String?>>("preview_thumbs", args("paths" to paths))
            if (tok == token) previews.value = previews.value + paths.zip(list).mapNotNull { (p, th) -> th?.let { p to it } }
        }
    }

    private suspend fun runScan(project: Project, total: Int, quiet: Boolean, tok: Int, force: Boolean = false) {
        results.clear(); stages.clear(); lastStage = null
        startedAt = System.currentTimeMillis()
        progress.value = Progress(0, total)
        phase.value = if (quiet) Phase.Loading else Phase.Scanning
        if (!quiet) Scans.started(app, project.name)
        try {
            val res = Core.get<List<Photo>>("scan_project", args(
                "key" to project.id, "dirs" to project.folders, "compute" to "auto", "quality" to Strategy.currentTier(), "style" to project.style,
                "learn" to (Settings.value.learn && project.learn != false), "quiet" to quiet, "force" to force, "eye_method" to Strategy.eyeMethodNow(),
            ))
            if (tok != token) return
            // 进项目时取舍已清空；照片和上次一模一样时 StateFlow 不会再发一次，得自己重算
            val same = photos.value == res
            photos.value = res
            if (same) redecide()
            phase.value = Phase.Done
            dirty.value = 0
            Projects.update(project.id) { it.copy(cover = res.sortedByDescending { p -> p.score }.take(4).map { p -> p.thumb }) }
            if (!quiet) { celebrate.value = true; scope.launch { delay(3200); celebrate.value = false } }
        } catch (e: Exception) {
            // a cancelled scan is not an error: the project simply goes back to "ready to analyse" (or waits to resume, when the phone paused it)
            if (tok == token) { if (e.message != "cancelled") error.value = e.message; phase.value = Phase.Ready; prepareReady(tok) }
        } finally {
            if (!quiet) Scans.stopped()
        }
    }

    /** "开始分析"：应用定下模型；它要的（含眼睛与人物）第一次在一个问题里下载。 */
    fun startScan() {
        val project = current() ?: return
        val tok = token
        scope.launch {
            val tier = Strategy.resolveTier()
            if (Models.ensurePacks(Strategy.analysisPacks(tier), optional = setOf(PackIds.DETECT, PackIds.EYES, PackIds.FACES)) == Answer.Cancelled) return@launch
            if (tok != token) return@launch
            paused.value = Pause.None
            runScan(project, files.value.size, quiet = false, tok = tok)
        }
    }

    fun stopScan() { runCatching { Core.callBlocking("scan_cancel") } }

    /** 手机太热或电量太低时停下（已分析的都在缓存里）；条件好转后从停下的地方接着分析。 */
    fun pauseFor(why: Pause) {
        if (phase.value != Phase.Scanning) return
        paused.value = why
        stopScan()
    }

    fun resumeAfterPause() {
        if (paused.value == Pause.None) return
        paused.value = Pause.None
        if (phase.value == Phase.Ready) startScan()
    }

    fun reevaluate() { val p = current() ?: return; val tok = token; scope.launch { runScan(p, files.value.size, quiet = true, tok = tok) } }
    fun forceRescan() { val p = current() ?: return; val tok = token; scope.launch { runScan(p, files.value.size, quiet = false, tok = tok, force = true) } }

    fun addFolder(dir: String) { val p = current() ?: return; if (dir !in p.folders) Projects.update(p.id) { it.copy(folders = it.folders + dir) } }
    fun removeFolder(dir: String) { val p = current() ?: return; Projects.update(p.id) { it.copy(folders = it.folders - dir) } }

    /**
     * Keep / review / reject is decided by the engine (packages/core/src/decide.rs): only the keep ratio and the manual overrides cross over, the photos
     * stay there from the last scan. A photo marked by hand changes only that photo (unless a ceiling or "one per group" is on), so it is patched here at
     * once and the whole table is not asked for again on every tap.
     */
    private fun redecide() {
        val list = photos.value
        val p = current()
        if (list.isEmpty() || p == null) { decisions.value = emptyMap(); return }
        val lp = localPatch
        localPatch = null
        if (lp != null && lp.third === list && p.overrides[lp.first] == lp.second) {
            decideJob?.cancel()
            val cur = decisions.value[lp.first] ?: return
            val reasons = listOf(Reason("reason.manual", kotlinx.serialization.json.buildJsonObject { put("stateKey", kotlinx.serialization.json.JsonPrimitive("state.${lp.second}")) })) + if (cur.manual) cur.reasons.drop(1) else cur.reasons
            decisions.value = decisions.value + (lp.first to Decision(lp.second, true, reasons))
            return
        }
        decideJob?.cancel()
        decideJob = scope.launch {
            try {
                val raw = Core.call("decide_photos", args("key" to p.id, "keep_ratio" to (p.keepCap ?: 1f), "pick_mode" to (p.pickMode ?: "diverse"), "one_per_group" to (p.onePerGroup ?: false), "overrides" to p.overrides))
                decisions.value = Fg.json.decodeFromString(MapSerializer(String.serializer(), Decision.serializer()), raw)
            } catch (e: Exception) {
                // "stale": the engine holds another project's photos (a scan of this one is on its way); the next scan result asks again
                decisions.value = emptyMap()
            }
        }
    }

    /** 把一张照片顺时针转 90°：核心库换缩略图与显示尺寸（原文件不动），这里只替换手里那张。 */
    suspend fun rotate(path: String) {
        val cur = photos.value.firstOrNull { it.path == path } ?: return
        val p: Photo = Core.get("set_rotation", args("path" to path, "deg" to (cur.rotation + 90) % 360))
        Large.forget(path)
        photos.value = photos.value.map { if (it.path == path) it.copy(thumb = p.thumb, width = p.width, height = p.height, rotation = p.rotation) else it }
    }

    /** 改判一张（null = 去掉我的标记，交还系统的建议）。只有明确的保留 / 淘汰拿来学喜好；关了学习就什么都不记。 */
    fun decide(path: String, st: String?, fromUndo: Boolean = false) {
        val project = current() ?: return
        if (photos.value.none { it.path == path }) return
        val prev = project.overrides[path]
        val next = project.overrides.toMutableMap()
        if (st == null) next.remove(path) else next[path] = st
        localPatch = if (st != null && project.keepCap == null && project.onePerGroup != true) Triple(path, st, photos.value) else null
        Projects.update(project.id) { it.copy(overrides = next) }
        if (Settings.value.learn && project.learn != false) {
            scope.launch { runCatching { Core.call("record_label", args("path" to path, "label" to if (st == null || st == "review") null else st == "keep", "style" to project.style)) } }
        }
        dirty.value += 1
        if (!fromUndo) {
            undoStack.addLast(path to prev)
            if (undoStack.size > 50) undoStack.removeFirst()
            undoCount.value = undoStack.size
        }
    }

    fun canUndo(): Boolean = undoStack.isNotEmpty()
    /** 撤销最近一次改判；返回被撤销的那张（界面把它选中、滚到眼前）。 */
    fun undo(): String? { val e = undoStack.removeLastOrNull() ?: return null; undoCount.value = undoStack.size; decide(e.first, e.second, fromUndo = true); return e.first }

    /** 刚才整理掉（导出 / 回收）之后：重新读文件夹，已不在的照片的改判随之忘掉。 */
    fun reload() { enter() }
}
