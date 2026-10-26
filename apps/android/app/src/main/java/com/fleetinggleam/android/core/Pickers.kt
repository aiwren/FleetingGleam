package com.fleetinggleam.android.core

import android.Manifest
import android.content.IntentSender
import android.net.Uri
import android.os.Build
import androidx.activity.ComponentActivity
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.IntentSenderRequest
import androidx.activity.result.contract.ActivityResultContracts
import kotlinx.coroutines.CompletableDeferred

/**
 * 系统对话框（权限、文件选择器、系统回收站的确认），包成可以 `await` 的函数——就像桌面端的 `open()` / `save()` 对话框。
 * 发起者在 MainActivity 里注册一次；同一时间只会有一个在等。
 */
object Pickers {
    private lateinit var perms: ActivityResultLauncher<Array<String>>
    private lateinit var tree: ActivityResultLauncher<Uri?>
    private lateinit var create: ActivityResultLauncher<String>
    private lateinit var openDoc: ActivityResultLauncher<Array<String>>
    private lateinit var createTxt: ActivityResultLauncher<String>
    private lateinit var sender: ActivityResultLauncher<IntentSenderRequest>
    private var waiting: CompletableDeferred<Any?>? = null
    private var activity: java.lang.ref.WeakReference<ComponentActivity>? = null

    /** 上一次问读照片权限的结果：被拒绝、而且系统不会再弹窗（用户选了"不再询问"，或拒绝了两次），只能去系统设置里开。 */
    var mediaBlocked = false
        private set

    private fun <T> answer(v: T) { val w = waiting; waiting = null; w?.complete(v) }

    fun register(a: ComponentActivity) {
        activity = java.lang.ref.WeakReference(a)
        perms = a.registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { answer(it.values.all { ok -> ok }) }
        tree = a.registerForActivityResult(ActivityResultContracts.OpenDocumentTree()) { answer(it) }
        create = a.registerForActivityResult(ActivityResultContracts.CreateDocument("application/json")) { answer(it) }
        openDoc = a.registerForActivityResult(ActivityResultContracts.OpenDocument()) { answer(it) }
        createTxt = a.registerForActivityResult(ActivityResultContracts.CreateDocument("text/plain")) { answer(it) }
        sender = a.registerForActivityResult(ActivityResultContracts.StartIntentSenderForResult()) { answer(it.resultCode == android.app.Activity.RESULT_OK) }
    }

    private suspend fun <T> wait(start: () -> Unit): T {
        waiting?.complete(null)
        val d = CompletableDeferred<Any?>()
        waiting = d
        start()
        @Suppress("UNCHECKED_CAST")
        return d.await() as T
    }

    /**
     * 读照片的权限（没有就问）。Android 14 的"部分照片"也算给了。
     * `more`：已经只给了部分照片时再问一次——系统会弹出照片选择器，让用户多选一些或改成全部允许。
     */
    suspend fun mediaPermission(ctx: android.content.Context, more: Boolean = false): Boolean {
        if (Media.hasPermission(ctx) || (!more && Media.partialOnly(ctx))) { mediaBlocked = false; return true }
        val ask = if (Build.VERSION.SDK_INT >= 34) Media.permissions + Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED else Media.permissions
        wait<Boolean?>({ perms.launch(ask) })
        val ok = Media.hasPermission(ctx) || Media.partialOnly(ctx)
        // 被拒绝、系统也不再给理由弹窗：再点"允许"不会有任何反应，界面要改成带用户去系统设置
        mediaBlocked = !ok && activity?.get()?.let { a -> Media.permissions.none { a.shouldShowRequestPermissionRationale(it) } } == true
        return ok
    }

    /** 打开本应用在系统设置里的页面（权限被永久拒绝后，只能在那里打开）。 */
    fun openAppSettings(ctx: android.content.Context) {
        val i = android.content.Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", ctx.packageName, null))
            .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
        runCatching { ctx.startActivity(i) }
    }

    /** 通知权限（Android 13 起要问；分析的进度在通知里）。不给也照样分析。 */
    suspend fun notificationPermission(ctx: android.content.Context) {
        if (Build.VERSION.SDK_INT < 33) return
        if (androidx.core.content.ContextCompat.checkSelfPermission(ctx, Manifest.permission.POST_NOTIFICATIONS) == android.content.pm.PackageManager.PERMISSION_GRANTED) return
        wait<Boolean?>({ perms.launch(arrayOf(Manifest.permission.POST_NOTIFICATIONS)) })
    }

    /** 系统的"选文件夹"。 */
    suspend fun openTree(): Uri? = wait({ tree.launch(null) })

    /** 系统的"存为"（导出我的设置与喜好）。 */
    suspend fun createJson(name: String): Uri? = wait({ create.launch(name) })

    /** 系统的"打开文件"（导入我的设置与喜好）。 */
    /** 存一个文本文件（导出诊断信息）：系统的"另存为"。 */
    suspend fun createText(name: String): Uri? = wait({ createTxt.launch(name) })

    suspend fun openJson(): Uri? = wait({ openDoc.launch(arrayOf("application/json", "text/plain", "application/octet-stream")) })

    /** 系统的确认框（把照片移进系统回收站要用户点同意）：同意了返回 true。 */
    suspend fun confirm(sender: IntentSender): Boolean = wait<Boolean?>({ this.sender.launch(IntentSenderRequest.Builder(sender).build()) }) == true
}
