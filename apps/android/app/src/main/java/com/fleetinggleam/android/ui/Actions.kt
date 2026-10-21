package com.fleetinggleam.android.ui

import com.fleetinggleam.android.core.Nas
import com.fleetinggleam.android.core.Projects
import com.fleetinggleam.android.core.baseName

/** 和桌面端 src/projectActions.ts 一样：选一个可以当项目文件夹的地方，或者从它新建一个项目。 */
object Actions {
    /**
     * 手机上项目的文件夹总有几种来源（相册、系统文件选择器里的任意文件夹、添加过的 NAS），所以总是先问"在哪里"（FolderPicker）。
     * 拿到的是一个本地路径：相册所在的文件夹、换算出来的文件夹、系统文件夹或 NAS 共享在镜像里的文件夹。
     */
    suspend fun pickFolder(shareId: String? = null): String? = Nas.pick(shareId)

    /** Choose a folder and start a new project from it (it opens right away, like on the Mac). */
    suspend fun newProjectFromDialog(shareId: String? = null): Boolean {
        val dir = pickFolder(shareId) ?: return false
        Projects.create(baseName(dir), listOf(dir))
        return true
    }
}
