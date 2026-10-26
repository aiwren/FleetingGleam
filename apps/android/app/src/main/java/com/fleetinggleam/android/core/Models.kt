package com.fleetinggleam.android.core

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import com.fleetinggleam.android.ui.components.Toasts

/** 模型包的名字（和桌面端 src/models.ts 的 PackId 一样，多一个 Android 才用的 "detect"）。 */
object PackIds {
    const val STANDARD = "standard"; const val HIGH = "high"; const val ULTRA = "ultra"; const val SEARCH = "search"
    const val EYES = "eyes"; const val FACES = "faces"; const val DETECT = "detect"; const val SCENETEXT = "scenetext"
}

/**
 * `Skipped`：可选的包没下成，用户选了先不用（分析照常，只是少了那部分）。
 * `Fallback`：问要不要下大模型时，用户选了"先用标准模型"（调用方改去要标准模型那一套）。
 */
enum class Answer { Ready, Downloaded, Cancelled, Skipped, Fallback }

/** `id` names the dialog's text: a pack, or "bundle" when several are fetched under one question (`ids` says which, `sizes` how big each is). */
data class Gate(
    val id: String,
    val ids: List<String>? = null,
    val sizes: List<Long>? = null,
    val phase: String, // "ask" | "downloading" | "error"
    val bytes: Long,
    val done: Long = 0,
    val total: Long = 0,
    val error: String? = null,
    val resolve: CompletableDeferred<Answer>,
    /** 下载转到后台了：对话框收起，通知栏和底部一粒小胶囊显示进度；等它的那件事（分析、搜索…）下载完照样接着做。 */
    val hidden: Boolean = false,
    /** 这些包没有也能用（人脸检测、眼睛、人物识别）：它们下载失败时可以"先跳过"。 */
    val optional: Set<String> = emptySet(),
    /** 下载失败的那个包。 */
    val failed: String? = null,
    /** 询问时可以改用的那一套更小的（"先用标准模型"）要下多少字节；null = 不提供这个选项。 */
    val fallbackBytes: Long? = null,
    /** 询问时多说一句（文案键），比如示例照片为什么要下载。 */
    val note: String? = null,
)

/**
 * Models are downloaded when they are first needed, one pack at a time (packages/core/src/modelpacks.rs; the same manifest and SHA-256 checks as the Mac).
 * `ensurePack` is the single door: call it before anything that needs a pack, and it either says "ready" straight away or asks the person (size shown),
 * downloads with progress, and answers.
 */
object Models {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val _packs = MutableStateFlow<List<PackStatus>>(emptyList())
    val packs: StateFlow<List<PackStatus>> = _packs
    private val _gate = MutableStateFlow<Gate?>(null)
    val gate: StateFlow<Gate?> = _gate
    /** Bytes of the packs of a bundle that are already down, while the next one is fetched. */
    @Volatile private var base = 0L

    private var app: android.content.Context? = null

    fun init(ctx: android.content.Context) {
        app = ctx.applicationContext
        Core.listen("model-progress") { raw ->
            val p = runCatching { Fg.json.decodeFromString<ModelProgress>(raw) }.getOrNull() ?: return@listen
            val g = _gate.value ?: return@listen
            if (g.phase == "downloading") _gate.value = g.copy(done = if (g.ids != null) minOf(g.bytes, base + p.done) else p.done, total = if (g.ids != null) g.bytes else p.total)
        }
        scope.launch { refresh() }
    }

    suspend fun refresh() {
        runCatching { _packs.value = Core.get<List<PackStatus>>("packs_status", args("with_sizes" to true)) }
    }

    /** Whether a pack is on this phone (as of the last look). */
    fun installed(id: String): Boolean = _packs.value.firstOrNull { it.id == id }?.installed == true

    /** 这次运行里用户选了"先跳过"的可选包：不再追问（设置 → 模型与存储里随时可以再下）。 */
    private val skipped = java.util.Collections.synchronizedSet(mutableSetOf<String>())

    /**
     * 同一时间只有一个对话框（核心也只允许一个下载）：后来的排在这里，等前一个答完（下载完、取消或跳过）再问。
     * 以前直接覆盖 `_gate`，前一个下载完又把新的清掉，等着新的那一方永远卡住（审查第 20 条）。轮到时重新看一遍装没装——前一个可能刚好下了它。
     */
    private val turn = Mutex()

    suspend fun ensurePack(id: String, optional: Set<String> = emptySet()): Answer = turn.withLock { ensurePackNow(id, optional) }

    private suspend fun ensurePackNow(id: String, optional: Set<String>, note: String? = null): Answer {
        refresh()
        if (_packs.value.isEmpty() || installed(id)) return Answer.Ready
        if (id in optional && id in skipped) return Answer.Skipped
        val d = CompletableDeferred<Answer>()
        val bytes = try { Core.get<Long>("pack_size", args("id" to id)) } catch (e: Exception) {
            _gate.value = Gate(id = id, phase = "error", bytes = 0, error = e.message, resolve = d, optional = optional, failed = id)
            return answer(d)
        }
        _gate.value = Gate(id = id, phase = "ask", bytes = bytes, total = bytes, resolve = d, optional = optional, note = note)
        return answer(d)
    }

    /**
     * Several packs under one question: the person is told the whole size once and agrees once. What is already here is left out; answers "ready"
     * when nothing is missing.
     */
    suspend fun ensurePacks(ids: List<String>, optional: Set<String> = emptySet(), fallback: List<String>? = null, note: String? = null): Answer =
        turn.withLock { ensurePacksNow(ids, optional, fallback, note) }

    /** `fallback`：询问框里提供"先用标准模型"时那一套包（还缺的才算大小）；选了它答 `Fallback`。 */
    private suspend fun ensurePacksNow(ids: List<String>, optional: Set<String>, fallback: List<String>? = null, note: String? = null): Answer {
        refresh()
        if (_packs.value.isEmpty()) return Answer.Ready
        // 必需的先下，可选的放后面：可选的失败时前面的已经到了
        val missing = ids.distinct().filter { !installed(it) && !(it in optional && it in skipped) }.sortedBy { it in optional }
        if (missing.isEmpty()) return if (ids.any { it in skipped && !installed(it) }) Answer.Skipped else Answer.Ready
        // 改用的那一套和要下的一样（或者已经都在）就不提供
        val alt = fallback?.distinct()?.filter { !installed(it) }?.takeIf { f -> f.toSet() != missing.toSet() }
        val altBytes = alt?.let { f -> runCatching { f.sumOf { Core.get<Long>("pack_size", args("id" to it)) } }.getOrNull() }
        if (missing.size == 1 && altBytes == null) return ensurePackNow(missing[0], optional, note)
        val d = CompletableDeferred<Answer>()
        val sizes = mutableListOf<Long>()
        for (id in missing) {
            try { sizes.add(Core.get<Long>("pack_size", args("id" to id))) } catch (e: Exception) {
                _gate.value = Gate(id = "bundle", ids = missing, phase = "error", bytes = 0, error = e.message, resolve = d, optional = optional, failed = id)
                return answer(d)
            }
        }
        val bytes = sizes.sum()
        _gate.value = Gate(id = "bundle", ids = missing, sizes = sizes, phase = "ask", bytes = bytes, total = bytes, resolve = d, optional = optional, fallbackBytes = altBytes, note = note)
        return answer(d)
    }

    /** 询问框里选了"先用标准模型"：这个对话框答 `Fallback`，调用方接着去要标准模型那一套。 */
    fun fallbackGate() {
        val g = _gate.value ?: return
        if (g.phase != "ask" || g.fallbackBytes == null) return
        _gate.value = null
        g.resolve.complete(Answer.Fallback)
    }

    /** 可选包下载失败后"先跳过"：没下到的可选包这次运行不再问，等着的那件事接着做（少了那部分）。 */
    fun skipGate() {
        val g = _gate.value ?: return
        (g.ids ?: listOf(g.id)).filter { it in g.optional && !installed(it) }.forEach { skipped.add(it) }
        _gate.value = null
        g.resolve.complete(Answer.Skipped)
    }

    fun startDownload() {
        val g = _gate.value ?: return
        _gate.value = g.copy(phase = "downloading", done = 0, error = null)
        val ids = g.ids ?: listOf(g.id)
        // 下载期间挂一个前台服务：切到别的应用、锁屏，下载也不会被系统停掉
        app?.let { com.fleetinggleam.android.service.DownloadService.start(it) }
        // the packs one after the other; the progress of the one being fetched is added to the sizes of those already done
        scope.launch {
            var current: String? = null
            try {
                var before = 0L
                for ((i, id) in ids.withIndex()) {
                    base = before
                    current = id
                    if (installed(id)) { before += g.sizes?.getOrNull(i) ?: 0; continue }
                    Core.call("pack_download", args("id" to id))
                    before += g.sizes?.getOrNull(i) ?: 0
                }
                base = 0
                refresh()
                val wasHidden = _gate.value?.hidden == true
                clear(g)
                if (wasHidden) Toasts.show(t("android.models.done", "name" to t(if (g.ids != null) "android.models.bundle" else "models.name.${g.id}")))
                g.resolve.complete(Answer.Downloaded)
            } catch (e: Exception) {
                base = 0
                if (Errors.isCancelled(e.message) || e.message?.contains("cancelled") == true) {
                    clear(g)
                    g.resolve.complete(Answer.Cancelled)
                } else {
                    // 出错了：后台的也把对话框拿回来，说清楚、可以重试
                    refresh()
                    _gate.value = (_gate.value ?: g).copy(phase = "error", error = e.message, hidden = false, failed = current)
                }
            } finally {
                app?.let { com.fleetinggleam.android.service.DownloadService.stop(it) }
            }
        }
    }

    /**
     * 等对话框的答复。等的那一方被取消了（比如设置页关掉了）对话框却还开着、下载还在跑：照样等它答完再放开排队的锁，
     * 否则排在后面的会把它覆盖掉。
     */
    private suspend fun answer(d: CompletableDeferred<Answer>): Answer = try { d.await() } catch (e: kotlinx.coroutines.CancellationException) {
        kotlinx.coroutines.withContext(kotlinx.coroutines.NonCancellable) { d.await() }
        throw e
    }

    /** 撤掉这一次的对话框（只在它还是当前那个时）：别把排在后面、刚问上的那个清掉。 */
    private fun clear(g: Gate) { if (_gate.value?.resolve === g.resolve) _gate.value = null }

    /** 下载转到后台（对话框收起）。 */
    fun hideGate() { _gate.value?.let { if (it.phase == "downloading") _gate.value = it.copy(hidden = true) } }

    /** 从底部的小胶囊点回来：再显示对话框。 */
    fun showGate() { _gate.value?.let { _gate.value = it.copy(hidden = false) } }

    /** Closes the dialog (before or during a download) and answers "cancelled". */
    fun cancelGate() {
        val g = _gate.value ?: return
        if (g.phase == "downloading") {
            scope.launch { runCatching { Core.call("pack_cancel") } }
            return
        }
        _gate.value = null
        g.resolve.complete(Answer.Cancelled)
    }

    suspend fun remove(id: String) {
        try { Core.call("pack_remove", args("id" to id)) } finally { refresh() }
    }
}
