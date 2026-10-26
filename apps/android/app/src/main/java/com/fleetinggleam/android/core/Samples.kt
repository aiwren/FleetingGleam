package com.fleetinggleam.android.core

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import java.io.File

/**
 * 示例项目：应用自带的十几张小照片（落地页用的那批，Lorem Picsum，Unsplash 授权），复制到应用自己的目录里建一个项目。
 * 给还不想授权相册的人先看一眼效果；其中两组是同一张的轻微裁切 / 模糊变体，能看到"相似的归成一组、推荐一张"。
 */
object Samples {
    private const val DIR = "samples"

    /** 这是示例项目吗（它的文件夹就是示例照片那个）。 */
    fun isSample(ctx: Context, p: Project): Boolean = p.folders == listOf(File(ctx.filesDir, DIR).absolutePath)

    /** 示例照片所在的文件夹（第一次调用时从 assets 复制出来）。 */
    suspend fun folder(ctx: Context): File = withContext(Dispatchers.IO) {
        val dir = File(ctx.filesDir, DIR)
        val names = ctx.assets.list(DIR)?.filter { it.endsWith(".jpg") }.orEmpty()
        dir.mkdirs()
        for (n in names) {
            val f = File(dir, n)
            if (!f.exists()) ctx.assets.open("$DIR/$n").use { src -> f.outputStream().use { src.copyTo(it) } }
        }
        dir
    }

    /**
     * 建一个示例项目并打开（已经有一个就直接打开它）。示例项目开着"相似的只留最好的一张"，这样两组连拍一眼能看出推荐；
     * 准备页不用再点"开始分析"——它本来就是来看效果的。
     */
    suspend fun open(ctx: Context) {
        val dir = folder(ctx)
        val existing = Projects.store.value.projects.firstOrNull { it.trashed == null && it.folders == listOf(dir.absolutePath) }
        if (existing != null) { Projects.setActive(existing.id); return }
        val p = Projects.create(t("android.start.sampleName"), listOf(dir.absolutePath))
        Projects.update(p.id) { it.copy(onePerGroup = true) }
        // 等探测完到了准备页就自动开始
        withTimeoutOrNull(15_000) { Session.phase.first { it == Phase.Ready } }
        // 示例只是看一眼效果：用不必下载或最小的那档，要下载就先在询问框里说清楚为什么、多大（审查第 24 条）
        if (Projects.store.value.activeId == p.id && Session.phase.value == Phase.Ready) Session.startScan(sample = true)
    }
}
