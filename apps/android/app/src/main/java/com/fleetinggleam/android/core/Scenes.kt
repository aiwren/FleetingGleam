package com.fleetinggleam.android.core

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer

/**
 * 场景的名字（桌面端 Culling.tsx 的 describeScenes）：照片定下来后，请核心库给每个场景起名——先从一个很大的地点词库里直接起，
 * 词库叫不出名字的、装了描述模型时再写一句话。名字一个个到达，攒一攒、每秒合并几次再交给界面，而不是每来一个就重画整页。
 */
object Scenes {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val _texts = MutableStateFlow<Map<Int, String>>(emptyMap())
    val texts: StateFlow<Map<Int, String>> = _texts
    private val _describing = MutableStateFlow<Progress?>(null)
    val describing: StateFlow<Progress?> = _describing
    private val buf = HashMap<Int, String>()
    private var flushJob: Job? = null
    private var listening = false

    @Serializable private data class Req(val id: Int, val paths: List<String>)

    private fun push(items: List<SceneText>, waitMs: Long) {
        synchronized(buf) { items.forEach { buf[it.id] = it.text } }
        if (flushJob?.isActive == true) return
        flushJob = scope.launch {
            delay(waitMs)
            val b = synchronized(buf) { HashMap(buf).also { buf.clear() } }
            _texts.value = _texts.value + b
        }
    }

    private fun listen() {
        if (listening) return
        listening = true
        Core.listen("scene-text") { raw -> runCatching { Fg.json.decodeFromString<SceneText>(raw) }.getOrNull()?.let { push(listOf(it), 250) } }
        Core.listen("scene-texts") { raw -> runCatching { Fg.json.decodeFromString(ListSerializer(SceneText.serializer()), raw) }.getOrNull()?.let { push(it, 50) } }
        Core.listen("scene-text-progress") { raw -> runCatching { Fg.json.decodeFromString<Progress>(raw) }.getOrNull()?.let { _describing.value = if (it.done >= it.total) null else it } }
    }

    /** 照片换了（重新分组后场景编号会变）：从头再来；`caption` = 叫不出名字的也写一句话（要描述模型）。 */
    fun describe(photos: List<Photo>, caption: Boolean) {
        listen()
        _texts.value = emptyMap()
        if (photos.isEmpty()) { _describing.value = null; return }
        val reqs = photos.groupBy { it.scene }.entries.sortedByDescending { it.value.size }
            .map { (id, ps) -> Req(id, ps.sortedByDescending { it.score }.map { it.path }.take(6)) }
        _describing.value = Progress(0, reqs.size)
        scope.launch {
            val arr = Fg.json.encodeToJsonElement(ListSerializer(Req.serializer()), reqs)
            runCatching { Core.call("scene_text", args("scenes" to arr, "zh" to I18n.isZh, "caption" to caption)) }.onFailure { _describing.value = null }
        }
    }

    fun clear() { _texts.value = emptyMap(); _describing.value = null }
}
