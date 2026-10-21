package com.fleetinggleam.android.core

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import com.fleetinggleam.android.ui.components.Toasts

/** 模型包的名字（和桌面端 src/models.ts 的 PackId 一样，多一个 Android 才用的 "detect"）。 */
object PackIds {
    const val STANDARD = "standard"; const val HIGH = "high"; const val ULTRA = "ultra"; const val SEARCH = "search"
    const val EYES = "eyes"; const val FACES = "faces"; const val DETECT = "detect"; const val SCENETEXT = "scenetext"
}

enum class Answer { Ready, Downloaded, Cancelled }

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

    suspend fun ensurePack(id: String): Answer {
        refresh()
        if (_packs.value.isEmpty() || installed(id)) return Answer.Ready
        val d = CompletableDeferred<Answer>()
        val bytes = try { Core.get<Long>("pack_size", args("id" to id)) } catch (e: Exception) {
            _gate.value = Gate(id = id, phase = "error", bytes = 0, error = e.message, resolve = d)
            return d.await()
        }
        _gate.value = Gate(id = id, phase = "ask", bytes = bytes, total = bytes, resolve = d)
        return d.await()
    }

    /**
     * Several packs under one question: the person is told the whole size once and agrees once. What is already here is left out; answers "ready"
     * when nothing is missing.
     */
    suspend fun ensurePacks(ids: List<String>): Answer {
        refresh()
        if (_packs.value.isEmpty()) return Answer.Ready
        val missing = ids.distinct().filter { !installed(it) }
        if (missing.isEmpty()) return Answer.Ready
        if (missing.size == 1) return ensurePack(missing[0])
        val d = CompletableDeferred<Answer>()
        val sizes = mutableListOf<Long>()
        for (id in missing) {
            try { sizes.add(Core.get<Long>("pack_size", args("id" to id))) } catch (e: Exception) {
                _gate.value = Gate(id = "bundle", ids = missing, phase = "error", bytes = 0, error = e.message, resolve = d)
                return d.await()
            }
        }
        val bytes = sizes.sum()
        _gate.value = Gate(id = "bundle", ids = missing, sizes = sizes, phase = "ask", bytes = bytes, total = bytes, resolve = d)
        return d.await()
    }

    fun startDownload() {
        val g = _gate.value ?: return
        _gate.value = g.copy(phase = "downloading", done = 0, error = null)
        val ids = g.ids ?: listOf(g.id)
        // 下载期间挂一个前台服务：切到别的应用、锁屏，下载也不会被系统停掉
        app?.let { com.fleetinggleam.android.service.DownloadService.start(it) }
        // the packs one after the other; the progress of the one being fetched is added to the sizes of those already done
        scope.launch {
            try {
                var before = 0L
                for ((i, id) in ids.withIndex()) {
                    base = before
                    Core.call("pack_download", args("id" to id))
                    before += g.sizes?.getOrNull(i) ?: 0
                }
                base = 0
                refresh()
                val wasHidden = _gate.value?.hidden == true
                _gate.value = null
                if (wasHidden) Toasts.show(t("android.models.done", "name" to t(if (g.ids != null) "android.models.bundle" else "models.name.${g.id}")))
                g.resolve.complete(Answer.Downloaded)
            } catch (e: Exception) {
                base = 0
                if (e.message?.contains("cancelled") == true) {
                    _gate.value = null
                    g.resolve.complete(Answer.Cancelled)
                } else {
                    // 出错了：后台的也把对话框拿回来，说清楚、可以重试
                    _gate.value = (_gate.value ?: g).copy(phase = "error", error = e.message, hidden = false)
                }
            } finally {
                app?.let { com.fleetinggleam.android.service.DownloadService.stop(it) }
            }
        }
    }

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
