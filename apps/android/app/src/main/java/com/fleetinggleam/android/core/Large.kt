package com.fleetinggleam.android.core

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import java.util.concurrent.ConcurrentHashMap

/**
 * 大图（桌面端 largeSrc.ts）：全屏查看、单张与对比用的大预览，每张照片只请求一次、所有调用方共用。
 * `full` = 大图查看器：手机能直接显示的格式就用原图本身（HDR 增益图、广色域、每一个像素都在）；其余地方 2560 px 的预览就够，也便宜得多。
 */
object Large {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val jobs = ConcurrentHashMap<String, Deferred<String?>>()
    private val done = ConcurrentHashMap<String, String>()
    private fun keyOf(path: String, full: Boolean) = (if (full) "f:" else "p:") + path

    /** 已经准备好的（没有就是 null，调用方先显示缩略图）。 */
    fun ready(path: String, full: Boolean = false): String? = done[keyOf(path, full)]

    suspend fun get(path: String, full: Boolean = false): String? {
        val k = keyOf(path, full)
        done[k]?.let { return it }
        val job = jobs.getOrPut(k) {
            scope.async {
                runCatching { Core.get<String>("large_preview", args("path" to path, "full" to full, "showable" to true)) }.getOrNull()?.also { done[k] = it }
            }
        }
        return job.await()
    }

    /** 预先准备接下来可能打开的几张。 */
    fun prefetch(paths: List<String>) {
        paths.forEach { p -> if (!jobs.containsKey(keyOf(p, false))) scope.async { get(p) } }
    }

    /** 一张照片转过之后：它的大图换了文件，忘掉手里的，下次再要。 */
    fun forget(path: String) {
        for (full in listOf(true, false)) { val k = keyOf(path, full); jobs.remove(k); done.remove(k) }
    }

    /** 清除缓存后：之前的文件已经没了。 */
    fun forget() { jobs.clear(); done.clear() }
}
