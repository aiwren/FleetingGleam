package com.fleetinggleam.android.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import androidx.core.app.NotificationCompat
import com.fleetinggleam.android.MainActivity
import com.fleetinggleam.android.R
import com.fleetinggleam.android.core.Gate
import com.fleetinggleam.android.core.Models
import com.fleetinggleam.android.core.fmtBytes
import com.fleetinggleam.android.core.t
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.sample
import kotlinx.coroutines.launch

/**
 * 模型下载期间的前台服务：下载转到后台、切走或锁屏都不会被系统停掉；通知栏里显示下载进度，可以取消。
 * 下载本身在核心库里跑（Models.startDownload 调 `pack_download`），这里只负责挂着与显示。
 */
class DownloadService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private var job: Job? = null

    override fun onBind(intent: Intent?): IBinder? = null

    @OptIn(kotlinx.coroutines.FlowPreview::class)
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_CANCEL) { Models.cancelGate(); return START_NOT_STICKY }
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, t("android.models.channel"), NotificationManager.IMPORTANCE_LOW).apply { setShowBadge(false) })
        startForeground(ID, build(Models.gate.value), ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        job?.cancel()
        // 通知栏刷新太勤系统会丢：一秒两次就够
        job = scope.launch { Models.gate.sample(500).collect { g -> if (g != null) nm.notify(ID, build(g)) } }
        return START_NOT_STICKY
    }

    private fun build(g: Gate?): Notification {
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_IMMUTABLE)
        val cancel = PendingIntent.getService(this, 2, Intent(this, DownloadService::class.java).setAction(ACTION_CANCEL), PendingIntent.FLAG_IMMUTABLE)
        val total = g?.total ?: 0; val done = g?.done ?: 0
        val name = g?.let { t(if (it.ids != null) "android.models.bundle" else "models.name.${it.id}") } ?: ""
        return NotificationCompat.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_stat_gleam)
            .setContentTitle(t("android.models.downloading", "name" to name))
            .setContentText(if (total > 0) "${fmtBytes(done)} / ${fmtBytes(total)}" else "")
            .setProgress(1000, if (total > 0) (done * 1000 / total).toInt() else 0, total == 0L)
            .setOngoing(true).setOnlyAlertOnce(true).setSilent(true)
            .setContentIntent(open)
            .addAction(0, t("organize.cancel"), cancel)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            .build()
    }

    override fun onDestroy() { job?.cancel(); super.onDestroy() }

    companion object {
        private const val CHANNEL = "models"
        private const val ID = 8
        private const val ACTION_CANCEL = "com.fleetinggleam.android.CANCEL_DOWNLOAD"
        fun start(ctx: Context) { runCatching { ctx.startForegroundService(Intent(ctx, DownloadService::class.java)) } }
        fun stop(ctx: Context) { runCatching { ctx.stopService(Intent(ctx, DownloadService::class.java)) } }
    }
}
