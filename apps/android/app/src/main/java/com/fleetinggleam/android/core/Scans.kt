package com.fleetinggleam.android.core

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import android.os.PowerManager
import androidx.core.content.ContextCompat
import com.fleetinggleam.android.service.AnalysisService

/**
 * 分析在手机上要考虑的两件事（桌面端没有）：
 * - **前台服务**：分析可能要几分钟到几十分钟，用户切到别的应用时系统不能把它停掉，所以分析期间应用挂一个前台服务（通知栏里有进度和"停止"）。
 * - **发热与电量**：手机过热（THERMAL_STATUS_SEVERE 及以上）或没在充电且电量不到 15% 时先停下——已分析的照片都在缓存里，什么都不丢；
 *   降温到 LIGHT 以下、或接上电源 / 电量回到 20% 以上，就从停下的地方接着分析。停下期间前台服务不撤（通知写"已暂停（n/N）"），
 *   不然接着分析时应用在后台、系统不让再启前台服务，就续不上了。
 */
object Scans {
    private lateinit var app: Context
    @Volatile private var running = false
    /** 通知上显示的项目名：前台服务跟着它刷新。 */
    val label = kotlinx.coroutines.flow.MutableStateFlow("")
    private var thermal = PowerManager.THERMAL_STATUS_NONE
    private var batteryLow = false
    private var charging = true

    fun init(ctx: Context) {
        app = ctx.applicationContext
        val pm = app.getSystemService(PowerManager::class.java)
        thermal = pm.currentThermalStatus
        pm.addThermalStatusListener(ContextCompat.getMainExecutor(app)) { s ->
            thermal = s
            if (s >= PowerManager.THERMAL_STATUS_SEVERE) Session.pauseFor(Pause.Hot) else if (s <= PowerManager.THERMAL_STATUS_LIGHT && Session.paused.value == Pause.Hot) Session.resumeAfterPause()
        }
        ContextCompat.registerReceiver(app, object : BroadcastReceiver() {
            override fun onReceive(c: Context, i: Intent) {
                val level = i.getIntExtra(BatteryManager.EXTRA_LEVEL, -1)
                val scale = i.getIntExtra(BatteryManager.EXTRA_SCALE, 100).coerceAtLeast(1)
                val status = i.getIntExtra(BatteryManager.EXTRA_STATUS, -1)
                charging = status == BatteryManager.BATTERY_STATUS_CHARGING || status == BatteryManager.BATTERY_STATUS_FULL
                val pct = if (level < 0) 100 else level * 100 / scale
                val wasLow = batteryLow
                batteryLow = !charging && pct <= 15
                if (batteryLow && !wasLow) Session.pauseFor(Pause.Battery)
                if (Session.paused.value == Pause.Battery && (charging || pct >= 20)) Session.resumeAfterPause()
            }
        }, IntentFilter(Intent.ACTION_BATTERY_CHANGED), ContextCompat.RECEIVER_NOT_EXPORTED)
    }

    /** 系统已经不"优化"这个应用的电量了（切到后台不会被停掉）。 */
    fun backgroundAllowed(ctx: Context): Boolean = ctx.getSystemService(PowerManager::class.java).isIgnoringBatteryOptimizations(ctx.packageName)

    /** 请系统允许在后台运行：弹的是系统自己的确认框；有的手机没有这个框，就打开电池优化的列表让人自己找。 */
    fun askBackground(ctx: Context) {
        val direct = Intent(android.provider.Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, android.net.Uri.parse("package:${ctx.packageName}")).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        runCatching { ctx.startActivity(direct) }.onFailure {
            runCatching { ctx.startActivity(Intent(android.provider.Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
        }
    }

    /** 现在适不适合开始分析（太热或电量太低就先等）。 */
    fun blocker(): Pause = when {
        thermal >= PowerManager.THERMAL_STATUS_SEVERE -> Pause.Hot
        batteryLow -> Pause.Battery
        else -> Pause.None
    }

    /**
     * 一次有进度的分析开始了，或者还没开始就因为太热 / 电量低进了暂停：挂上前台服务（暂停期间留着的那个就接着用，不从后台再启一次）。
     * 适不适合分析由调用方（`Session.runScan`）在发起之前查 `blocker()`，这里不再补一个取消：那时核心还没登记任务，取消会被丢掉（审查第 40 条）。
     */
    fun started(ctx: Context, projectName: String) {
        // 暂停期间留着的服务接着用：换了项目分析时把通知上的名字换过来（审查第 58 条）
        label.value = projectName
        if (running) return
        running = true
        val i = Intent(ctx, AnalysisService::class.java).putExtra(AnalysisService.EXTRA_NAME, projectName)
        runCatching { ContextCompat.startForegroundService(ctx, i) }
    }

    /** 前台服务到了系统的 6 小时上限，自己撤掉了：只记下不在跑，别再去停它。 */
    fun timedOut() { running = false }

    fun stopped() {
        if (!running) return
        running = false
        runCatching { app.stopService(Intent(app, AnalysisService::class.java)) }
    }
}
