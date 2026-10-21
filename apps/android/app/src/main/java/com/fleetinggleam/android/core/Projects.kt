package com.fleetinggleam.android.core

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.Serializable

/** A project is a named set of folders plus the decisions made on its photos. Analysis results live in the engine's cache. */
@Serializable
data class Project(
    val id: String,
    val name: String,
    val folders: List<String>,
    val overrides: Map<String, String> = emptyMap(),
    /** Legacy: the old keep quota. Ignored now; see `keepCap`. */
    val keepRatio: Float = 0.3f,
    /** At most this share of the photos is kept (a ceiling for when you want fewer). Unset = keep every good photo. */
    val keepCap: Float? = null,
    /** true = of a group of similar shots only the best can be kept. Off by default. */
    val onePerGroup: Boolean? = null,
    val count: Int = 0,
    /** Up to four thumbnail paths shown on the project card. */
    val cover: List<String>? = null,
    /** Default sort for this project: "group" | "order" | "time" | "score". */
    val sort: String? = null,
    /** Which selection style judges this project (unset = the default style). */
    val style: String? = null,
    /** "diverse" (default) spreads keeps across scenes, "best" just takes the top scores. */
    val pickMode: String? = null,
    /** Free text the user keeps about the shoot. */
    val note: String? = null,
    /** Search on or off for this project; when unset it follows the app setting. */
    val search: Boolean? = null,
    /** false = this project ignores the learned taste model. */
    val learn: Boolean? = null,
    val updated: Long = 0,
    /** Set when the project is in the recycle bin (ms timestamp). Its photo files are never touched. */
    val trashed: Long? = null,
)

@Serializable
data class ProjectsFile(val projects: List<Project> = emptyList(), val activeId: String? = null)

/**
 * Whether the projects have been read yet, and what went wrong: `saveError` is the last write that failed, `loadError` is "corrupt" when the file
 * was there but unreadable (set aside, started empty) or the engine's message when it could not be read at all; then `readOnly` is true and nothing
 * is written this session, so the file is not replaced.
 */
data class ProjectsStatus(val loaded: Boolean = false, val saveError: String? = null, val loadError: String? = null, val readOnly: Boolean = false)

/**
 * 项目（和桌面端 src/projects.ts 一样）：存在引擎管的 `projects.json` 里，整份原子写入；格式也一样，只是路径是这台手机上的。
 * 回收站只保留最近 TRASH_DAYS 天删的项目；项目里的照片文件从来不动。
 */
object Projects {
    const val TRASH_DAYS = 7
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val _store = MutableStateFlow(ProjectsFile())
    val store: StateFlow<ProjectsFile> = _store
    private val _status = MutableStateFlow(ProjectsStatus())
    val status: StateFlow<ProjectsStatus> = _status

    private fun parse(raw: String): Pair<ProjectsFile, Boolean>? = runCatching {
        val v = Fg.camel.decodeFromString<ProjectsFile>(raw)
        val cutoff = System.currentTimeMillis() - TRASH_DAYS * 86_400_000L
        val projects = v.projects.filter { it.trashed == null || it.trashed > cutoff }
        val active = if (projects.any { it.id == v.activeId && it.trashed == null }) v.activeId else null
        ProjectsFile(projects, active) to (projects.size != v.projects.size)
    }.getOrNull()

    /** Reads the projects; called once at start-up. Nothing that shows projects renders before it is done. */
    suspend fun load() {
        var got: Pair<ProjectsFile, Boolean>? = null
        var loadError: String? = null
        var readOnly = false
        try {
            val raw = Fg.json.decodeFromString<String?>(Core.call("projects_load"))
            if (raw != null) {
                got = parse(raw)
                if (got == null) {
                    // the file is there but is not a projects store: set it aside for repair by hand, start empty
                    try { Core.call("projects_quarantine"); loadError = "corrupt" } catch (e: Exception) { loadError = e.message; readOnly = true }
                }
            }
        } catch (e: Exception) {
            // the file could not be read at all: nothing is written this session, or the next write would replace it
            loadError = e.message
            readOnly = true
        }
        _store.value = got?.first ?: ProjectsFile()
        _status.value = ProjectsStatus(loaded = true, loadError = loadError, readOnly = readOnly)
        if (!readOnly && got?.second == true) persist()
    }

    /* Writes go one at a time: a change made while one is in flight is written once it is done, as the latest state. */
    private val writing = Mutex()
    @Volatile private var again = false
    private fun persist() {
        val st = _status.value
        if (st.readOnly) {
            if (st.saveError != st.loadError) _status.value = st.copy(saveError = st.loadError)
            return
        }
        again = true
        scope.launch {
            writing.withLock {
                if (!again) return@withLock
                try {
                    while (again) {
                        again = false
                        Core.call("projects_save", args("json" to Fg.camel.encodeToString(ProjectsFile.serializer(), _store.value)))
                    }
                    if (_status.value.saveError != null) _status.value = _status.value.copy(saveError = null)
                } catch (e: Exception) {
                    // the state on screen is still right; the person is told, and the next change tries again
                    _status.value = _status.value.copy(saveError = e.message)
                }
            }
        }
    }

    private fun commit(next: ProjectsFile) {
        _store.value = next
        persist()
    }

    val value: ProjectsFile get() = _store.value
    fun get(id: String?): Project? = _store.value.projects.firstOrNull { it.id == id }

    fun create(name: String, folders: List<String>): Project {
        val p = Project(id = "p" + java.lang.Long.toString(System.currentTimeMillis(), 36) + java.lang.Long.toString((Math.random() * 1.6e6).toLong(), 36), name = name, folders = folders, updated = System.currentTimeMillis())
        commit(ProjectsFile(_store.value.projects + p, p.id))
        return p
    }

    fun update(id: String, patch: (Project) -> Project) {
        commit(_store.value.copy(projects = _store.value.projects.map { if (it.id == id) patch(it).copy(updated = System.currentTimeMillis()) else it }))
    }

    /** Soft delete: the project moves to the recycle bin and can be restored. */
    fun trash(id: String) {
        val s = _store.value
        commit(ProjectsFile(s.projects.map { if (it.id == id) it.copy(trashed = System.currentTimeMillis()) else it }, if (s.activeId == id) null else s.activeId))
    }

    fun restore(id: String) {
        commit(_store.value.copy(projects = _store.value.projects.map { if (it.id == id) it.copy(trashed = null) else it }))
    }

    /** Permanent removal of the project record (never the photos on disk). */
    fun purge(id: String) {
        val s = _store.value
        commit(ProjectsFile(s.projects.filter { it.id != id }, if (s.activeId == id) null else s.activeId))
    }

    fun emptyTrash() {
        commit(_store.value.copy(projects = _store.value.projects.filter { it.trashed == null }))
    }

    fun setActive(id: String?) {
        if (_store.value.activeId != id) commit(_store.value.copy(activeId = id))
    }

    /** Background clean-up for a long-running session: drops recycle-bin items older than TRASH_DAYS. */
    fun purgeExpired() {
        val cutoff = System.currentTimeMillis() - TRASH_DAYS * 86_400_000L
        if (_store.value.projects.any { it.trashed != null && it.trashed <= cutoff }) {
            commit(_store.value.copy(projects = _store.value.projects.filter { it.trashed == null || it.trashed > cutoff }))
        }
    }

    fun daysLeft(p: Project): Int = maxOf(1, Math.ceil(TRASH_DAYS - (System.currentTimeMillis() - (p.trashed ?: 0)) / 86_400_000.0).toInt())
}

/** 文件夹路径的最后一级（项目的默认名字）。 */
fun baseName(p: String): String = p.trimEnd('/').substringAfterLast('/').ifEmpty { p }
