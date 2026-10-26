package com.fleetinggleam.android

import android.app.Application
import com.fleetinggleam.android.core.Backup
import com.fleetinggleam.android.core.Core
import com.fleetinggleam.android.core.I18n
import com.fleetinggleam.android.core.Models
import com.fleetinggleam.android.core.Nas
import com.fleetinggleam.android.core.Projects
import com.fleetinggleam.android.core.Scans
import com.fleetinggleam.android.core.Session
import com.fleetinggleam.android.core.Settings
import com.fleetinggleam.android.core.Styles
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

/** 进程启动：建引擎，读文案、设置与项目，开始盯着 NAS、备份、发热与电量。和桌面端 App.tsx 里启动时做的事一样。 */
class FgApp : Application() {
    override fun onCreate() {
        super.onCreate()
        I18n.init(this)
        Settings.init(this)
        com.fleetinggleam.android.core.UiPrefs.init(this)
        Core.init(this, packageManager.getPackageInfo(packageName, 0).versionName ?: "0.1.0")
        Models.init(this)
        com.fleetinggleam.android.core.Media.init(this)
        Nas.init()
        Backup.watch()
        Session.init(this)
        Scans.init(this)
        CoroutineScope(SupervisorJob() + Dispatchers.IO).launch {
            Styles.refresh()
            Projects.load()
            Projects.purgeExpired()
        }
    }
}
