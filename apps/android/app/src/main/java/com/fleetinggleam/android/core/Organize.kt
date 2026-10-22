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

    /** 导出到一个路径（相册目录或换算得出路径的文件夹），然后让媒体库收录新文件。 */
    suspend fun exportTo(ctx: Context, paths: List<String>, dest: String): ExportResult {
        val r = Core.get<ExportResult>("export_photos", args("paths" to paths, "dest" to dest))
        // 传到 NAS 上的不在这台手机上，不用让媒体库收录
        if (!r.remote) {
            val files = File(dest).listFiles()?.map { it.absolutePath }?.toTypedArray() ?: emptyArray()
            if (files.isNotEmpty()) MediaScannerConnection.scanFile(ctx, files, null, null)
        }
        return r
    }

    /** 导出到换算不出路径的文件夹（网盘等）：逐张用 ContentResolver 复制；重名的加序号。 */
    fun exportToTree(ctx: Context, paths: List<String>, tree: Uri, onProgress: (Int, Int) -> Unit): ExportResult {
        val dir = DocumentFile.fromTreeUri(ctx, tree) ?: return ExportResult(0, paths.size, tree.toString())
        val names = dir.listFiles().mapNotNull { it.name }.toMutableSet()
        var copied = 0; var skipped = 0
        paths.forEachIndexed { i, p ->
            val src = File(p)
            val ok = runCatching {
                var name = src.name
                var n = 1
                while (name in names) { name = "${src.nameWithoutExtension} ($n).${src.extension}"; n++ }
                val mime = when (src.extension.lowercase()) { "png" -> "image/png"; "webp" -> "image/webp"; "heic", "heif" -> "image/heif"; else -> "image/jpeg" }
                val out = dir.createFile(mime, name) ?: error("create")
                ctx.contentResolver.openOutputStream(out.uri)?.use { o -> src.inputStream().use { it.copyTo(o) } } ?: error("open")
                names.add(name)
            }.isSuccess
            if (ok) copied++ else skipped++
            onProgress(i + 1, paths.size)
        }
        return ExportResult(copied, skipped, dir.name ?: tree.toString())
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
        }.getOrDefault(false) || runCatching {
            val out = bin.createFile(doc.type ?: "image/jpeg", doc.name ?: parts.last()) ?: return@runCatching false
            ctx.contentResolver.openInputStream(doc.uri)?.use { i -> ctx.contentResolver.openOutputStream(out.uri)?.use { i.copyTo(it) } }
            doc.delete()
        }.getOrDefault(false)
        if (!ok) return null
        File(path).delete()
        return "${m.name}/$RECYCLE/$day"
    }

    /** 照片已不在了：从项目的改判里忘掉（和桌面端一样）。 */
    fun forget(projectId: String, gone: List<String>) {
        if (gone.isEmpty()) return
        val set = gone.toSet()
        Projects.update(projectId) { p -> p.copy(overrides = p.overrides.filterKeys { it !in set }) }
    }

}
