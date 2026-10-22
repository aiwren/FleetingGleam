package com.fleetinggleam.android.core

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/** 跨页面的跳转请求：起始屏的"连接 NAS"要打开设置页的 NAS 分区，而页签和分区各自住在自己的页面里。 */
object Nav {
    private val _settings = MutableStateFlow<String?>(null)
    /** 想打开的设置分区（"nas" 之类）；AppShell 看到就切到设置页签，设置页看到就进那个分区并清掉。 */
    val settings: StateFlow<String?> = _settings
    fun openSettings(section: String) { _settings.value = section }
    fun consumed() { _settings.value = null }
}
