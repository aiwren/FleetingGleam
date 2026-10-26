package com.fleetinggleam.android.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import com.fleetinggleam.android.MainActivity
import com.fleetinggleam.android.R
import com.fleetinggleam.android.core.Core
import com.fleetinggleam.android.core.Fg
import com.fleetinggleam.android.core.Pause
import com.fleetinggleam.android.core.Progress
import com.fleetinggleam.android.core.Scans
import com.fleetinggleam.android.core.Session
import com.fleetinggleam.android.core.t
import kotlinx.coroutines.launch

/**
 * 分析期间的前台服务：让系统知道应用在干活，用户切走也不会被停掉；通知栏里显示进度（和应用里的横幅同一个数），带一个"停止分析"。
 * 分析本身在引擎里跑（Session 调 `scan_project`），这里只负责挂着与显示。
 */
class AnalysisService : Service() {
    private var off: (() -> Unit)? = null
    /** 分析期间不让 CPU 睡下去：熄屏后前台服务还在，可 CPU 一睡分析就停了。最多拿 6 小时，分析结束就放。 */
    private var wake: android.os.PowerManager.WakeLock? = null
    private var name = ""
    private var lastShown = 0L
    private val scope = kotlinx.coroutines.CoroutineScope(kotlinx.coroutines.SupervisorJob() + kotlinx.coroutines.Dispatchers.Main)
    private var pauseJob: kotlinx.coroutines.Job? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            Session.stopScan()
            // 暂停中点"停止"：不再等着接着分析，服务随之撤掉
            Session.dropPause()
            return START_NOT_STICKY
        }
        name = intent?.getStringExtra(EXTRA_NAME) ?: name
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, t("cull.analyzing"), NotificationManager.IMPORTANCE_LOW).apply { setShowBadge(false) })
        val type = if (Build.VERSION.SDK_INT >= 35) ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROCESSING else ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC
        // 直接调系统的 startForeground：androidx 的 ServiceCompat 不认识 API 35 新加的 mediaProcessing，会把它屏蔽成"无类型"，系统随即拒绝
        startForeground(ID, build(Progress()), type)
        if (wake == null) wake = getSystemService(android.os.PowerManager::class.java).newWakeLock(android.os.PowerManager.PARTIAL_WAKE_LOCK, "fleetinggleam:analysis").apply {
            setReferenceCounted(false)
            acquire(6 * 60 * 60 * 1000L)
        }
        off?.invoke()
        off = Core.listen("scan-progress") { raw ->
            val p = runCatching { Fg.json.decodeFromString<Progress>(raw) }.getOrNull() ?: return@listen
            // 一秒最多更新几次：通知栏刷新太勤系统会丢
            val now = System.currentTimeMillis()
            if (now - lastShown < 400 && p.done < p.total) return@listen
            lastShown = now
            if (Session.paused.value == Pause.None) nm.notify(ID, build(p))
        }
        // 太热 / 电量低停下时服务留着：通知改成"已暂停（n/N）"，接着分析时换回进度
        pauseJob?.cancel()
        pauseJob = scope.launch { Session.paused.collect { nm.notify(ID, build(Session.progress.value)) } }
        return START_NOT_STICKY
    }

    private fun build(p: Progress): Notification {
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_IMMUTABLE)
        val stop = PendingIntent.getService(this, 1, Intent(this, AnalysisService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE)
        val pause = Session.paused.value
        val title = when (pause) {
            Pause.Hot -> t("android.scan.pausedHot", "done" to p.done, "total" to p.total)
            Pause.Battery -> t("android.scan.pausedBattery", "done" to p.done, "total" to p.total)
            Pause.None -> if (name.isNotEmpty()) "${t("cull.analyzing")} · $name" else t("cull.analyzing")
        }
        val text = when (pause) {
            Pause.Hot -> t("android.scan.pausedHotBody")
            Pause.Battery -> t("android.scan.pausedBatteryBody")
            Pause.None -> if (p.total > 0) "${p.done} / ${p.total}" else t("cull.loadingModel")
        }
        return NotificationCompat.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_stat_gleam)
            .setContentTitle(title)
            .setContentText(text)
            .setProgress(p.total.coerceAtLeast(0), p.done.coerceAtLeast(0), p.total == 0)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setContentIntent(open)
            .addAction(0, t("cull.stop"), stop)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            .build()
    }

    /**
     * Android 15 起 dataSync / mediaProcessing 的前台服务一天合计最多 6 小时：到点系统调这里，几秒内不 stopSelf 就当应用崩了（审查第 22 条）。
     * 先停下分析（分析过的都在缓存里，回到应用点"开始分析"就从这里接着来），撤掉前台服务，再留一条通知说分析到哪儿了。
     */
    override fun onTimeout(startId: Int, fgsType: Int) {
        val p = Session.progress.value
        Session.stopScan()
        Session.dropPause()
        Scans.timedOut()
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_IMMUTABLE)
        val body = t("android.scan.timeoutBody", "done" to p.done, "total" to p.total)
        getSystemService(NotificationManager::class.java).notify(TIMEOUT_ID, NotificationCompat.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_stat_gleam)
            .setContentTitle(if (name.isNotEmpty()) "${t("android.scan.timeoutTitle")} · $name" else t("android.scan.timeoutTitle"))
            .setContentText(body)
            .setStyle(NotificationCompat.BigTextStyle().bigText(body))
            .setAutoCancel(true)
            .setContentIntent(open)
            .build())
    }

    override fun onDestroy() {
        pauseJob?.cancel()
        pauseJob = null
        off?.invoke()
        off = null
        wake?.let { if (it.isHeld) it.release() }
        wake = null
        super.onDestroy()
    }

    companion object {
        const val EXTRA_NAME = "name"
        const val ACTION_STOP = "com.fleetinggleam.android.STOP_SCAN"
        private const val CHANNEL = "analysis"
        private const val ID = 7
        /** 6 小时到点停下时留的那条（不是常驻的；8 是下载服务的）。 */
        private const val TIMEOUT_ID = 9
    }
}
