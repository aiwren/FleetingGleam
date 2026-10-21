package com.fleetinggleam.android.core

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

/**
 * 照片搜索（桌面端 search.ts）：每张照片要先有一个小向量才能被搜到，文件夹一确定就在后台从缩略图做（快、在本机），
 * 等有人来搜时多半已经齐了。索引的进度给搜索框画一圈细环；搜索在打字停下一会儿后才发出去（由界面防抖）。
 */
object Search {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    data class Index(val available: Boolean = false, val ready: Int = 0, val total: Int = 0, val progress: Float? = null)

    private val _index = MutableStateFlow(Index())
    val index: StateFlow<Index> = _index
    private var key = ""
    private var running = ""
    private var job: Job? = null
    private var listening = false

    /** 让这些文件夹的索引保持完整（换了文件夹、或模型刚下载好时 `rev` 加一，重新检查一遍）。 */
    fun keep(dirs: List<String>, rev: Int = 0) {
        val k = dirs.joinToString("\n") + "|" + rev
        if (k == key) return
        key = k
        job?.cancel()
        if (dirs.isEmpty()) { _index.value = Index(); return }
        if (!listening) {
            listening = true
            Core.listen("search-index-progress") { raw ->
                runCatching { Fg.json.decodeFromString<Progress>(raw) }.getOrNull()?.let { p ->
                    _index.value = _index.value.copy(progress = if (p.total > 0) p.done.toFloat() / p.total else 1f)
                }
            }
        }
        job = scope.launch {
            val s = runCatching { status(dirs) }.getOrNull() ?: return@launch
            _index.value = Index(s.available, s.ready, s.total)
            if (!s.available || s.ready >= s.total || running == k) return@launch
            running = k
            _index.value = _index.value.copy(progress = 0f)
            // 索引可有可无：没建完时搜索只覆盖已经好的那些
            runCatching { Core.call("search_index", args("dirs" to dirs)) }
            running = ""
            if (key == k) runCatching { status(dirs) }.getOrNull()?.let { _index.value = Index(it.available, it.ready, it.total) }
        }
    }

    suspend fun status(dirs: List<String>): SearchStatus = Core.get("search_status", args("dirs" to dirs))

    suspend fun photos(dirs: List<String>, query: String): List<Hit> = Core.get("search_photos", args("dirs" to dirs, "query" to query))

    /** 全局搜索里点了一张：项目打开后把它选中。 */
    private var pending: String? = null
    fun setPendingSelect(path: String?) { pending = path }
    fun takePendingSelect(): String? = pending.also { pending = null }
}
