package com.fleetinggleam.android.core

/**
 * 核心库对用户可能碰到的错误返回带前缀的字符串（第二轮接口约定的错误码表）：`space:<需要字节>:<可用字节>`、`verify:…`、`net:…`、
 * `auth:…`、`denied:…`、`cancelled`。这里把前缀换成一句中文说明，冒号后的细节留给"详情"。没有前缀的（老错误、别处来的）原样返回 null，
 * 界面照旧用自己的说法。
 */
object Errors {
    /** 一个错误的分类：`kind` 是前缀（"space"、"net"……），`detail` 是冒号后面的部分。 */
    data class Coded(val kind: String, val detail: String)

    private val KINDS = setOf("space", "verify", "net", "auth", "denied")

    fun parse(err: String?): Coded? {
        val s = err?.trim() ?: return null
        if (s == "cancelled") return Coded("cancelled", "")
        val i = s.indexOf(':')
        if (i <= 0) return null
        val kind = s.substring(0, i)
        return if (kind in KINDS) Coded(kind, s.substring(i + 1).trim()) else null
    }

    fun isCancelled(err: String?): Boolean = parse(err)?.kind == "cancelled"

    /** 给人看的一句说明；不认识的前缀（或没有前缀）返回 null。取消返回 null（取消不提示）。 */
    fun explain(err: String?): String? {
        val c = parse(err) ?: return null
        return when (c.kind) {
            "space" -> {
                val (need, avail) = c.detail.split(':').let { it.getOrNull(0)?.toLongOrNull() to it.getOrNull(1)?.toLongOrNull() }
                if (need != null && avail != null) t("android.err.space", "need" to fmtBytes(need), "avail" to fmtBytes(avail)) else t("android.err.spaceShort")
            }
            "verify" -> t("android.err.verify")
            "net" -> t("android.err.net")
            "auth" -> t("android.err.auth")
            "denied" -> t("android.err.denied")
            else -> null
        }
    }

    /** "详情"里那一段：有前缀的只留冒号后面（空间不够的已经在说明里了，不再重复）。 */
    fun detail(err: String?): String {
        val c = parse(err) ?: return err ?: ""
        return if (c.kind == "space" || c.kind == "cancelled") "" else c.detail
    }
}
