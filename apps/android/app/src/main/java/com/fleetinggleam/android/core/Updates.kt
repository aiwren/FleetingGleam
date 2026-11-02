package com.fleetinggleam.android.core

import android.content.Context
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.net.Uri
import android.provider.Settings as SysSettings
import androidx.core.content.FileProvider
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import java.io.File
import java.net.HttpURLConnection
import java.net.URL

/**
 * 应用内更新（和桌面端 src/updater.ts 同一套状态与文案）。没有应用商店：版本只来自 GitHub Releases——
 * 读 `releases/latest` 这个公开接口，拿 tag 和当前 versionName 比；新的就把固定名字的 `FleetingGleam-android.apk`
 * 下到应用私有缓存里，再经 FileProvider 交给系统安装器（要用户先允许"安装未知应用"）。
 *   idle → checking → latest / failed / found → downloading → ready（APK 已在本机，等用户点"安装"）/ error
 * 启动后几秒静默查一次（设置里可关）：离线、GitHub 不通都安静地回到 idle；只有手动点"检查更新"才会说"没查到"。
 */
object Updates {
    private const val REPO = "aiwren/FleetingGleam"
    private const val API = "https://api.github.com/repos/$REPO/releases/latest"
    private const val APK_NAME = "FleetingGleam-android.apk"

    /** 新版本的信息（给设置页"关于"显示）。 */
    data class Info(val version: String, val current: String, val notes: String?, val url: String, val size: Long)

    sealed class State {
        object Idle : State()
        object Checking : State()
        object Latest : State()
        object Failed : State()
        data class Found(val info: Info) : State()
        data class Downloading(val info: Info, val done: Long, val total: Long) : State()
        /** APK 已经下好在 `file`，等用户点"安装"（系统会再问一次）。 */
        data class Ready(val info: Info, val file: File) : State()
        data class Error(val info: Info, val error: String) : State()
    }

    private val _state = MutableStateFlow<State>(State.Idle)
    val state: StateFlow<State> = _state
    private lateinit var app: Context
    private var current = "0.0.0"
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var scheduled = false

    fun init(ctx: Context) {
        app = ctx.applicationContext
        current = runCatching { ctx.packageManager.getPackageInfo(ctx.packageName, 0).versionName }.getOrNull() ?: "0.0.0"
        // 上个版本装好之后，缓存里那份 APK 就没用了（比当前版本新的留着：下过没装，下次不用再下）
        scope.launch {
            dir().listFiles()?.forEach { f -> if (compare(versionOf(f.name) ?: "0", current) <= 0) f.delete() }
        }
    }

    /** 启动后 6 秒静默检查一次（设置"自动检查更新"关着就不查）。进程里只安排一次。 */
    fun scheduleAutoCheck() {
        if (scheduled) return
        scheduled = true
        scope.launch { delay(6000); if (Settings.value.autoUpdate) check(silent = true) }
    }

    /** 问一次 GitHub 有没有新版本。`silent`（自动检查）查不到就当没事；手动点的才显示"没查到"。 */
    suspend fun check(silent: Boolean = false) {
        val s = _state.value
        if (s is State.Checking || s is State.Downloading) return
        if (s is State.Ready) return
        _state.value = State.Checking
        val found = withContext(Dispatchers.IO) { runCatching { fetchLatest() } }
        _state.value = found.fold(
            onSuccess = { info ->
                if (info == null) State.Latest
                else {
                    // 这个版本以前下过而且没下坏：直接可装
                    val cached = File(dir(), "FleetingGleam-${info.version}.apk")
                    if (cached.isFile && cached.length() == info.size && info.size > 0) State.Ready(info, cached) else State.Found(info)
                }
            },
            onFailure = { if (silent) State.Idle else State.Failed },
        )
    }

    /** 下载到应用私有缓存（进度写进 state）。下完进入 Ready；没下完的 `.part` 留着，下次接着下。 */
    fun download() {
        val info = when (val s = _state.value) { is State.Found -> s.info; is State.Error -> s.info; else -> return }
        _state.value = State.Downloading(info, 0, info.size)
        scope.launch {
            val out = File(dir(), "FleetingGleam-${info.version}.apk")
            val part = File(dir(), "FleetingGleam-${info.version}.apk.part")
            runCatching {
                var have = if (part.isFile) part.length() else 0L
                if (have >= info.size && info.size > 0) { part.delete(); have = 0 }
                val conn = open(info.url)
                if (have > 0) conn.setRequestProperty("Range", "bytes=$have-")
                conn.connect()
                val code = conn.responseCode
                if (code != 200 && code != 206) throw IllegalStateException("HTTP $code")
                if (code == 200) have = 0
                val total = if (info.size > 0) info.size else conn.contentLengthLong.let { if (code == 206) it + have else it }
                var done = have
                var lastShown = 0L
                conn.inputStream.use { input ->
                    java.io.FileOutputStream(part, code == 206).use { output ->
                        val buf = ByteArray(256 * 1024)
                        while (true) {
                            val n = input.read(buf); if (n < 0) break
                            output.write(buf, 0, n); done += n
                            // 界面一秒刷十次就够
                            if (done - lastShown > 512 * 1024 || done == total) { lastShown = done; _state.value = State.Downloading(info, done, total) }
                        }
                    }
                }
                if (info.size > 0 && done != info.size) throw IllegalStateException("size mismatch: $done / ${info.size}")
                if (!part.renameTo(out)) throw IllegalStateException("rename failed")
                out
            }.fold(
                onSuccess = { _state.value = State.Ready(info, it) },
                onFailure = { _state.value = State.Error(info, it.message ?: it.javaClass.simpleName) },
            )
        }
    }

    /** 能直接拉起系统安装器吗；不能的话要先去系统设置里允许"安装未知应用"（`openInstallPermission`）。 */
    fun canInstall(): Boolean = app.packageManager.canRequestPackageInstalls()

    fun openInstallPermission(ctx: Context) {
        runCatching { ctx.startActivity(Intent(SysSettings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${app.packageName}")).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
    }

    /** 把下好的 APK 交给系统安装器（系统会显示版本并再问一次）。装完应用会被系统重启成新版本。 */
    fun install(ctx: Context): Boolean {
        val s = _state.value as? State.Ready ?: return false
        val uri = FileProvider.getUriForFile(app, "${app.packageName}.updates", s.file)
        val intent = Intent(Intent.ACTION_VIEW).setDataAndType(uri, "application/vnd.android.package-archive")
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_GRANT_READ_URI_PERMISSION)
        return runCatching { ctx.startActivity(intent) }.isSuccess
    }

    // ---------------------------------------------------------------- GitHub

    @Serializable private data class Asset(val name: String, val browser_download_url: String, val size: Long = 0)
    @Serializable private data class Release(val tag_name: String, val body: String? = null, val draft: Boolean = false, val prerelease: Boolean = false, val assets: List<Asset> = emptyList())
    private val json = kotlinx.serialization.json.Json { ignoreUnknownKeys = true; coerceInputValues = true }

    /** `null` = 已是最新（或最新的 Release 里没有安卓包）。 */
    private fun fetchLatest(): Info? {
        val conn = open(apiUrl())
        conn.setRequestProperty("Accept", "application/vnd.github+json")
        conn.connect()
        if (conn.responseCode != 200) throw IllegalStateException("HTTP ${conn.responseCode}")
        val rel = conn.inputStream.bufferedReader().use { json.decodeFromString(Release.serializer(), it.readText()) }
        val version = rel.tag_name.removePrefix("v")
        if (rel.draft || compare(version, current) <= 0) return null
        val apk = rel.assets.firstOrNull { it.name == APK_NAME } ?: return null
        return Info(version, current, notesOf(rel.body), apk.browser_download_url, apk.size)
    }

    private fun open(url: String): HttpURLConnection = (URL(url).openConnection() as HttpURLConnection).apply {
        connectTimeout = 15_000; readTimeout = 30_000; instanceFollowRedirects = true
        setRequestProperty("User-Agent", "FleetingGleam-android/$current")
    }

    /** 只有可调试的包可以把接口指到别处（本地假的 release JSON）：把地址写进 files/update_url。正式包只认 GitHub。 */
    private fun apiUrl(): String {
        val debuggable = app.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0
        if (!debuggable) return API
        return File(app.filesDir, "update_url").takeIf { it.isFile }?.readText()?.trim()?.ifEmpty { null } ?: API
    }

    /** Release 正文里"## 更新内容"之后的那一段（发布工作流从 CHANGELOG 摘的）；没有这个标题就整段。 */
    private fun notesOf(body: String?): String? {
        val b = body?.replace("\r", "")?.trim().takeUnless { it.isNullOrEmpty() } ?: return null
        val i = b.indexOf("## 更新内容")
        if (i < 0) return b
        val rest = b.substring(i).lines().drop(1).joinToString("\n").trim()
        // 末尾那行"完整记录 / Full history"链接不算说明
        return rest.lines().filterNot { it.startsWith("完整记录") }.joinToString("\n").trim().ifEmpty { null }
    }

    private fun dir(): File = File(app.cacheDir, "updates").apply { mkdirs() }

    private fun versionOf(fileName: String): String? = Regex("""FleetingGleam-(\d+(?:\.\d+)*)""").find(fileName)?.groupValues?.get(1)

    /** 按数字逐段比（"0.2.0" > "0.1.10"）；预发布后缀（-beta.1）不参与。 */
    fun compare(a: String, b: String): Int {
        fun parts(v: String) = v.substringBefore('-').split('.').map { it.toIntOrNull() ?: 0 }
        val pa = parts(a); val pb = parts(b)
        for (i in 0 until maxOf(pa.size, pb.size)) {
            val d = (pa.getOrNull(i) ?: 0).compareTo(pb.getOrNull(i) ?: 0)
            if (d != 0) return d
        }
        return 0
    }
}
