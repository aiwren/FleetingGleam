package com.fleetinggleam.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import com.fleetinggleam.android.core.Pickers
import com.fleetinggleam.android.core.Projects
import com.fleetinggleam.android.core.Settings
import com.fleetinggleam.android.ui.AppShell
import com.fleetinggleam.android.ui.theme.FleetingGleamTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge(statusBarStyle = SystemBarStyle.auto(android.graphics.Color.TRANSPARENT, android.graphics.Color.TRANSPARENT))
        Pickers.register(this)
        // 调试开关：`adb shell am start -n com.fleetinggleam.android/.MainActivity --es fg_accel_probe <模型文件名>`
        // 在加速器上单独试跑一个模型，结果进系统日志（tag FleetingGleam）。界面上没有入口
        intent?.getStringExtra("fg_accel_probe")?.let { file ->
            // 试跑时可以临时改 NPU 的环境：fg_adsp = 驱动路径（"lib" 前置应用的库目录 / "none" 不动 / 其他原样），fg_qnn_arch、fg_qnn_soc
            val libDir = applicationInfo.nativeLibraryDir
            when (val a = intent.getStringExtra("fg_adsp")) {
                null, "none" -> {}
                "lib" -> android.system.Os.setenv("ADSP_LIBRARY_PATH", "$libDir;" + (System.getenv("ADSP_LIBRARY_PATH") ?: ""), true)
                "only" -> android.system.Os.setenv("ADSP_LIBRARY_PATH", libDir, true)
                else -> android.system.Os.setenv("ADSP_LIBRARY_PATH", a, true)
            }
            intent.getStringExtra("fg_qnn_arch")?.let { android.system.Os.setenv("FG_QNN_ARCH", it, true) }
            intent.getStringExtra("fg_qnn_soc")?.let { android.system.Os.setenv("FG_QNN_SOC", it, true) }
            if (intent.getBooleanExtra("fg_ort_verbose", false)) android.system.Os.setenv("FG_ORT_VERBOSE", "1", true)
            android.util.Log.i("FleetingGleam", "试跑环境：ADSP_LIBRARY_PATH=${System.getenv("ADSP_LIBRARY_PATH")} arch=${System.getenv("FG_QNN_ARCH")} soc=${System.getenv("FG_QNN_SOC")} SOC_MODEL=${android.os.Build.SOC_MODEL}")
            Thread({ runCatching { com.fleetinggleam.android.core.Core.callBlocking("accel_probe", com.fleetinggleam.android.core.args("file" to file)) }
                .onFailure { android.util.Log.i("FleetingGleam", "加速器试跑失败：${it.message}") } }, "fg-accel-probe").start()
        }
        setContent {
            val s by Settings.state.collectAsState()
            FleetingGleamTheme(s.theme) { AppShell() }
        }
    }

    override fun onResume() {
        super.onResume()
        // 回收站里过期的项目在回到应用时清掉（桌面端是窗口重新获得焦点时）
        Projects.purgeExpired()
    }
}
