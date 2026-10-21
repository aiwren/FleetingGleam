package com.fleetinggleam.android.core

import android.Manifest
import android.content.ContentUris
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.DocumentsContract
import android.provider.MediaStore
import androidx.core.content.ContextCompat
import androidx.documentfile.provider.DocumentFile
import kotlinx.serialization.Serializable
import java.io.File
import java.security.MessageDigest

/**
 * 照片从哪来（Android 才有的部分）。项目的文件夹在两端都是一个本地路径，核心库按路径读文件；这里负责把手机上的照片变成这样的路径：
 *
 * - **相册**（MediaStore）：按相册列出（相机、截图、微信……），每个相册就是一个文件夹路径。有了"读取照片"权限，应用可以按路径直接读这些照片。
 * - **任意文件夹**（系统的文件选择器 SAF）：手机存储或 SD 卡上的文件夹换算成路径直接用；网盘之类换算不出路径的，照片复制到应用自己的镜像
 *   （`files/saf/<编号>/`），之后每次打开项目只补新增、删掉已没有的——和 NAS 的镜像一个道理。
 */
object Media {
    /** 读照片要的权限：Android 13 起是 READ_MEDIA_IMAGES，之前是 READ_EXTERNAL_STORAGE。 */
    val permissions: Array<String>
        get() = if (Build.VERSION.SDK_INT >= 33) arrayOf(Manifest.permission.READ_MEDIA_IMAGES) else arrayOf(Manifest.permission.READ_EXTERNAL_STORAGE)

    fun hasPermission(ctx: Context): Boolean = permissions.all { ContextCompat.checkSelfPermission(ctx, it) == PackageManager.PERMISSION_GRANTED }

    /** Android 14 起用户可以只给"部分照片"：这时只能看到他选的那几张。 */
    fun partialOnly(ctx: Context): Boolean = Build.VERSION.SDK_INT >= 34 &&
        ContextCompat.checkSelfPermission(ctx, Manifest.permission.READ_MEDIA_IMAGES) != PackageManager.PERMISSION_GRANTED &&
        ContextCompat.checkSelfPermission(ctx, Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED) == PackageManager.PERMISSION_GRANTED

    /** 一个相册：名字、它所在的文件夹、照片数、最新一张（封面）。 */
    data class Album(val name: String, val dir: String, val count: Int, val cover: Uri, val latest: Long)

    /** 手机上的相册，照片多的在前。 */
    fun albums(ctx: Context): List<Album> {
        val cols = arrayOf(MediaStore.Images.Media._ID, MediaStore.Images.Media.BUCKET_DISPLAY_NAME, MediaStore.Images.Media.DATA, MediaStore.Images.Media.DATE_TAKEN, MediaStore.Images.Media.DATE_MODIFIED)
        data class Acc(var name: String, val dir: String, var count: Int, var cover: Uri, var latest: Long)
        val by = LinkedHashMap<String, Acc>()
        ctx.contentResolver.query(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, cols, null, null, "${MediaStore.Images.Media.DATE_MODIFIED} DESC")?.use { c ->
            val id = c.getColumnIndexOrThrow(cols[0]); val bucket = c.getColumnIndexOrThrow(cols[1]); val data = c.getColumnIndexOrThrow(cols[2])
            val taken = c.getColumnIndexOrThrow(cols[3]); val mod = c.getColumnIndexOrThrow(cols[4])
            while (c.moveToNext()) {
                val path = c.getString(data) ?: continue
                val dir = path.substringBeforeLast('/', "")
                if (dir.isEmpty()) continue
                val uri = ContentUris.withAppendedId(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, c.getLong(id))
                val time = c.getLong(taken).takeIf { it > 0 } ?: (c.getLong(mod) * 1000)
                val a = by.getOrPut(dir) { Acc(c.getString(bucket) ?: baseName(dir), dir, 0, uri, time) }
                a.count++
                if (time > a.latest) { a.latest = time; a.cover = uri }
            }
        }
        // 直接放在存储根目录下的照片：文件夹名是"0"，显示成"手机存储"
        val root = android.os.Environment.getExternalStorageDirectory().absolutePath.trimEnd('/')
        return by.values.map { Album(if (it.dir.trimEnd('/') == root) t("android.picker.storageRoot") else it.name, it.dir, it.count, it.cover, it.latest) }.sortedByDescending { it.count }
    }

    /** 一张照片在 MediaStore 里的 Uri（回收、分享时要用）；不在相册里的（NAS 镜像、应用自己的文件）是 null。 */
    fun uriOf(ctx: Context, path: String): Uri? {
        ctx.contentResolver.query(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, arrayOf(MediaStore.Images.Media._ID), "${MediaStore.Images.Media.DATA} = ?", arrayOf(path), null)?.use { c ->
            if (c.moveToFirst()) return ContentUris.withAppendedId(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, c.getLong(0))
        }
        return null
    }

    /** 系统文件选择器选的文件夹换算成路径：手机存储是 `/storage/emulated/0/…`，SD 卡是 `/storage/<卷号>/…`；换算不出（网盘等）是 null。 */
    fun treeToPath(uri: Uri): String? {
        if (uri.authority != "com.android.externalstorage.documents") return null
        val docId = runCatching { DocumentsContract.getTreeDocumentId(uri) }.getOrNull() ?: return null
        val volume = docId.substringBefore(':')
        val rel = docId.substringAfter(':', "")
        val root = if (volume.equals("primary", true)) Environment.getExternalStorageDirectory().absolutePath else "/storage/$volume"
        return if (rel.isEmpty()) root else "$root/$rel"
    }

    /** 这个路径的照片应用能不能直接读（有读照片的权限，且文件夹列得出来）。 */
    fun readable(path: String): Boolean = File(path).let { it.isDirectory && it.listFiles() != null }

    // ------------------------------------------------------------ 换算不出路径的文件夹：复制到应用自己的镜像

    @Serializable
    data class Mirror(val id: String, val uri: String, val name: String)

    private fun mirrorsFile(ctx: Context) = File(ctx.filesDir, "saf/mirrors.json")

    fun mirrors(ctx: Context): List<Mirror> = runCatching { Fg.camel.decodeFromString<List<Mirror>>(mirrorsFile(ctx).readText()) }.getOrDefault(emptyList())

    private fun saveMirrors(ctx: Context, list: List<Mirror>) {
        val f = mirrorsFile(ctx)
        f.parentFile?.mkdirs()
        val tmp = File(f.path + ".tmp")
        tmp.writeText(Fg.camel.encodeToString(kotlinx.serialization.builtins.ListSerializer(Mirror.serializer()), list))
        tmp.renameTo(f)
    }

    /** 一个镜像文件夹的本地路径（项目的文件夹就是它）。 */
    fun mirrorDir(ctx: Context, id: String): File = File(ctx.filesDir, "saf/$id")

    /** 选了一个换算不出路径的文件夹：记下它（以后重启也能读，要先拿到持久的读权限），返回它在镜像里的路径。照片在第一次打开项目时复制。 */
    fun adoptTree(ctx: Context, uri: Uri): String {
        runCatching { ctx.contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION) }
        val id = MessageDigest.getInstance("SHA-1").digest(uri.toString().toByteArray()).joinToString("") { "%02x".format(it) }.take(12)
        val name = DocumentFile.fromTreeUri(ctx, uri)?.name ?: id
        val list = mirrors(ctx).filter { it.id != id } + Mirror(id, uri.toString(), name)
        saveMirrors(ctx, list)
        val dir = mirrorDir(ctx, id)
        dir.mkdirs()
        return dir.absolutePath
    }

    private val IMAGE_EXT = setOf("jpg", "jpeg", "jpe", "png", "webp")

    /**
     * 项目文件夹里落在镜像上的，先从原处同步：拉新增或变了的照片（按大小 + 修改时间比），删掉原处已没有的。进度经 `onProgress(done, total, name)` 报出。
     * `cancelled()` 为真就停。没有镜像的文件夹什么都不做。
     */
    fun syncMirrors(ctx: Context, dirs: List<String>, onProgress: (Int, Int, String) -> Unit, cancelled: () -> Boolean) {
        val all = mirrors(ctx)
        for (d in dirs) {
            val m = all.firstOrNull { d == mirrorDir(ctx, it.id).absolutePath || d.startsWith(mirrorDir(ctx, it.id).absolutePath + "/") } ?: continue
            val tree = DocumentFile.fromTreeUri(ctx, Uri.parse(m.uri)) ?: continue
            val root = mirrorDir(ctx, m.id)
            // 先把原处的照片列全（递归，跳过隐藏的），再比
            data class Src(val doc: DocumentFile, val rel: String)
            val found = mutableListOf<Src>()
            fun walk(f: DocumentFile, rel: String) {
                for (c in f.listFiles()) {
                    val name = c.name ?: continue
                    if (name.startsWith(".")) continue
                    if (c.isDirectory) walk(c, if (rel.isEmpty()) name else "$rel/$name")
                    else if (name.substringAfterLast('.', "").lowercase() in IMAGE_EXT && c.length() >= 8 * 1024) found.add(Src(c, if (rel.isEmpty()) name else "$rel/$name"))
                }
            }
            walk(tree, "")
            val keep = found.map { File(root, it.rel).absolutePath }.toHashSet()
            val todo = found.filter { s -> val f = File(root, s.rel); !(f.exists() && f.length() == s.doc.length() && f.lastModified() == s.doc.lastModified()) }
            var done = 0
            onProgress(0, todo.size, m.name)
            for (s in todo) {
                if (cancelled()) throw CoreException("cancelled")
                val out = File(root, s.rel)
                out.parentFile?.mkdirs()
                val part = File(out.parentFile, ".${out.name}.part")
                ctx.contentResolver.openInputStream(s.doc.uri)?.use { input -> part.outputStream().use { input.copyTo(it) } }
                part.renameTo(out)
                out.setLastModified(s.doc.lastModified())
                onProgress(++done, todo.size, m.name)
            }
            // 原处已经没有的，从镜像里删掉
            root.walkBottomUp().filter { it.isFile && it.absolutePath !in keep && it.extension.lowercase() in IMAGE_EXT }.forEach { it.delete() }
        }
    }

    /** 这个路径是不是镜像里的（这样的照片在原处回收，不进系统回收站）。 */
    fun isMirror(ctx: Context, path: String): Boolean = path.startsWith(File(ctx.filesDir, "saf").absolutePath + "/")
}
