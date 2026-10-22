@file:OptIn(ExperimentalSerializationApi::class)

package com.fleetinggleam.android.core

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNamingStrategy
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull

/** JSON 的两种读法：核心库的数据是 snake_case（Rust 的字段名），项目文件与设置是 camelCase（和桌面端前端存的一样）。 */
object Fg {
    val json = Json { ignoreUnknownKeys = true; explicitNulls = false; coerceInputValues = true; namingStrategy = JsonNamingStrategy.SnakeCase }
    // 写出默认值：项目文件与导出的设置要和桌面端同一格式（桌面端总是写全字段），默认值也得落盘
    val camel = Json { ignoreUnknownKeys = true; explicitNulls = false; encodeDefaults = true; coerceInputValues = true }
}

// 下面每个类型都和桌面端 src/types.ts 等处的同名类型一一对应（字段名按 Rust 的 serde 输出）。

/** 一张照片的分析结果（packages/core/src/analyze.rs 的 `Photo`）。 */
@Serializable
data class Photo(
    val path: String,
    val name: String = "",
    val thumb: String = "",
    val width: Int = 3,
    val height: Int = 2,
    val sharpness: Float = 0f,
    val brightness: Float = 0f,
    val clippedHigh: Float = 0f,
    val clippedLow: Float = 0f,
    val sharpScore: Float = 0f,
    val expoScore: Float = 0f,
    val techScore: Float = 0f,
    val clipScore: Float? = null,
    val sysScore: Float? = null,
    val aiScore: Float? = null,
    val prefScore: Float? = null,
    val prefDelta: Float = 0f,
    val faceCount: Int = 0,
    val faceMin: Float? = null,
    /** Openness of the most closed eyes among measurable faces (null = not measured). */
    val eyesOpen: Float? = null,
    val eyeMethod: String? = null,
    /** Scene kind: portrait, people, landscape, architecture, food, animal, object, document… (or empty). */
    val kind: String = "",
    val utility: Boolean = false,
    val flags: List<String> = emptyList(),
    val takenAt: Long? = null,
    val score: Float = 0f,
    val group: Int = 0,
    val scene: Int = 0,
    val groupBest: Boolean = false,
    /** Faces big enough to recognise: `[x, y, w, h]`, top-left origin, normalised to the photo. */
    val faceBoxes: List<List<Float>> = emptyList(),
    /** Which recurring person each of `faceBoxes` is (0 = the person in the most photos, -1 = nobody in particular). */
    val people: List<Int> = emptyList(),
    val hdr: Boolean = false,
    val motion: Boolean = false,
    /** 显示时顺时针转的角度（0 / 90 / 180 / 270）；thumb 与 width / height 已经是转过之后的。 */
    val rotation: Int = 0,
) {
    val ratio: Float get() = width.toFloat() / height.coerceAtLeast(1)
}

/** 桶里最新那份备份的说明（engine.rs 的 `BackupMeta`）；老版本做的备份没有。 */
@Serializable
data class BackupMeta(val device: String = "", val platform: String = "", val exportedAt: Long = 0, val appVersion: String = "", val styles: Int = 0, val examples: Int = 0)

/** 保留 / 待复查 / 淘汰。 */
enum class State(val key: String) {
    Keep("keep"), Review("review"), Reject("reject");
    companion object { fun of(s: String?): State? = entries.firstOrNull { it.key == s } }
}

/** A reason is a message key plus parameters, so the UI can render it in the current language. */
@Serializable
data class Reason(val k: String, val p: JsonObject? = null)

@Serializable
data class Decision(val state: String, val manual: Boolean = false, val reasons: List<Reason> = emptyList()) {
    val st: State get() = State.of(state) ?: State.Review
}

@Serializable data class ProbeFile(val path: String, val width: Int, val height: Int, val cached: Boolean)
@Serializable data class Probe(val files: List<ProbeFile> = emptyList(), val bytes: Long = 0, val uncached: Int = 0)
@Serializable data class Progress(val done: Int = 0, val total: Int = 0)
/** "scan-decide" 里的一条：分析中这张照片眼下的判定。 */
@Serializable data class Judged(val path: String, val decision: Decision)

/** 分析中一张照片走到哪一步（"scan-stage" 事件）。 */
@Serializable data class StageEvent(val path: String, val stage: String, val pct: Int = 0, val thumb: String? = null, val width: Int? = null, val height: Int? = null)

/** 上一次分析实际用了什么（"scan-info" 事件）。 */
@Serializable data class ScanInfo(val cpuOnly: Boolean = false, val clip: String? = null, val model: String? = null, val vision: Boolean = false, val cores: Int = 1)

/** 应用自己连接的一台 NAS。 */
@Serializable data class Share(val id: String, val name: String, val host: String, val user: String = "", val enabled: Boolean = true, val local: String = "", val connected: Boolean = false)
@Serializable data class NasEntry(val name: String, val dir: Boolean, val size: Long = 0)
@Serializable data class Listing(val path: String, val local: String, val entries: List<NasEntry> = emptyList())
@Serializable data class NasProgress(val share: String, val done: Int, val total: Int, val bytesDone: Long = 0, val bytesTotal: Long = 0)

@Serializable data class BackupTarget(val id: String, val name: String, val kind: String = "r2", val account: String = "", val bucket: String = "", val accessKey: String = "", val hasSecret: Boolean = false, val lastOk: Long = 0, val lastError: String? = null)
@Serializable data class BackupStatus(val configured: Boolean = false, val enabled: Boolean = false, val keep: Int = 30, val targets: List<BackupTarget> = emptyList())
@Serializable data class TasteSummary(val styles: Int = 0, val examples: Int = 0, val refs: Int = 0, val skipped: Int = 0)

@Serializable data class PrefsInfo(
    val labels: Int = 0, val keeps: Int = 0, val rejects: Int = 0, val refs: Int = 0, val refsFrom: String = "", val imported: Int = 0, val active: Boolean = false,
    val trained: Int = 0, val strength: Float = 0f, val maxStrength: Float = 0.35f, val minPerClass: Int = 3, val halfAt: Int = 29, val fullAt: Int = 81,
)
@Serializable data class StyleInfo(val id: String, val name: String, val isDefault: Boolean = false, val info: PrefsInfo = PrefsInfo())

@Serializable data class PackStatus(val id: String, val installed: Boolean, val bytes: Long = 0, val bundled: Boolean = false)
@Serializable data class ModelProgress(val pack: String = "", val file: String = "", val done: Long = 0, val total: Long = 0)

@Serializable data class Hit(val path: String, val name: String, val thumb: String, val width: Int, val height: Int, val score: Float)
@Serializable data class SearchStatus(val total: Int = 0, val ready: Int = 0, val available: Boolean = false)

@Serializable data class FileInfo(
    val format: String? = null, val bytes: Long = 0, val colorSpace: String? = null, val bitDepth: String? = null, val camera: String? = null, val lens: String? = null,
    val focal: String? = null, val aperture: String? = null, val shutter: String? = null, val iso: String? = null, val software: String? = null, val taken: String? = null,
)

@Serializable data class TrashResult(val moved: Int = 0, val failed: List<String> = emptyList(), val networkBins: List<String> = emptyList(), val deferred: List<String> = emptyList())
@Serializable data class ExportResult(val copied: Int = 0, val skipped: Int = 0, val dest: String = "", val remote: Boolean = false)
@Serializable data class Machine(val memoryGb: Int = 8, val cores: Int = 4, val accelerator: String = "cpu")
@Serializable data class SceneText(val id: Int, val text: String)

/** 一个参数值的文字（理由里的数字和字符串）。 */
fun JsonObject.str(k: String): String? = (this[k] as? JsonPrimitive)?.contentOrNull

/** 0..1 的分数写成百分制整数，和桌面端的 `pct` 一样。 */
fun pct(v: Float): Int = Math.round(v * 100)
