package com.fleetinggleam.android.core

import android.app.ActivityManager
import android.app.ApplicationExitInfo
import android.content.Context
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * 本地的崩溃记录（审查第 36 条）。核心自己装了 panic hook、写 `crash-<时间>.txt`；这里补 Kotlin 这一半：
 * - 未捕获的异常：写 `crash-kotlin-<时间>.txt`，再交给系统原来的处理（照常弹"应用已停止"）。
 * - 启动时读系统记下的上几次退出（`ApplicationExitInfo`）：崩溃、ANR、原生崩溃、因内存被杀……没记过的写 `exit-<时间>.txt`。
 * 都写到核心 `diagnostics_dir` 给的目录（就是 `<数据目录>/logs`；引擎还没建好时先用同一个路径）。纯本地，什么都不发出去；
 * 关于页"导出诊断信息"把它们连同 app.log 打成一个文本文件，用户自己决定发不发。
 */
object Diagnostics {
    private const val KEEP = 5
    @Volatile private var dir: File? = null
    private var version = ""

    private fun stamp(ms: Long = System.currentTimeMillis()): String = SimpleDateFormat("yyyyMMdd-HHmmss", Locale.ROOT).format(Date(ms))

    /** 日志目录：核心给的，或者（引擎还没好、老核心没有这个命令）`<数据目录>/logs`。 */
    private fun logs(ctx: Context): File = dir ?: File(ctx.filesDir, "logs")

    /** 进程一开始就调：装上未捕获异常的处理；再在后台问核心要目录、把上几次的退出原因记下来。 */
    fun init(ctx: Context, appVersion: String) {
        val app = ctx.applicationContext
        version = appVersion
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, e ->
            runCatching { writeCrash(app, thread, e) }
            previous?.uncaughtException(thread, e)
        }
        Thread({
            runCatching { dir = File(Fg.json.decodeFromString<String>(Core.callBlocking("diagnostics_dir"))) }
            runCatching { recordExits(app) }
        }, "fg-diagnostics").start()
    }

    private fun header(): String = buildString {
        appendLine("FleetingGleam $version")
        appendLine("Android ${android.os.Build.VERSION.RELEASE} (API ${android.os.Build.VERSION.SDK_INT}) · ${android.os.Build.MANUFACTURER} ${android.os.Build.MODEL} · ${android.os.Build.SOC_MODEL}")
    }

    private fun writeCrash(ctx: Context, thread: Thread, e: Throwable) {
        val d = logs(ctx).apply { mkdirs() }
        val sw = java.io.StringWriter()
        e.printStackTrace(java.io.PrintWriter(sw))
        File(d, "crash-kotlin-${stamp()}.txt").writeText(header() + "线程：${thread.name}\n\n$sw")
        prune(d, "crash-kotlin-")
    }

    /** 系统记下的、上次看过之后的退出：只记不正常的那些（正常退出、用户划掉、自己停的不记）。 */
    private fun recordExits(ctx: Context) {
        val am = ctx.getSystemService(ActivityManager::class.java) ?: return
        val prefs = ctx.getSharedPreferences("fg.diag", Context.MODE_PRIVATE)
        val seen = prefs.getLong("lastExit", 0L)
        val list = am.getHistoricalProcessExitReasons(ctx.packageName, 0, 10).filter { it.timestamp > seen }
        if (list.isEmpty()) return
        prefs.edit().putLong("lastExit", list.maxOf { it.timestamp }).apply()
        val bad = setOf(
            ApplicationExitInfo.REASON_CRASH, ApplicationExitInfo.REASON_CRASH_NATIVE, ApplicationExitInfo.REASON_ANR,
            ApplicationExitInfo.REASON_LOW_MEMORY, ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE, ApplicationExitInfo.REASON_INITIALIZATION_FAILURE,
            ApplicationExitInfo.REASON_SIGNALED,
        )
        val d = logs(ctx).apply { mkdirs() }
        // 第一次装上这个功能时 seen 是 0：只记最近一次，别把很久以前的都翻出来
        for (x in (if (seen == 0L) list.take(1) else list).filter { it.reason in bad }) {
            val trace = runCatching { x.traceInputStream?.use { s -> String(head(s, 256 * 1024)) } }.getOrNull()
            val text = header() + buildString {
                appendLine("退出时间：${stamp(x.timestamp)}")
                appendLine("原因：${reasonName(x.reason)}（${x.reason}）· 状态 ${x.status} · 重要性 ${x.importance}")
                x.description?.let { appendLine("说明：$it") }
                appendLine("内存：PSS ${x.pss} KB · RSS ${x.rss} KB")
                if (!trace.isNullOrEmpty()) { appendLine(); appendLine(trace) }
            }
            File(d, "exit-${stamp(x.timestamp)}.txt").writeText(text)
        }
        prune(d, "exit-")
    }

    /** 流的前 `max` 个字节（ANR 的线程转储可能好几 MB，只要开头）。 */
    private fun head(s: java.io.InputStream, max: Int): ByteArray {
        val buf = ByteArray(max)
        var n = 0
        while (n < max) { val r = s.read(buf, n, max - n); if (r < 0) break; n += r }
        return buf.copyOf(n)
    }

    private fun reasonName(r: Int): String = when (r) {
        ApplicationExitInfo.REASON_CRASH -> "crash"
        ApplicationExitInfo.REASON_CRASH_NATIVE -> "native crash"
        ApplicationExitInfo.REASON_ANR -> "anr"
        ApplicationExitInfo.REASON_LOW_MEMORY -> "low memory"
        ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE -> "excessive resource usage"
        ApplicationExitInfo.REASON_INITIALIZATION_FAILURE -> "initialization failure"
        ApplicationExitInfo.REASON_SIGNALED -> "signaled"
        else -> "other"
    }

    /** 同一种只留最近的几份。 */
    private fun prune(d: File, prefix: String) {
        d.listFiles { f -> f.name.startsWith(prefix) }?.sortedByDescending { it.name }?.drop(KEEP)?.forEach { it.delete() }
    }

    /**
     * 关于页的"导出诊断信息"：核心把版本、系统信息、去掉密码与令牌的设置、最近的 app.log 和全部崩溃记录写成一个文本文件，
     * 再复制到用户在系统选择器里选的位置。
     */
    suspend fun export(ctx: Context, uri: android.net.Uri) {
        val tmp = File(ctx.cacheDir, "diagnostics/fleetinggleam-diagnostics-${stamp()}.txt").apply { parentFile?.mkdirs() }
        try {
            val written = Fg.json.decodeFromString<String>(Core.call("diagnostics_export", args("out" to tmp.absolutePath)))
            kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
                val out = ctx.contentResolver.openOutputStream(uri, "wt") ?: throw CoreException("denied:open")
                out.use { o -> File(written).inputStream().use { it.copyTo(o) } }
            }
        } finally {
            tmp.delete()
        }
    }
}
