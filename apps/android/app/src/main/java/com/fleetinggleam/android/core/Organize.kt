package com.fleetinggleam.android.core

import android.content.Context
import android.media.MediaScannerConnection
import android.net.Uri
import android.os.Environment
import android.provider.DocumentsContract
import android.provider.MediaStore
import androidx.documentfile.provider.DocumentFile
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * 整理（桌面端 Organize.tsx 的手机版）。
 * - 导出保留的照片：复制到相册里的「惊鸿/项目名」（Pictures 下，别的相册应用也看得到），或者用户在系统里选的任意文件夹；原图不动。
 * - 把淘汰的照片移走：手机相册里的照片进系统回收站（系统会弹一个确认框，30 天内能在相册的"回收站"里恢复）；NAS 上的照片移进共享里的回收文件夹
 *   （核心库处理）；从网盘等选来的文件夹（镜像）里的照片，在原处移进根目录的「.FleetingGleam Recycle Bin/日期」，和 NAS 一样可以手动放回。
 */
object Organize {
    const val RECYCLE = ".FleetingGleam Recycle Bin"

    data class TrashOutcome(val moved: Int, val failed: Int, val bins: List<String>, val cancelled: Boolean = false)

    /** 相册里导出用的文件夹：Pictures/惊鸿/项目名（Android 11 起应用可以直接按路径在公共媒体目录里建自己的文件）。 */
    fun albumDir(projectName: String): File {
        val safe = projectName.replace(Regex("[\\\\/:*?\"<>|]"), "_").trim().ifEmpty { "FleetingGleam" }
        return File(Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_PICTURES), "${t("brand.name")}/$safe")
    }

    /** 导出到一个路径（相册目录或换算得出路径的文件夹），然后让媒体库收录新文件。`withRaw`：同名的 RAW 原片一起复制（核心展开）。 */
    suspend fun exportTo(ctx: Context, paths: List<String>, dest: String, withRaw: Boolean = false): ExportResult {
        val r = Core.get<ExportResult>("export_photos", args("paths" to paths, "dest" to dest, "withRaw" to withRaw))
        // 传到 NAS 上的不在这台手机上，不用让媒体库收录
        if (!r.remote) {
            val files = File(dest).listFiles()?.map { it.absolutePath }?.toTypedArray() ?: emptyArray()
            if (files.isNotEmpty()) MediaScannerConnection.scanFile(ctx, files, null, null)
        }
        return r
    }

    /** 正在导出到网盘的那次要不要停（"停止"按钮）；核心那边的导出用 `export_cancel`。 */
    @Volatile var treeCancel = false

    /**
     * 导出到换算不出路径的文件夹（网盘等）：逐张用 ContentResolver 复制；重名的加序号。点了"停止"就在这一张写完后停下（`cancelled`）；
     * 一张写失败了删掉写了一半的那个文件，原因记进 `failed`（审查第 35 条）。
     */
    fun exportToTree(ctx: Context, paths: List<String>, tree: Uri, onProgress: (Int, Int) -> Unit): ExportResult {
        treeCancel = false
        val dir = DocumentFile.fromTreeUri(ctx, tree) ?: return ExportResult(0, 0, tree.toString(), failed = paths.map { ExportFailure(it, "denied:tree") })
        val names = dir.listFiles().mapNotNull { it.name }.toMutableSet()
        var copied = 0
        val failed = mutableListOf<ExportFailure>()
        for ((i, p) in paths.withIndex()) {
            if (treeCancel) return ExportResult(copied, 0, dir.name ?: tree.toString(), failed = failed, cancelled = true)
            val src = File(p)
            var out: DocumentFile? = null
            val r = runCatching {
                var name = src.name
                var n = 1
                while (name in names) { name = "${src.nameWithoutExtension} ($n).${src.extension}"; n++ }
                // RAW 等没有通用图片类型的写成 octet-stream：提供者不会给它另加扩展名
                val mime = when (src.extension.lowercase()) { "png" -> "image/png"; "webp" -> "image/webp"; "heic", "heif" -> "image/heif"; "jpg", "jpeg", "jpe" -> "image/jpeg"; "dng" -> "image/x-adobe-dng"; else -> "application/octet-stream" }
                out = dir.createFile(mime, name) ?: error("denied:create")
                val written = ctx.contentResolver.openOutputStream(out!!.uri)?.use { o -> src.inputStream().use { it.copyTo(o) } } ?: error("denied:open")
                if (written != src.length()) error("short write")
                names.add(name)
            }
            if (r.isSuccess) copied++
            else {
                runCatching { out?.delete() }
                val msg = r.exceptionOrNull()?.message.orEmpty()
                // 写满了多半是空间不够（网盘的提供者给不出可用空间，只能这样猜）
                failed.add(ExportFailure(p, if (Errors.parse(msg) != null) msg else if ("ENOSPC" in msg || "No space" in msg) "space:" else msg))
            }
            onProgress(i + 1, paths.size)
        }
        return ExportResult(copied, 0, dir.name ?: tree.toString(), failed = failed)
    }

    /**
     * 把照片移走。先交给核心库（NAS 上的它自己处理）；核心库放回来的本机照片（`deferred`）：镜像里的在原处回收，
     * 相册里的一次性请求系统回收站（确认框由 `confirm` 弹出，用户拒绝就算取消）。
     */
    suspend fun trash(ctx: Context, paths: List<String>, confirm: suspend (android.content.IntentSender) -> Boolean): Pair<TrashOutcome, List<String>> {
        val r = Core.get<TrashResult>("trash_photos", args("paths" to paths))
        var moved = r.moved
        val failed = r.failed.toMutableList()
        val bins = r.networkBins.toMutableList()
        val gone = paths.filter { it !in r.failed && it !in r.deferred }.toMutableList()
        // 镜像里的：在原处移进回收文件夹
        val (mirrored, local) = r.deferred.partition { Media.isMirror(ctx, it) }
        for (p in mirrored) {
            val where = recycleMirror(ctx, p)
            if (where != null) { moved++; gone.add(p); if (where !in bins) bins.add(where) } else failed.add(p)
        }
        // 相册里的：系统回收站（一个确认框处理全部）
        val uris = local.mapNotNull { p -> Media.uriOf(ctx, p)?.let { p to it } }
        failed.addAll(local.filter { p -> uris.none { it.first == p } })
        var cancelled = false
        if (uris.isNotEmpty()) {
            val req = MediaStore.createTrashRequest(ctx.contentResolver, uris.map { it.second }, true)
            if (confirm(req.intentSender)) { moved += uris.size; gone.addAll(uris.map { it.first }) }
            else cancelled = true
        }
        return TrashOutcome(moved, failed.size, bins, cancelled) to gone
    }

    /** 镜像里的一张：在原处找到它，移进根目录的回收文件夹（按日期分开），然后删掉本机副本。返回回收文件夹的说法，失败是 null。 */
    private fun recycleMirror(ctx: Context, path: String): String? {
        val (m, rel) = Media.mirrorOf(ctx, path) ?: return null
        val treeUri = Uri.parse(m.uri)
        val root = DocumentFile.fromTreeUri(ctx, treeUri) ?: return null
        var parent: DocumentFile = root
        val parts = rel.split('/')
        for (seg in parts.dropLast(1)) parent = parent.findFile(seg) ?: return null
        val doc = parent.findFile(parts.last()) ?: return null
        val day = SimpleDateFormat("yyyy-MM-dd", Locale.ROOT).format(Date())
        val bin = (root.findFile(RECYCLE) ?: root.createDirectory(RECYCLE))?.let { it.findFile(day) ?: it.createDirectory(day) } ?: return null
        val ok = runCatching {
            // 能直接移动的提供者（大多数本地与网盘）就移动；不支持的复制过去再删
            DocumentsContract.moveDocument(ctx.contentResolver, doc.uri, parent.uri, bin.uri) != null
        }.getOrDefault(false) || copyThenDelete(ctx, doc, bin, parts.last())
        if (!ok) return null
        File(path).delete()
        return "${m.name}/$RECYCLE/$day"
    }

    /**
     * 不支持直接移动的提供者：复制进回收文件夹，**确认写进去的字节数等于原图大小**才删原图。输入 / 输出流打不开、
     * 复制到一半出错或字节数对不上，都删掉回收夹里那份残缺的、原图不动，算失败（以前流是 null 也照删原图，原图就没了）。
     */
    private fun copyThenDelete(ctx: Context, doc: DocumentFile, bin: DocumentFile, fallbackName: String): Boolean {
        val out = runCatching { bin.createFile(doc.type ?: "image/jpeg", doc.name ?: fallbackName) }.getOrNull() ?: return false
        val expected = doc.length()
        val written = runCatching {
            ctx.contentResolver.openInputStream(doc.uri)?.use { i -> ctx.contentResolver.openOutputStream(out.uri)?.use { o -> i.copyTo(o) } }
        }.getOrNull()
        // length() 取不到时是 0：这时只要求确实写进了东西
        val ok = written != null && written > 0 && (expected <= 0 || written == expected)
        if (!ok) { runCatching { out.delete() }; return false }
        // 原图删不掉：回收夹里那份也撤掉，免得同一张出现两份
        if (!runCatching { doc.delete() }.getOrDefault(false)) { runCatching { out.delete() }; return false }
        return true
    }

    /** 评分 / 标签会写到哪里：JPEG 写进文件，RAW / TIFF 写旁边的 .xmp，其它格式写不了。和核心的 `xmp::carrier_of`、桌面端 carrierOf 一致。 */
    fun carrierOf(path: String): String {
        val ext = path.substringAfterLast('.', "").lowercase()
        return when (ext) {
            "jpg", "jpeg", "jpe" -> "jpeg"
            "heic", "heif", "png", "webp", "gif", "bmp", "avif", "jxl" -> "none"
            else -> "sidecar"
        }
    }

    /**
     * 写回只对这台手机上的原图有意义：NAS 上的和从网盘等选来的文件夹里的照片，手里只是应用私有目录里的一份副本，
     * 写进副本 Lightroom 看不到，下次同步还会被盖掉，所以跳过（桌面端直接挂载共享，没有这个区别）。
     */
    fun isCopy(ctx: Context, path: String): Boolean = path.startsWith(ctx.applicationInfo.dataDir + "/") || path.startsWith(ctx.filesDir.absolutePath + "/") || Media.isMirror(ctx, path)

    /** 一张要写回的照片：路径、判定、应用的评分（0..1）和手动星级（有就优先）。 */
    data class MarkItem(val path: String, val state: String, val score: Float, val stars: Int?)

    /**
     * 写回 Lightroom / Bridge（桌面端 Organize.tsx 的 marks）：保留 → 星级 + 绿色标签，待复查 → 黄色，淘汰 → 拒绝 + 红色。
     * 改写过的 JPEG 让媒体库重新收录（核心是换文件写的，相册里那条记录要跟上）。
     */
    suspend fun writeMarks(ctx: Context, items: List<MarkItem>, stars: Boolean, labels: Boolean): MarkResult {
        val list = items.map { mapOf("path" to it.path, "state" to it.state, "score" to it.score, "stars" to it.stars) }
        val r = Core.get<MarkResult>("write_marks", args("items" to list, "stars" to stars, "labels" to labels))
        rescanJpegs(ctx, items.map { it.path }.filter { carrierOf(it) == "jpeg" && it !in r.skipped && r.failed.none { f -> f.path == it } })
        return r
    }

    /** 撤销整次写回：写前的原样（`MarkResult.undo`）原样交回核心。 */
    suspend fun undoMarks(ctx: Context, before: List<kotlinx.serialization.json.JsonElement>): MarkUndo {
        val r = Core.get<MarkUndo>("undo_marks", args("before" to before))
        rescanJpegs(ctx, r.restored.filter { carrierOf(it) == "jpeg" })
        return r
    }

    private fun rescanJpegs(ctx: Context, paths: List<String>) {
        val local = paths.filter { File(it).exists() && !Media.isMirror(ctx, it) }
        if (local.isNotEmpty()) MediaScannerConnection.scanFile(ctx, local.toTypedArray(), null, null)
    }

    /** 照片已不在了：从项目的改判里忘掉（和桌面端一样）。 */
    fun forget(projectId: String, gone: List<String>) {
        if (gone.isEmpty()) return
        val set = gone.toSet()
        Projects.update(projectId) { p -> p.copy(overrides = p.overrides.filterKeys { it !in set }, stars = p.stars.filterKeys { it !in set }) }
    }

}
