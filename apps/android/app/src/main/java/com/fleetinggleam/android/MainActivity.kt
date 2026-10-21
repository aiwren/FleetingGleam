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
