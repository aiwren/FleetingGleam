package com.fleetinggleam.android.core

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject

private val io = CoroutineScope(SupervisorJob() + Dispatchers.IO)

/** One learned taste ("style"): its own labels, reference photos and model (same as the Mac's src/styles.ts). */
object Styles {
    private val _list = MutableStateFlow<List<StyleInfo>>(emptyList())
    val list: StateFlow<List<StyleInfo>> = _list
    private val _revision = MutableStateFlow(0)
    /** Bumped whenever a learned taste changes, so open projects can offer to re-evaluate. */
    val revision: StateFlow<Int> = _revision
    private val subs = mutableListOf<() -> Unit>()

    /** 风格或喜好变化的通知（云备份靠它知道该上传了）。 */
    fun subscribe(f: () -> Unit) { subs.add(f) }
    fun bumpRevision() { _revision.value += 1; subs.forEach { it() } }
    private fun set(v: List<StyleInfo>) { _list.value = v; subs.forEach { it() } }

    suspend fun refresh() { runCatching { set(Core.get("styles_list")) } }

    /** Creates a style and returns its id (the one that was not in the list before), or null if it could not be made. */
    suspend fun create(name: String): String? = runCatching {
        val before = _list.value.map { it.id }.toSet()
        set(Core.get("style_create", args("name" to name)))
        _list.value.firstOrNull { it.id !in before }?.id
    }.getOrNull()

    suspend fun rename(id: String, name: String) { runCatching { set(Core.get("style_rename", args("id" to id, "name" to name))) } }
    suspend fun delete(id: String) { runCatching { set(Core.get("style_delete", args("id" to id))); bumpRevision() } }
    suspend fun setDefault(id: String) { runCatching { set(Core.get("style_set_default", args("id" to id))); bumpRevision() } }

    /** Learns a style from a folder of past picks. `onProgress` gets 0..1. */
    suspend fun learnFromFolder(dir: String, style: String?, onProgress: (Float) -> Unit): Boolean {
        val off = Core.listen("reference-progress") { raw ->
            val p = runCatching { Fg.json.decodeFromString<Progress>(raw) }.getOrNull() ?: return@listen
            onProgress(if (p.total > 0) p.done.toFloat() / p.total else 1f)
        }
        return try {
            Core.call("learn_from_folder", args("dir" to dir, "style" to style, "compute" to "auto", "quality" to Strategy.currentTier()))
            refresh()
            bumpRevision()
            true
        } catch (_: Exception) {
            false
        } finally {
            off()
        }
    }

    suspend fun reset(id: String?) {
        runCatching { Core.call("prefs_reset", args("style" to id)); refresh(); bumpRevision() }
    }

    suspend fun prefsInfo(style: String? = null): PrefsInfo? = runCatching { Core.get<PrefsInfo>("prefs_info", args("style" to style)) }.getOrNull()
}

/**
 * 应用自己连接的 NAS（和桌面端 src/nas.ts 一样）。选文件夹时：有 NAS 就先问"本机还是哪个共享"，由界面里的 FolderPicker 来问，
 * 调用方只管 `Nas.pick()`，拿到的是一个本地路径（本机文件夹，或共享镜像里的文件夹）。
 */
object Nas {
    private val _shares = MutableStateFlow<List<Share>>(emptyList())
    val shares: StateFlow<List<Share>> = _shares
    private fun set(v: List<Share>) { _shares.value = v }

    fun init() {
        // 引擎启动时在后台连 NAS，连完会说一声；设置页的状态跟着刷新
        Core.listen("nas-changed") { io.launch { refresh() } }
        io.launch { refresh() }
    }

    fun enabled(): List<Share> = _shares.value.filter { it.enabled }
    suspend fun refresh() { runCatching { set(Core.get("nas_list")) } }
    /** 马上连接；连不上就什么都不记，抛出原因。 */
    suspend fun add(name: String, host: String, user: String, password: String, remember: Boolean) = set(Core.get("nas_add", args("name" to name, "host" to host, "user" to user, "password" to password, "remember" to remember)))
    suspend fun update(id: String, name: String, host: String, user: String, password: String, remember: Boolean) = set(Core.get("nas_update", args("id" to id, "name" to name, "host" to host, "user" to user, "password" to password, "remember" to remember)))
    suspend fun remove(id: String) = set(Core.get("nas_remove", args("id" to id)))
    suspend fun connect(id: String, password: String? = null) = set(Core.get("nas_connect", args("id" to id, "password" to password)))
    suspend fun disconnect(id: String) = set(Core.get("nas_disconnect", args("id" to id)))
    suspend fun setEnabled(id: String, enabled: Boolean) = set(Core.get("nas_set_enabled", args("id" to id, "enabled" to enabled)))
    /** 添加 NAS 时的"测试连接"：连上这台服务器，返回它的共享名；什么都不记。 */
    suspend fun probe(host: String, user: String, password: String, id: String? = null): List<String> = Core.get("nas_probe", args("host" to host, "user" to user, "password" to password, "id" to id))
    /** 共享里一个文件夹（`/` 分隔，根是空串）的内容，以及它在镜像里的本地路径。 */
    suspend fun browse(id: String, path: String): Listing = Core.get("nas_browse", args("id" to id, "path" to path))

    /** 选文件夹的请求：界面里的 FolderPicker 看到它就出现，回答之后结束。`shareId` 直接进到某个共享（设置页"从这里新建项目"）。 */
    data class PickRequest(val shareId: String?, val resolve: CompletableDeferred<String?>)
    private val _request = MutableStateFlow<PickRequest?>(null)
    val request: StateFlow<PickRequest?> = _request

    suspend fun pick(shareId: String? = null): String? {
        _request.value?.resolve?.complete(null)
        val d = CompletableDeferred<String?>()
        _request.value = PickRequest(shareId, d)
        return d.await()
    }

    /** FolderPicker 回答之后调用：`dir` 为 null 表示取消。 */
    fun answer(dir: String?) {
        val r = _request.value
        _request.value = null
        r?.resolve?.complete(dir)
    }
}

/** 云备份（和桌面端 src/backup.ts 一样）：设置或喜好变了之后，静默一会儿再上传一次；失败了不打扰，状态里能看到。 */
object Backup {
    private val _status = MutableStateFlow<BackupStatus?>(null)
    val status: StateFlow<BackupStatus?> = _status
    private const val QUIET_MS = 20_000L
    private var timer: Job? = null
    @Volatile private var busy = false
    @Volatile private var again = false
    private var watching = false

    suspend fun refresh() { runCatching { _status.value = Core.get("backup_status") } }
    /** 添加（没有 id）或修改一个目标；先连一次桶，连不上抛出原因、什么都不改。修改时 secret 留空就沿用。 */
    suspend fun saveTarget(id: String?, name: String, account: String, bucket: String, accessKey: String, secret: String) {
        _status.value = Core.get("backup_save_target", args("id" to id, "name" to name, "kind" to "r2", "account" to account, "bucket" to bucket, "access_key" to accessKey, "secret" to secret.ifEmpty { null }))
    }
    suspend fun removeTarget(id: String) { _status.value = Core.get("backup_remove_target", args("id" to id)) }
    suspend fun setOptions(enabled: Boolean? = null, keep: Int? = null) { _status.value = Core.get("backup_set_options", args("enabled" to enabled, "keep" to keep)) }
    /** 备份一次到每一个目标；"立即备份"和自动触发走的是同一套。 */
    suspend fun now() { _status.value = Core.get("backup_now", args("settings" to Settings.asJson())) }
    /** 从某个目标的最新备份恢复：返回摘要与备份里带的设置。 */
    suspend fun restore(id: String): Pair<TasteSummary, JsonObject?> = tastePair(Core.call("backup_restore", args("id" to id)))

    private fun schedule() {
        val s = _status.value ?: return
        if (!s.configured || !s.enabled) return
        timer?.cancel()
        timer = io.launch { delay(QUIET_MS); run() }
    }

    private suspend fun run() {
        if (busy) { again = true; return }
        busy = true
        try { now() } catch (_: Exception) { refresh() }
        busy = false
        if (again) { again = false; schedule() }
    }

    /** 应用启动时调一次：读状态，并开始盯着设置和喜好的变化。 */
    fun watch() {
        if (watching) return
        watching = true
        io.launch { refresh() }
        Settings.subscribe { schedule() }
        Styles.subscribe { schedule() }
    }
}

/** `[summary, settings]` 这种两项数组（导入、恢复的结果）。 */
fun tastePair(raw: String): Pair<TasteSummary, JsonObject?> {
    val arr = Fg.json.parseToJsonElement(raw) as JsonArray
    return Fg.json.decodeFromJsonElement(TasteSummary.serializer(), arr[0]) to (arr.getOrNull(1) as? JsonObject)
}
