package com.fleetinggleam.android.core

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull
import java.util.Locale

/**
 * 文案：和桌面端同一份（apps/desktop/src/i18n 导出成 assets/i18n/base.<语言>.json），再叠上 Android 才有的（android.<语言>.json）。
 * 用法照着 i18next：`t("cull.doneBody", "keep" to 3, …)`，`{{名字}}` 换成参数，带 `count` 参数时先找 `_one` / `_other` 的键。
 * 语言跟随系统，或在设置里选；选了马上整个界面换过去。
 */
object I18n {
    data class Lang(val code: String, val label: String)
    val LANGS = listOf(Lang("zh-CN", "简体中文"), Lang("en", "English"))

    private val tables = HashMap<String, JsonObject>()
    private val _lang = MutableStateFlow("en")
    /** 当前语言（"zh-CN" / "en"）；界面读它来重组。 */
    val lang: StateFlow<String> = _lang
    /** 同一个值的快照状态：`t()` 在组合里读它，谁用了文案谁就随语言重组（LazyColumn 的 item、被跳过的子组件也一样）。 */
    private val current = mutableStateOf("en")
    private lateinit var prefs: android.content.SharedPreferences

    fun init(ctx: Context) {
        prefs = ctx.getSharedPreferences("fg", Context.MODE_PRIVATE)
        for (l in LANGS.map { it.code }) {
            val base = read(ctx, "i18n/base.$l.json")
            val extra = read(ctx, "i18n/android.$l.json")
            tables[l] = merge(base, extra)
        }
        switchTo(resolve(pref()))
    }

    private fun read(ctx: Context, name: String): JsonObject =
        runCatching { Json.parseToJsonElement(ctx.assets.open(name).bufferedReader().use { it.readText() }) as JsonObject }.getOrDefault(JsonObject(emptyMap()))

    private fun merge(a: JsonObject, b: JsonObject): JsonObject {
        val out = a.toMutableMap()
        for ((k, v) in b) {
            val old = out[k]
            out[k] = if (old is JsonObject && v is JsonObject) merge(old, v) else v
        }
        return JsonObject(out)
    }

    /** 用户的选择："system"、"zh-CN" 或 "en"。 */
    fun pref(): String = prefs.getString("lang", "system") ?: "system"

    fun setPref(p: String) {
        prefs.edit().putString("lang", p).apply()
        switchTo(resolve(p))
    }

    private fun switchTo(l: String) {
        current.value = l
        _lang.value = l
    }

    /** 当前语言；在组合里读会订阅重组。 */
    val code: String get() = current.value

    private fun resolve(p: String): String = if (p == "system") (if (Locale.getDefault().language.startsWith("zh")) "zh-CN" else "en") else p

    val isZh: Boolean get() = current.value.startsWith("zh")

    private fun lookup(table: JsonObject?, key: String): JsonElement? {
        var cur: JsonElement? = table
        for (part in key.split('.')) cur = (cur as? JsonObject)?.get(part) ?: return null
        return cur
    }

    private fun find(key: String, count: Number?): JsonElement? {
        for (l in listOf(current.value, "en")) {
            val table = tables[l]
            if (count != null) {
                val one = count.toDouble() == 1.0 && !l.startsWith("zh")
                lookup(table, key + if (one) "_one" else "_other")?.let { return it }
                lookup(table, "${key}_other")?.let { return it }
            }
            lookup(table, key)?.let { return it }
        }
        return null
    }

    /** 一句话：`{{参数}}` 换好；找不到就给键本身（和 i18next 一样）。 */
    fun t(key: String, vararg params: Pair<String, Any?>): String {
        val map = params.toMap()
        val el = find(key, map["count"] as? Number) ?: return key
        var s = (el as? JsonPrimitive)?.contentOrNull ?: return key
        if ("{{" in s) for ((k, v) in map) s = s.replace("{{$k}}", fmt(v))
        return s
    }

    /** 一组句子（`cull.steps`、`styles.newExamples` 这样的数组）。 */
    fun list(key: String): List<String> = (find(key, null) as? JsonArray)?.mapNotNull { (it as? JsonPrimitive)?.contentOrNull } ?: emptyList()

    /** 有这个键吗（`t(..., defaultValue)` 的替代）。 */
    fun has(key: String): Boolean = find(key, null) != null

    private fun fmt(v: Any?): String = when (v) {
        null -> ""
        is Float, is Double -> { val d = (v as Number).toDouble(); if (d == Math.floor(d)) d.toLong().toString() else d.toString() }
        else -> v.toString()
    }
}

/** 界面里用的 `t`：语言一换就重组。 */
@Composable
fun rememberLang(): String {
    val l by I18n.lang.collectAsState()
    return l
}

fun t(key: String, vararg params: Pair<String, Any?>): String = I18n.t(key, *params)
