package com.fleetinggleam.android.core

import android.content.Context
import android.net.ConnectivityManager
import android.net.LinkProperties
import android.net.Network
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import uniffi.fleetinggleam.EventSink
import uniffi.fleetinggleam.FgEngine
import uniffi.fleetinggleam.FgException
import java.io.File
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CopyOnWriteArrayList

/** 核心库报出来的错误：消息直接给界面显示（和桌面端 `invoke` 抛出的字符串一样）。 */
class CoreException(message: String) : Exception(message)

/**
 * 核心库的引擎（packages/core/src/engine.rs），整个进程一个。用法和桌面端的 Tauri 一模一样：
 * `Core.call("probe_folder", args("dirs" to dirs))` 就是 `invoke("probe_folder", { dirs })`，`Core.listen("scan-photo") { … }` 就是 `listen("scan-photo", …)`。
 * 方法名、参数名（snake_case）、返回的 JSON、事件名与载荷都和桌面端一样，所以 Kotlin 这边照着 TypeScript 写，功能对得上。
 */
object Core {
    @Volatile private var engine: FgEngine? = null
    private val listeners = ConcurrentHashMap<String, CopyOnWriteArrayList<(String) -> Unit>>()

    /** 引擎用的目录：数据（项目、风格、缓存、NAS 镜像……）、缩略图、模型、草稿。 */
    lateinit var data: File; private set
    lateinit var thumbs: File; private set
    lateinit var models: File; private set
    lateinit var scratch: File; private set

    /** 引擎建好之前的调用在这里等（建引擎要加载 JNA 和核心库、读喜好与模型清单，上百毫秒，不该挡着首帧）。 */
    private val ready = java.util.concurrent.CountDownLatch(1)

    fun init(ctx: Context, version: String) {
        if (engine != null) return
        // 目录立刻可用（设置页、下载服务会直接读它们）；引擎本身在后台建
        data = ctx.filesDir
        // 缩略图放在 filesDir：cacheDir 会被系统或厂商的"清理加速"清掉，清掉后（以前）整个项目都得重新分析。
        // 以前放在 cacheDir 里的那些不用搬：分析结果还在，引擎打开项目时发现缩略图不在就重做到新地方
        thumbs = File(ctx.filesDir, "thumbs")
        File(ctx.cacheDir, "thumbs").let { old -> if (old.exists()) Thread { old.deleteRecursively() }.start() }
        models = File(ctx.filesDir, "models")
        scratch = File(ctx.cacheDir, "scratch")
        Thread({ initNow(ctx, version) }, "fg-engine-init").start()
    }

    private fun initNow(ctx: Context, version: String) {
        synchronized(this) {
            if (engine != null) return
            val cm = ctx.getSystemService(ConnectivityManager::class.java)
            val config = args(
                "data" to data.absolutePath, "thumbs" to thumbs.absolutePath, "models" to models.absolutePath, "scratch" to scratch.absolutePath,
                "proxy" to proxyOf(cm?.getLinkProperties(cm.activeNetwork)), "version" to version,
            )
            // ONNX Runtime 由核心库运行时加载（带高通 QNN 的那份，随应用装在原生库目录里）。高通 NPU 的驱动（libQnnHtpV*Skel.so）
            // 是 DSP 那边按 ADSP_LIBRARY_PATH 找的：小米等系统给应用进程预设了这个变量、里面没有应用自己的库目录，ONNX Runtime 看到
            // 已有的就照用，于是 DSP 找不到驱动、NPU 设备建不起来、模型整个落回 CPU。所以把原生库目录放到最前面，系统的路径留在后面。
            // 另一半在 AndroidManifest.xml：声明 libcdsprpc.so（2026-10-23 真机：两样都有，大模型 CLIP 每张 650 → 46 ms）
            val libDir = ctx.applicationInfo.nativeLibraryDir
            runCatching {
                android.system.Os.setenv("ORT_DYLIB_PATH", "$libDir/libonnxruntime.so", true)
                val sys = System.getenv("ADSP_LIBRARY_PATH")?.takeIf { it.isNotBlank() } ?: "/vendor/dsp/cdsp;/vendor/lib/rfsa/adsp;/system/lib/rfsa/adsp;/dsp"
                android.system.Os.setenv("ADSP_LIBRARY_PATH", "$libDir;$sys", true)
            }
            // 核心库的解码交给系统：按需缩放、分块解码，两亿像素的照片也不整张进内存；顺带认识 HEIC / AVIF
            uniffi.fleetinggleam.setImageHelper(AndroidImages)
            val e = FgEngine(config.toString(), object : EventSink {
                // 从核心库的线程上来：监听者自己负责把状态交给界面（StateFlow 是线程安全的）
                override fun onEvent(name: String, payload: String) {
                    listeners[name]?.forEach { runCatching { it(payload) } }
                }
            })
            engine = e
            // 这台手机的名字写进备份：电脑那边恢复前认得出是谁做的
            runCatching { e.call("set_device", args("name" to "${android.os.Build.MANUFACTURER} ${android.os.Build.MODEL}".trim()).toString()) }
            ready.countDown()
            // 换了网络（或用户在系统里改了代理）：模型下载与云备份跟着走新的代理
            cm?.registerDefaultNetworkCallback(object : ConnectivityManager.NetworkCallback() {
                override fun onLinkPropertiesChanged(network: Network, lp: LinkProperties) { e.setProxy(proxyOf(lp)) }
            })
        }
    }

    /** 系统的 HTTP 代理（设置里配的，或模拟器的 -http-proxy），给核心库的联网用。 */
    private fun proxyOf(lp: LinkProperties?): String? {
        lp?.httpProxy?.let { p -> if (!p.host.isNullOrBlank() && p.port > 0) return "http://${p.host}:${p.port}" }
        val host = System.getProperty("https.proxyHost")?.takeIf { it.isNotBlank() } ?: System.getProperty("http.proxyHost")?.takeIf { it.isNotBlank() } ?: return null
        val port = System.getProperty("https.proxyPort") ?: System.getProperty("http.proxyPort") ?: "80"
        return "http://$host:$port"
    }

    /** 调一个引擎方法，返回结果的 JSON 文本。在 IO 线程上跑（引擎的方法都是阻塞的）。 */
    suspend fun call(method: String, args: JsonObject = JsonObject(emptyMap())): String = withContext(Dispatchers.IO) { callBlocking(method, args) }

    fun callBlocking(method: String, args: JsonObject = JsonObject(emptyMap())): String {
        if (engine == null) ready.await()
        val e = engine ?: throw CoreException("the engine is not ready")
        try {
            return e.call(method, args.toString())
        } catch (x: FgException) {
            throw CoreException(x.message ?: method)
        }
    }

    /** 调一个方法并把结果解成 `T`。 */
    suspend inline fun <reified T> get(method: String, args: JsonObject = JsonObject(emptyMap())): T = Fg.json.decodeFromString(call(method, args))

    /** 监听一个事件（载荷是 JSON 文本）；返回取消监听的函数。回调在核心库的线程上。 */
    fun listen(name: String, cb: (String) -> Unit): () -> Unit {
        val list = listeners.getOrPut(name) { CopyOnWriteArrayList() }
        list.add(cb)
        return { list.remove(cb) }
    }
}

/** 参数对象：`args("dirs" to listOf(...), "quiet" to true)`。null 的项写成 JSON 的 null。 */
fun args(vararg pairs: Pair<String, Any?>): JsonObject = JsonObject(pairs.associate { (k, v) -> k to v.toJson() })

fun Any?.toJson(): JsonElement = when (this) {
    null -> JsonNull
    is JsonElement -> this
    is String -> JsonPrimitive(this)
    is Boolean -> JsonPrimitive(this)
    is Number -> JsonPrimitive(this)
    is Enum<*> -> JsonPrimitive(name)
    is Map<*, *> -> JsonObject(entries.associate { (k, v) -> k.toString() to v.toJson() })
    is Iterable<*> -> JsonArray(map { it.toJson() })
    is Array<*> -> JsonArray(map { it.toJson() })
    else -> JsonPrimitive(toString())
}
