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
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.coroutines.flow.first

/** 一个项目在界面上的阶段（和桌面端 Culling.tsx 的 Phase 一样）。 */
enum class Phase { Idle, Probing, Ready, Scanning, Loading, Done }

/** 分析中一张照片的样子：走到哪一步，缩略图已经有了没有。 */
data class StageInfo(val stage: String, val pct: Int, val thumb: String?, val width: Int?, val height: Int?)

/** 分析为什么停了（手机才有：太热、电量太低），停了之后条件好转会自己接着分析。 */
enum class Pause { None, Hot, Battery }

/** 正在跑的那次分析（有进度的那种）属于哪个项目。离开这个项目时它不停，转到后台；首页那张卡上显示进度。 */
data class Running(val id: String, val name: String)

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
    /** 项目所在的 NAS 没连上，照片是本机镜像里上次同步的：顶上提示一句，NAS 上的整理操作置灰（`nasOff` 是那台的名字）。 */
    val offline = MutableStateFlow(false)
    /** 没连上的那台为什么没连上："auth"（密码不对 → 去设置）、"denied"、"net"（连不上 → 重试）。 */
    val nasFailure = MutableStateFlow("net")
    /** 读权限失效的那个系统文件夹镜像（`cull.safExpired` 时界面拿它去"重新授权"）。 */
    @Volatile var expired: Media.Mirror? = null; private set
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
    /** 边分析边判：引擎隔一会儿发来的判定（后来的照片会改变分组和排名，所以会变；分析完换成正式的）。 */
    val live = ConcurrentHashMap<String, Decision>()
    /** 刚判出来或刚改判的那些（路径 → 状态）：界面上让它们"飞"进对应的页签。 */
    val judged = kotlinx.coroutines.flow.MutableSharedFlow<List<Pair<String, String>>>(extraBufferCapacity = 32, onBufferOverflow = kotlinx.coroutines.channels.BufferOverflow.DROP_OLDEST)
    @Volatile var lastStage: Pair<String, String>? = null; private set
    @Volatile var startedAt = 0L; private set

    /** 正在分析的项目（同一时间只有一个）；离开它时照跑，`attached` 为假。 */
    val running = MutableStateFlow<Running?>(null)
    /** 要开始分析这个项目，可另一个项目还在分析：页面问一句要不要先停下它。 */
    val conflict = MutableStateFlow<Running?>(null)
    /** 分析中这一屏正显示着它（离开项目时为假：事件照收，界面不动）。 */
    @Volatile private var attached = false
    /** 开始分析时那个项目的文件列表（回到它时照片墙要用）。 */
    @Volatile private var runningFiles: List<ProbeFile> = emptyList()
    @Volatile private var runningFolders: List<String> = emptyList()
    /** 因为太热 / 电量低停下的那个项目（条件好转后接着分析，不管现在开着哪个项目）。 */
    @Volatile private var pausedProject: Running? = null
    /** 停下等着接着分析的那个项目（确认页上说一句"手机太热，先等等"）。 */
    val pausedId: String? get() = pausedProject?.id
    /** 暂停的那次是"重新分析全部"（还没发起就因为太热 / 电量低停下）：接着分析时照样全部重来。 */
    @Volatile private var pausedForce = false
    /** 上一次进的项目（离开时只取消它自己的读取 / 分析）。 */
    @Volatile private var enteredId: String? = null

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
        // 有进度的事件只来自正在跑的那次分析（安静地读缓存不发）：项目在后台分析时也照收，回来时照片墙是全的
        Core.listen("scan-progress") { raw -> if (running.value != null) runCatching { progress.value = Fg.json.decodeFromString<Progress>(raw) } }
        Core.listen("scan-info") { raw -> runCatching { Settings.setScanInfo(Fg.json.decodeFromString(raw)) } }
        Core.listen("scan-stage") { raw ->
            if (running.value == null) return@listen
            val p = runCatching { Fg.json.decodeFromString<StageEvent>(raw) }.getOrNull() ?: return@listen
            val old = stages[p.path]
            stages[p.path] = StageInfo(p.stage, p.pct, p.thumb ?: old?.thumb, p.width ?: old?.width, p.height ?: old?.height)
            lastStage = p.path to p.stage
            flush()
        }
        Core.listen("scan-photo") { raw ->
            if (running.value == null) return@listen
            val p = runCatching { Fg.json.decodeFromString<Photo>(raw) }.getOrNull() ?: return@listen
            results[p.path] = p
            stages.remove(p.path)
            flush()
        }
        // 边分析边判：只有判定变了的那些会发来
        Core.listen("scan-decide") { raw ->
            if (running.value == null) return@listen
            val list = runCatching { Fg.json.decodeFromString(ListSerializer(Judged.serializer()), raw) }.getOrNull() ?: return@listen
            for (j in list) live[j.path] = j.decision
            judged.tryEmit(list.map { it.path to it.decision.state })
            flush()
        }
        // the photos that were already analysed arrive in batches
        Core.listen("scan-photos") { raw ->
            if (running.value == null) return@listen
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
        val project = current()
        val run = running.value
        val left = enteredId
        enteredId = project?.id
        // 回到正在分析的那个项目（文件夹没变）：接上，照片墙和进度都还在，不重来
        if (run != null && project != null && project.id == run.id && project.folders == runningFolders) {
            attached = true
            currentKey = "${project.id}|${project.folders.joinToString("|")}"
            job?.cancel()
            error.value = null; nasProg.value = null; probeProg.value = null
            files.value = runningFiles
            phase.value = Phase.Scanning
            flush()
            return
        }
        // 离开的那个项目：只停它自己的读取 / 分析。正在分析的那个转到后台接着跑（通知栏照常显示进度）
        attached = false
        // 同样不在调用方的线程上等引擎（"重试"按钮、重新授权会从主线程进来）
        if (left != null && left != run?.id) scope.launch { runCatching { Core.call("scan_cancel", args("key" to left)) } }
        undoStack.clear(); undoCount.value = 0
        error.value = null; celebrate.value = false; dirty.value = 0; nasProg.value = null; peopleProg.value = null; mediaProg.value = null; offline.value = false
        // 上一个项目那台 NAS 为什么没连上，不能带进这个项目（审查第 58 条）
        nasFailure.value = "net"; nasOff.value = ""
        if (run == null && pausedProject == null) paused.value = Pause.None
        // 上一个项目分析中的实时表：没有分析在后台跑就清掉（在跑的那个回来时还要用，留着）
        if (run == null) { results.clear(); stages.clear(); live.clear(); lastStage = null }
        decisions.value = emptyMap()
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
                // NAS 没连上：不再清空照片、整个打不开——核心跳过同步、用本机镜像（`Probe.offline`），这里只提示"显示上次同步的照片"（审查第 30 条）
                val off = shares.filter { !it.connected }
                if (off.isNotEmpty()) { nasOff.value = off.joinToString(", ") { it.name }; nasFailure.value = off.firstNotNullOfOrNull { s -> s.failure?.takeIf { it != "net" } } ?: "net" }
                // a folder picked through the system picker that has no path: bring its new photos into the mirror first
                // 读网盘、写闪存，都是阻塞 IO：放到 IO 线程池上，别占着只有核数那么多的 Default 线程（审查第二批）
                kotlinx.coroutines.withContext(Dispatchers.IO) {
                    Media.syncMirrors(app, project.folders, { done, total, name -> nasProg.value = if (done >= total) null else NasProgress(name, done, total) }, { tok != token })
                }
                nasProg.value = null
                val probe = try {
                    Core.get<Probe>("probe_folder", args("dirs" to project.folders, "compute" to "auto", "quality" to Strategy.currentTier()))
                } catch (e: Exception) {
                    // 还是老核心（NAS 不通时 probe 报错）：照旧说连不上，给"重试"
                    if (off.isEmpty() || e.message == "cancelled") throw e
                    if (tok == token) { photos.value = emptyList(); files.value = emptyList(); phase.value = Phase.Idle; error.value = "cull.nasOffline" }
                    return@launch
                }
                if (tok != token) return@launch
                offline.value = probe.offline || off.isNotEmpty()
                files.value = probe.files; bytes.value = probe.bytes; uncached.value = probe.uncached
                // 没连上、本机也还没有同步过的照片：说连不上，不说"文件夹是空的"
                if (probe.files.isEmpty()) { photos.value = emptyList(); phase.value = Phase.Idle; error.value = if (offline.value) "cull.nasOffline" else "cull.emptyFolder"; return@launch }
                if (project.count != probe.files.size) Projects.update(project.id) { it.copy(count = probe.files.size) }
                if (probe.uncached > 0) {
                    phase.value = Phase.Ready
                    prepareReady(tok)
                } else runScan(project, probe.files.size, quiet = true, tok = tok)
            } catch (e: Media.SafExpired) {
                // 系统文件夹的读权限没了：和 NAS 连不上一样停在这里，给"重新授权"（镜像里的照片一张没动）
                if (tok == token) { photos.value = emptyList(); files.value = emptyList(); phase.value = Phase.Idle; expired = e.mirror; nasOff.value = e.mirror.name; error.value = e.message }
            } catch (e: Exception) {
                if (tok == token && e.message != "cancelled") { error.value = e.message; phase.value = Phase.Idle }
            }
        }
    }

    /** 重新授权失效的那个文件夹：拿到了就重新进项目；没拿到时返回原因（选错了文件夹，或选对了但权限没给）。 */
    suspend fun reauthorize(uri: android.net.Uri): Media.Reauth {
        val m = expired ?: return Media.Reauth.WrongFolder
        val r = kotlinx.coroutines.withContext(Dispatchers.IO) { Media.reauthorize(app, m, uri) }
        if (r == Media.Reauth.Ok) { expired = null; enter() }
        return r
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

    /**
     * `quiet` = 只是从缓存打开（没有进度事件，不挂前台服务）；否则是一次真正的分析：它属于 `project`，离开那个项目也照跑，
     * 跑完时如果页面还开着它就把结果交给页面，不然只更新项目封面、弹一句"分析完成"。`attach` = 页面现在就显示这次分析。
     */
    private suspend fun runScan(project: Project, total: Int, quiet: Boolean, tok: Int, force: Boolean = false, attach: Boolean = true, tier: String? = null) {
        // 诊断：每次分析 / 读取从哪里来（作者 2026-10-23："重新分析"点了没反应）
        android.util.Log.i("FleetingGleam", "runScan ${project.name}：${if (quiet) "安静读取" else "分析"}${if (force) "（强制）" else ""}，来自 ${Throwable().stackTrace.getOrNull(1)?.let { "${it.methodName}:${it.lineNumber}" }}")
        if (quiet) phase.value = Phase.Loading
        else {
            // 已经太热 / 电量太低：不发起分析，直接进暂停（审查第 40 条）。以前照常发起、再靠 `Scans.started` 里的取消停下，
            // 可那个取消早于 scan_project 登记任务、被核心丢掉，分析照跑，通知却一直停在"已暂停"
            val block = Scans.blocker()
            if (block != Pause.None) {
                val list = if (attach) files.value else runningFiles
                progress.value = Progress(if (force) 0 else list.count { it.cached }, total)
                pausedProject = Running(project.id, project.name)
                pausedForce = force
                paused.value = block
                // 前台服务照样挂上（通知写"已暂停（n/N）"）：好转时应用多半在后台，那时系统不让再启前台服务
                Scans.started(app, project.name)
                return
            }
            results.clear(); stages.clear(); live.clear(); lastStage = null
            startedAt = System.currentTimeMillis()
            progress.value = Progress(0, total)
            running.value = Running(project.id, project.name)
            runningFolders = project.folders
            if (attach) { runningFiles = files.value; attached = true; phase.value = Phase.Scanning }
            Scans.started(app, project.name)
        }
        // 结果交不交给页面：安静的读取看 token；分析看页面是不是还开着它
        fun mine() = if (quiet) tok == token else attached && current()?.id == project.id
        try {
            val res = Core.get<List<Photo>>("scan_project", args(
                "key" to project.id, "dirs" to project.folders, "compute" to "auto", "quality" to (tier ?: Strategy.currentTier()), "style" to project.style,
                "learn" to (Settings.value.learn && project.learn != false), "quiet" to quiet, "force" to force, "eye_method" to Strategy.eyeMethodNow(),
                // 边分析边判：和分析完的取舍用同一套参数
                "live" to if (quiet) null else mapOf("keep_ratio" to (project.keepCap ?: 1f), "pick_mode" to (project.pickMode ?: "diverse"), "one_per_group" to (project.onePerGroup ?: false), "overrides" to project.overrides),
            ))
            if (!quiet) finishRunning()
            if (!mine()) {
                if (!quiet) {
                    Projects.update(project.id) { it.copy(cover = res.sortedByDescending { p -> p.score }.take(4).map { p -> p.thumb }, count = res.size) }
                    com.fleetinggleam.android.ui.components.Toasts.show(t("android.cull.bgDone", "name" to project.name), t("android.cull.open")) { Projects.setActive(project.id) }
                }
                return
            }
            // 进项目时取舍已清空；照片和上次一模一样时 StateFlow 不会再发一次，得自己重算
            val same = photos.value == res
            photos.value = res
            if (same) redecide()
            phase.value = Phase.Done
            dirty.value = 0
            Projects.update(project.id) { it.copy(cover = res.sortedByDescending { p -> p.score }.take(4).map { p -> p.thumb }) }
            if (!quiet) { celebrate.value = true; scope.launch { delay(3200); celebrate.value = false } }
        } catch (e: Exception) {
            if (!quiet) finishRunning()
            // a cancelled scan is not an error: the project simply goes back to "ready to analyse" (or waits to resume, when the phone paused it)
            if (mine()) { if (e.message != "cancelled") error.value = e.message; phase.value = Phase.Ready; prepareReady(token) }
            else if (!quiet && e.message != "cancelled") com.fleetinggleam.android.ui.components.Toasts.show(t("android.cull.bgFailed", "name" to project.name, "error" to e.message))
        }
    }

    /**
     * 这次分析结束了（完成、停下或出错）：不再算"正在分析"，前台服务撤掉。因为太热 / 电量低停下的除外：前台服务留着、通知改成"已暂停"，
     * 好转后接着分析时还用它——那时应用多半在后台，系统不让从后台再启前台服务（审查第 29 条）。
     */
    private fun finishRunning() {
        running.value = null
        if (paused.value == Pause.None || pausedProject == null) Scans.stopped()
    }

    /**
     * "开始分析"：应用定下模型；它要的（含眼睛与人物）第一次在一个问题里下载，下载成功后才记下这一档（取消了下次照样问）。
     * 要下的是大模型时询问框里可以"先用标准模型"。另一个项目还在分析时先问一句。
     * `sample`：示例照片——用不必下载或最小的那档，不记下档位，询问框里说明为什么要下载。
     */
    fun startScan(sample: Boolean = false, force: Boolean = false, resume: Boolean = false) {
        val project = current()
        val busy = running.value
        if (project == null || (busy != null && busy.id != project.id)) {
            if (resume) endResume() else if (project != null) conflict.value = busy
            return
        }
        val tok = token
        val demo = sample || Samples.isSample(app, project)
        scope.launch { try { startScanNow(project, demo, tok, force) } finally { if (resume) endResume() } }
    }

    private suspend fun startScanNow(project: Project, demo: Boolean, tok: Int, force: Boolean) {
        val extras = setOf(PackIds.DETECT, PackIds.EYES, PackIds.FACES)
        var tier = if (demo) Strategy.sampleTier() else Strategy.predictTier()
        val alt = if (tier != "fast") Strategy.analysisPacks("fast") else null
        var answer = Models.ensurePacks(Strategy.analysisPacks(tier), optional = extras, fallback = alt, note = if (demo) "android.models.sampleNote" else null)
        if (answer == Answer.Fallback) {
            tier = "fast"
            answer = Models.ensurePacks(Strategy.analysisPacks(tier), optional = extras, note = if (demo) "android.models.sampleNote" else null)
        }
        if (answer == Answer.Cancelled) return
        if (!demo) Strategy.settle(tier)
        if (tok != token) return
        paused.value = Pause.None; pausedProject = null
        runScan(project, files.value.size, quiet = false, tok = tok, tier = tier, force = force)
    }

    /**
     * 停下正在跑的那次分析（不管页面开着哪个项目：通知栏的"停止"也走这里）。调用方多在主线程（停止按钮、通知栏、温度回调），
     * 而 scan_cancel 要等引擎的锁，所以放到 IO 上发出去，这里立刻返回；停下之后 `running` 会变回 null。
     */
    fun stopScan() {
        val r = running.value ?: return
        scope.launch { runCatching { Core.call("scan_cancel", args("key" to r.id)) } }
    }

    /** 问过了：先停下另一个项目的分析（分析过的都在缓存里，回到它时接着来），再分析这个。 */
    fun stopOtherAndStart() {
        conflict.value = null
        stopScan()
        scope.launch {
            withTimeoutOrNull(10_000) { running.first { it == null } }
            startScan()
        }
    }

    /** 手机太热或电量太低时停下（已分析的都在缓存里）；条件好转后从停下的地方接着分析，不管那时开着哪个项目。 */
    fun pauseFor(why: Pause) {
        val run = running.value ?: return
        pausedProject = run
        pausedForce = false
        paused.value = why
        stopScan()
    }

    /** 不再等着接着分析（用户点了通知栏的"停止"、前台服务到了时限）：忘掉暂停的那个项目，撤掉为它留着的前台服务。 */
    fun dropPause() {
        if (paused.value == Pause.None && pausedProject == null) return
        pausedProject = null
        paused.value = Pause.None
        if (running.value == null) Scans.stopped()
    }

    fun resumeAfterPause() {
        if (paused.value == Pause.None) return
        paused.value = Pause.None
        val p = pausedProject
        val force = pausedForce
        pausedProject = null; pausedForce = false
        // 留着的前台服务：接着分析时还用它；没接上（项目没了、下载被取消、页面换了）就撤掉，别一直挂着"已暂停"。
        // 接续还在路上（读文件夹、NAS 慢、等下载确认）时不撤：撤了之后应用在后台，前台服务就再也起不来（审查第 50 条）；接续结束时 endResume 收尾
        resuming = true
        scope.launch { delay(30_000); if (!resuming && running.value == null && paused.value == Pause.None) Scans.stopped() }
        if (p == null) { endResume(); return }
        val project = Projects.store.value.projects.firstOrNull { it.id == p.id && it.trashed == null } ?: run { endResume(); return }
        if (current()?.id == project.id) {
            if (phase.value == Phase.Ready || (force && phase.value == Phase.Done)) startScan(force = force, resume = true) else endResume()
            return
        }
        // 那个项目已经不在眼前：在后台接着分析（先把文件列表拿回来，回到它时照片墙要用）
        scope.launch {
            try {
                val probe = runCatching { Core.get<Probe>("probe_folder", args("dirs" to project.folders, "compute" to "auto", "quality" to Strategy.currentTier())) }.getOrNull() ?: return@launch
                if (running.value != null) return@launch
                runningFiles = probe.files
                runScan(project, probe.files.size, quiet = false, tok = token, attach = false, force = force)
            } finally { endResume() }
        }
    }

    /** 暂停后的接续正在路上（从好转到真的发起分析之间）：这期间不撤留着的前台服务。 */
    @Volatile private var resuming = false

    /** 接续结束（发起过分析、又停下了，或者没接上）：没在分析也没在等着接着分析，就撤掉留着的前台服务。 */
    private fun endResume() {
        resuming = false
        if (running.value == null && paused.value == Pause.None) Scans.stopped()
    }

    /** 用现在学到的喜好重新给这个项目打分（只读缓存、重新分组取舍，不重新分析），做完说一声。 */
    fun reevaluate() {
        val p = current() ?: return
        val tok = token
        scope.launch {
            runScan(p, files.value.size, quiet = true, tok = tok)
            if (tok == token && phase.value == Phase.Done) com.fleetinggleam.android.ui.components.Toasts.show(t("android.cull.reevalDone"))
        }
    }
    fun forceRescan() {
        val p = current() ?: return
        val run = running.value
        if (run != null && run.id != p.id) { conflict.value = run; return }
        val tok = token
        scope.launch { paused.value = Pause.None; pausedProject = null; pausedForce = false; runScan(p, files.value.size, quiet = false, tok = tok, force = true) }
    }

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
