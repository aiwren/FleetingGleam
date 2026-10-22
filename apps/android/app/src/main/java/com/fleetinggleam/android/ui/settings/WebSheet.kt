package com.fleetinggleam.android.ui.settings

import android.content.ComponentName
import android.content.Context
import android.net.Uri
import androidx.browser.customtabs.CustomTabsClient
import androidx.browser.customtabs.CustomTabsIntent
import androidx.browser.customtabs.CustomTabsServiceConnection
import androidx.browser.customtabs.CustomTabsSession

/**
 * 应用内的半屏浏览器（隐私政策、使用条款）：Chrome Custom Tabs 从底部推上来、占屏幕大部分高度的那种。
 * 局部高度只在带会话（先连上浏览器的 CustomTabsService）时才生效，所以进关于页就先 `warm` 一次，点链接时带着会话打开；
 * 没有支持的浏览器、或者还没连上，就退成普通的 Custom Tab / 系统浏览器。
 */
object WebSheet {
    private var session: CustomTabsSession? = null
    private var binding = false

    /** 连上浏览器的服务并开一个会话（异步；连不上就算了）。 */
    fun warm(ctx: Context) {
        if (session != null || binding) return
        val pkg = CustomTabsClient.getPackageName(ctx, null) ?: return
        binding = runCatching {
            CustomTabsClient.bindCustomTabsService(ctx.applicationContext, pkg, object : CustomTabsServiceConnection() {
                override fun onCustomTabsServiceConnected(name: ComponentName, client: CustomTabsClient) {
                    client.warmup(0)
                    session = client.newSession(null)
                }
                override fun onServiceDisconnected(name: ComponentName) { session = null; binding = false }
            })
        }.getOrDefault(false)
    }

    /** 在半屏里打开；打不开就 `fallback`（调用方一般是跳系统浏览器）。 */
    fun open(ctx: Context, url: String, fallback: () -> Unit) {
        runCatching {
            val h = (ctx.resources.displayMetrics.heightPixels * 0.88f).toInt()
            CustomTabsIntent.Builder(session)
                .setInitialActivityHeightPx(h, CustomTabsIntent.ACTIVITY_HEIGHT_ADJUSTABLE)
                .setToolbarCornerRadiusDp(16)
                .setShowTitle(true)
                .build()
                .launchUrl(ctx, Uri.parse(url))
        }.onFailure { fallback() }
    }
}
