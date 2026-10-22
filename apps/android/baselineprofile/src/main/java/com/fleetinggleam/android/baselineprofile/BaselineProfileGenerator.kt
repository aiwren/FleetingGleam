package com.fleetinggleam.android.baselineprofile

import androidx.benchmark.macro.junit4.BaselineProfileRule
import androidx.test.uiautomator.By
import androidx.test.uiautomator.Until
import org.junit.Rule
import org.junit.Test

/**
 * 走一遍最常见的路：冷启动到首页、进最近的项目、来回滚照片墙、开大图。设备上要先有一个分析过的项目（没有就只记启动那段）。
 * 运行：`./gradlew :app:generateBaselineProfile`。
 */
class BaselineProfileGenerator {
    @get:Rule
    val rule = BaselineProfileRule()

    @Test
    fun generate() = rule.collect(packageName = "com.fleetinggleam.android", includeInStartupProfile = true, maxIterations = 5) {
        pressHome()
        // 不用 startActivityAndWait()：模拟器上它读不到帧统计会报 "Unable to confirm activity launch completion"；自己起、等够久
        device.executeShellCommand("am start -W -n com.fleetinggleam.android/.MainActivity")
        Thread.sleep(4_000)
        device.waitForIdle()
        // 首页：有项目就点最近那张大卡进去
        device.wait(Until.hasObject(By.textContains("继续")), 3_000)
        device.findObject(By.textContains("继续"))?.click()
        device.waitForIdle()
        Thread.sleep(1_500)
        // 照片墙来回滚
        val w = device.displayWidth; val h = device.displayHeight
        repeat(3) {
            device.swipe(w / 2, h * 3 / 4, w / 2, h / 4, 20)
            device.waitForIdle()
        }
        repeat(3) {
            device.swipe(w / 2, h / 4, w / 2, h * 3 / 4, 20)
            device.waitForIdle()
        }
        // 点一张、放大查看、关掉
        device.click(w / 7, h / 4)
        Thread.sleep(600)
        device.findObject(By.textContains("放大"))?.click()
        Thread.sleep(1_500)
        device.pressBack()
        Thread.sleep(500)
        device.pressBack()
    }
}
