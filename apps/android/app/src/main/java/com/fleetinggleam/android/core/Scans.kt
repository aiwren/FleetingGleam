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
 *   降温到 LIGHT 以下、或接上电源 / 电量回到 20% 以上，就从停下的地方接着分析。
 */
object Scans {
    private lateinit var app: Context
    @Volatile private var running = false
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

    /** 现在适不适合开始分析（太热或电量太低就先等）。 */
    fun blocker(): Pause = when {
        thermal >= PowerManager.THERMAL_STATUS_SEVERE -> Pause.Hot
        batteryLow -> Pause.Battery
        else -> Pause.None
    }

    /** 一次有进度的分析开始了：挂上前台服务。 */
    fun started(ctx: Context, projectName: String) {
        if (running) return
        running = true
        val i = Intent(ctx, AnalysisService::class.java).putExtra(AnalysisService.EXTRA_NAME, projectName)
        runCatching { ContextCompat.startForegroundService(ctx, i) }
        blocker().takeIf { it != Pause.None }?.let { Session.pauseFor(it) }
    }

    fun stopped() {
        if (!running) return
        running = false
        runCatching { app.stopService(Intent(app, AnalysisService::class.java)) }
    }
}
