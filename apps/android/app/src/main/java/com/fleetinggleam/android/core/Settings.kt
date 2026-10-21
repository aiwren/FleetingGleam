package com.fleetinggleam.android.core

import android.content.Context
import android.content.SharedPreferences
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.encodeToJsonElement
import kotlinx.serialization.json.jsonObject

/**
 * 应用设置（和桌面端 src/settings.ts 一样的字段，存法也一样是一个 JSON；导出"我的设置与喜好"时原样带上，两端互导）。
 * `compute`、`quality`、`search`、`customStyles`、`eyeMethod` 以前是设置页上的选项，现在由应用自己决定（Strategy.kt），留着是为了导入的文件意思不变。
 * `tier` 是应用定下的分析模型：整个应用一个（学到的喜好、搜索、分组比较的都是向量，只有同一个模型做出来的才能比），第一次分析前是 null。
 */
@Serializable
data class AppSettings(
    val compute: String = "auto",
    val theme: String = "system",
    val learn: Boolean = true,
    val quality: String = "fast",
    val search: Boolean = false,
    val customStyles: Boolean = false,
    val eyeMethod: String = "mediapipe",
    val tier: String? = null,
    /** 每次有意换了更好的模型就加一（正在看的项目跟着重新看一遍）。 */
    val tierRev: Int = 0,
)

object Settings {
    private lateinit var prefs: SharedPreferences
    private val _state = MutableStateFlow(AppSettings())
    val state: StateFlow<AppSettings> = _state
    private val _info = MutableStateFlow<ScanInfo?>(null)
    /** 上一次分析实际用了什么（设置页"分析方式"的说明里显示）。 */
    val info: StateFlow<ScanInfo?> = _info
    private val _cacheRev = MutableStateFlow(0)
    /** 清除缩略图缓存后加一：正在看的项目重新读文件夹（会发现都要重新分析）。 */
    val cacheRev: StateFlow<Int> = _cacheRev
    private val subs = mutableListOf<() -> Unit>()

    fun init(ctx: Context) {
        prefs = ctx.getSharedPreferences("fg", Context.MODE_PRIVATE)
        _state.value = prefs.getString("settings", null)?.let { runCatching { Fg.camel.decodeFromString<AppSettings>(it) }.getOrNull() } ?: AppSettings()
        _info.value = prefs.getString("scaninfo", null)?.let { runCatching { Fg.json.decodeFromString<ScanInfo>(it) }.getOrNull() }
    }

    val value: AppSettings get() = _state.value

    fun set(f: (AppSettings) -> AppSettings) {
        val next = f(_state.value)
        _state.value = next
        prefs.edit().putString("settings", Fg.camel.encodeToString(AppSettings.serializer(), next)).apply()
        subs.forEach { it() }
    }

    /** 设置变化的通知（云备份靠它知道该上传了）。 */
    fun subscribe(f: () -> Unit) { subs.add(f) }

    fun setScanInfo(i: ScanInfo) {
        _info.value = i
        prefs.edit().putString("scaninfo", Fg.json.encodeToString(ScanInfo.serializer(), i)).apply()
    }

    fun bumpCacheRev() { _cacheRev.value += 1 }

    /** 导出 / 备份时带上的设置（和桌面端传给 `taste_export` 的 settings 对象同形）。 */
    fun asJson(): JsonObject = Fg.camel.encodeToJsonElement(_state.value).jsonObject

    /** 从别处导入的设置：只取在任何机器上意思都一样的那几项；这台机器装了哪个模型、用什么硬件，照旧。 */
    fun adopt(carried: JsonObject?) {
        if (carried == null) return
        val keep = setOf("theme", "learn", "search", "customStyles", "eyeMethod", "quality")
        val now = Fg.camel.encodeToJsonElement(_state.value).jsonObject.toMutableMap()
        for (k in keep) carried[k]?.let { now[k] = it }
        val next = runCatching { Fg.camel.decodeFromJsonElement(AppSettings.serializer(), JsonObject(now)) }.getOrNull() ?: return
        set { next.copy(eyeMethod = "mediapipe") }
    }
}
