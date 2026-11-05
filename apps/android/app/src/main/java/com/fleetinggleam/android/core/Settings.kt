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
    /** 液态玻璃（折射、模糊、高光）。关掉就是普通的半透明磨砂，弱机器和模拟器上省很多 GPU。 */
    val glass: Boolean = true,
    /** 液态玻璃档位：`auto` 按这台机器的性能决定 / `on` / `off`。null 是还没选过的旧设置：旧的 glass=false 当作 off，其余当作 auto。 */
    val glassMode: String? = null,
    /** 判定后自动前进：大图里判完翻到下一张，照片墙上判完选中下一张（A7，默认开）。 */
    val autoAdvance: Boolean = true,
    /** 界面动效：`full` 完整 / `calm` 克制（判定的星光、光圈不播，提示类浮层更淡）。和桌面端同名。 */
    val effects: String = "full",
    /** 启动后几秒在后台问一次 GitHub Releases 有没有新版本（Updates.kt）。和桌面端同名。 */
    val autoUpdate: Boolean = true,
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

    /** 自动档下这台机器开不开液态玻璃（启动时判断一次，见 [strongGpu]）。 */
    var autoGlass: Boolean = false
        private set

    fun init(ctx: Context) {
        prefs = ctx.getSharedPreferences("fg", Context.MODE_PRIVATE)
        autoGlass = strongGpu(ctx)
        _state.value = prefs.getString("settings", null)?.let { runCatching { Fg.camel.decodeFromString<AppSettings>(it) }.getOrNull() } ?: AppSettings(glassMode = "auto")
        _info.value = prefs.getString("scaninfo", null)?.let { runCatching { Fg.json.decodeFromString<ScanInfo>(it) }.getOrNull() }
    }

    val value: AppSettings get() = _state.value

    /** 实际的液态玻璃档位（auto / on / off）。 */
    fun glassMode(s: AppSettings = _state.value): String = s.glassMode ?: if (s.glass) "auto" else "off"

    /** 现在要不要画液态玻璃：模拟器一律不画（没有真 GPU，着色器一帧要几百毫秒）。 */
    fun glassOn(s: AppSettings = _state.value): Boolean = !isEmulator() && when (glassMode(s)) { "on" -> true; "off" -> false; else -> autoGlass }

    /**
     * 这台机器的 GPU 撑不撑得住液态玻璃（每帧对背后的内容做模糊 + 折射着色器）。
     * 1. 模拟器、系统标为低内存的机器：不开。
     * 2. 厂商声明了媒体性能等级（Android 12 起，S 档及以上，基本都是近几年的旗舰和次旗舰）：开。
     * 3. 没声明的看硬件：内存 7 GB 以上、最快的核心 2.8 GHz 以上才开（骁龙 870 / 8 系、天玑 9000 系这一档）。
     */
    private fun strongGpu(ctx: Context): Boolean = runCatching {
        if (isEmulator()) return false
        val am = ctx.getSystemService(android.app.ActivityManager::class.java) ?: return false
        if (am.isLowRamDevice) return false
        if (android.os.Build.VERSION.MEDIA_PERFORMANCE_CLASS >= android.os.Build.VERSION_CODES.S) return true
        val mem = android.app.ActivityManager.MemoryInfo().also { am.getMemoryInfo(it) }.totalMem
        val ghz = maxCpuGhz()
        mem >= 7L * 1024 * 1024 * 1024 && ghz >= 2.8
    }.getOrDefault(false)

    /** 各个核心里最高的标称频率（GHz），读不到就是 0。 */
    private fun maxCpuGhz(): Double = (0 until Runtime.getRuntime().availableProcessors()).maxOfOrNull { i ->
        runCatching { java.io.File("/sys/devices/system/cpu/cpu$i/cpufreq/cpuinfo_max_freq").readText().trim().toLong() / 1e6 }.getOrDefault(0.0)
    } ?: 0.0

    /** 模拟器（goldfish / ranchu / generic）。 */
    fun isEmulator(): Boolean = android.os.Build.HARDWARE.let { it.contains("goldfish") || it.contains("ranchu") } || android.os.Build.FINGERPRINT.contains("generic") || android.os.Build.PRODUCT.contains("sdk_gphone")

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
        val keep = setOf("theme", "learn", "search", "customStyles", "eyeMethod", "quality", "autoAdvance", "effects")
        val now = Fg.camel.encodeToJsonElement(_state.value).jsonObject.toMutableMap()
        for (k in keep) carried[k]?.let { now[k] = it }
        val next = runCatching { Fg.camel.decodeFromJsonElement(AppSettings.serializer(), JsonObject(now)) }.getOrNull() ?: return
        set { next.copy(eyeMethod = "mediapipe") }
    }
}
