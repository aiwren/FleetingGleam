package com.fleetinggleam.android.core

import android.content.Context
import android.content.SharedPreferences

/**
 * 只关乎这台手机界面的小记忆（桌面端存在 localStorage 里的那些）：浏览方式、对比几张、操作提示还要不要出现、实况是否自动播放。
 * 不进"我的设置与喜好"的导出。
 */
object UiPrefs {
    private lateinit var prefs: SharedPreferences
    fun init(ctx: Context) { prefs = ctx.getSharedPreferences("fg.ui", Context.MODE_PRIVATE) }
    fun str(k: String, def: String): String = if (::prefs.isInitialized) prefs.getString(k, def) ?: def else def
    fun int(k: String, def: Int): Int = if (::prefs.isInitialized) prefs.getInt(k, def) else def
    fun bool(k: String, def: Boolean): Boolean = if (::prefs.isInitialized) prefs.getBoolean(k, def) else def
    fun put(k: String, v: String) { if (::prefs.isInitialized) prefs.edit().putString(k, v).apply() }
    fun put(k: String, v: Int) { if (::prefs.isInitialized) prefs.edit().putInt(k, v).apply() }
    fun put(k: String, v: Boolean) { if (::prefs.isInitialized) prefs.edit().putBoolean(k, v).apply() }
}
