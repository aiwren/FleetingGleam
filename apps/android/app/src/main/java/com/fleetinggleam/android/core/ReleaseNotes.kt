package com.fleetinggleam.android.core

/**
 * 更新说明（发布时由 .github/scripts/release-notes.sh 生成，格式见 release-notes/README.md）的解析，
 * 和桌面端 apps/desktop/src/releaseNotes.ts 同一套规则：
 * - 中文在前、英文在后，中间一行 `<!-- en -->`；按界面语言只取一半，没有英文就都用中文。
 * - 只认几种块：`### 小标题`、单独一行的图片 `![说明](https://…)`、`- 列表项`、普通段落；行内只认 **加粗**。
 */
object ReleaseNotes {
    sealed interface Block {
        data class Heading(val text: String) : Block
        data class Image(val src: String, val alt: String) : Block
        data class Bullets(val items: List<String>) : Block
        data class Para(val text: String) : Block
    }

    private const val EN_MARK = "<!-- en -->"

    /** 取界面语言（"zh-CN" / "en"）那一半。 */
    fun forLang(md: String, lang: String): String {
        val i = md.indexOf(EN_MARK)
        if (i < 0) return md.trim()
        val zh = md.substring(0, i).trim()
        val en = md.substring(i + EN_MARK.length).trim()
        return if (lang.startsWith("zh")) zh else en.ifEmpty { zh }
    }

    /** 图片只认 https（调试时本机服务器的 http://127.0.0.1 / 10.0.2.2 也认）。 */
    private fun okSrc(s: String) = s.startsWith("https://") || Regex("""^http://(127\.0\.0\.1|10\.0\.2\.2)[:/]""").containsMatchIn(s)

    fun parse(md: String): List<Block> {
        val out = mutableListOf<Block>()
        val para = mutableListOf<String>()
        var list: MutableList<String>? = null
        fun flushPara() { if (para.isNotEmpty()) { out += Block.Para(para.joinToString(" ")); para.clear() } }
        fun flushList() { list?.let { out += Block.Bullets(it) }; list = null }
        val heading = Regex("""^#{1,6}\s+(.*)$""")
        val image = Regex("""^!\[([^\]]*)\]\(([^)\s]+)\)$""")
        val bullet = Regex("""^[-*]\s+(.*)$""")
        for (raw in md.replace("\r", "").lines()) {
            val line = raw.trim()
            val h = heading.find(line)
            val img = image.find(line)
            val li = bullet.find(line)
            when {
                line.isEmpty() || line.startsWith("<!--") || Regex("^-{3,}$").matches(line) -> { flushPara(); flushList() }
                h != null -> { flushPara(); flushList(); out += Block.Heading(h.groupValues[1]) }
                img != null -> { flushPara(); flushList(); if (okSrc(img.groupValues[2])) out += Block.Image(img.groupValues[2], img.groupValues[1]) }
                li != null -> { flushPara(); (list ?: mutableListOf<String>().also { list = it }) += li.groupValues[1] }
                else -> { flushList(); para += line }
            }
        }
        flushPara(); flushList()
        return out
    }

    /** 摘要：小标题列表（没有小标题就取前三段的头一句）。 */
    fun headlines(blocks: List<Block>): List<String> {
        val hs = blocks.filterIsInstance<Block.Heading>().map { it.text }
        if (hs.isNotEmpty()) return hs.map { it.replace("**", "") }
        return blocks.flatMap { when (it) { is Block.Bullets -> it.items; is Block.Para -> listOf(it.text); else -> emptyList() } }.take(3).map { it.replace("**", "") }
    }
}
