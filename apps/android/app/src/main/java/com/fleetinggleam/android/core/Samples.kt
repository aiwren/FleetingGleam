package com.fleetinggleam.android.core

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
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
     * 把随应用带的示例分析结果（assets/samples/analysis.json，标准模型分析这十张照片的缓存记录）放进核心的缓存。
     * 放成了，打开示例项目时每一张都是"缓存里认识的"，不用先下载两百多兆的模型就能看到分析好的照片墙——
     * 新手几秒钟就能把保留 / 淘汰 / 整理走一遍（作者 2026-11-05）。带的结果和眼下的分析版本对不上时核心不认，照旧走下载。
     * 返回是不是每张示例照片都有记录了。
     */
    suspend fun seed(ctx: Context): Boolean = withContext(Dispatchers.IO) {
        runCatching {
            val dir = folder(ctx)
            val names = ctx.assets.list(DIR)?.filter { it.endsWith(".jpg") }.orEmpty()
            val json = ctx.assets.open("$DIR/analysis.json").use { it.readBytes().decodeToString() }
            val n = Core.call("seed_cache", args("dir" to dir.absolutePath, "json" to json)).trim().toInt()
            names.isNotEmpty() && n >= names.size
        }.getOrDefault(false)
    }

    /**
     * 建一个示例项目并打开（已经有一个就直接打开它）。示例项目开着"相似的只留最好的一张"，这样两组连拍一眼能看出推荐；
     * 准备页不用再点"开始分析"——它本来就是来看效果的。
     */
    fun open(ctx: Context) { scope.launch { openNow(ctx.applicationContext) } }

    /** 自己的协程域：点"用示例照片先试试"的那一页在项目建好后就离开组合了，用页面的域，等到准备页时协程早被取消，分析从来没自动开始过。 */
    private val scope = kotlinx.coroutines.CoroutineScope(kotlinx.coroutines.SupervisorJob() + Dispatchers.Main)

    private suspend fun openNow(ctx: Context) {
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
