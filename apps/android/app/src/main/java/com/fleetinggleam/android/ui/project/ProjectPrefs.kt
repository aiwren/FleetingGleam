package com.fleetinggleam.android.ui.project

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.fleetinggleam.android.core.Project
import com.fleetinggleam.android.core.Projects
import com.fleetinggleam.android.core.Styles
import com.fleetinggleam.android.core.pct
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgSwitch
import com.fleetinggleam.android.ui.components.Segmented
import com.fleetinggleam.android.ui.components.Select
import com.fleetinggleam.android.ui.components.SettingRow
import com.fleetinggleam.android.ui.components.StylePicker
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.delay

/**
 * 每个项目的选片偏好（桌面端 ProjectPrefs.tsx）：准备页和项目设置共用。画的是一组 iOS 分组列表里的行（调用方套 `SettingGroup`），
 * 每行一个短说明，长的解释收进"?"里。
 */
@Composable
fun ProjectPrefs(project: Project) {
    val c = LocalFg.current
    val styles by Styles.list.collectAsState()
    // 滑块先改本地状态，手停下来再写：每一格都重算整个项目太重
    var cap by remember(project.id) { mutableFloatStateOf(project.keepCap ?: 0.5f) }
    LaunchedEffect(project.keepCap) { cap = project.keepCap ?: 0.5f }
    LaunchedEffect(cap) { delay(180); if (project.keepCap != null && project.keepCap != cap) Projects.update(project.id) { it.copy(keepCap = cap) } }

    SettingRow(
        t("pset.keepShare"), t(if (project.keepCap == null) "android.pset.keepShareOff" else "android.pset.keepShareOn"), info = t(if (project.keepCap == null) "pset.keepShareDescAuto" else "pset.keepShareDesc"), first = true,
        below = if (project.keepCap == null) null else {
            {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Slider(cap, { cap = (Math.round(it / 0.05f) * 0.05f).coerceIn(0.05f, 0.95f) }, valueRange = 0.05f..0.95f, modifier = Modifier.weight(1f),
                        colors = SliderDefaults.colors(thumbColor = c.ink, activeTrackColor = c.ink, inactiveTrackColor = c.surface2, activeTickColor = c.ink, inactiveTickColor = c.surface2))
                    Text("${pct(cap)}%", style = Type.mono.copy(color = c.ink), modifier = Modifier.width(52.dp).padding(start = 10.dp))
                }
            }
        },
    ) { FgSwitch(project.keepCap != null, { on -> Projects.update(project.id) { it.copy(keepCap = if (on) 0.5f else null) } }, t("pset.keepShareLimit")) }
    SettingRow(t("pset.onePerGroup"), t("android.pset.onePerGroupDesc"), info = t("pset.onePerGroupDesc")) {
        FgSwitch(project.onePerGroup == true, { on -> Projects.update(project.id) { it.copy(onePerGroup = on) } }, t("pset.onePerGroup"))
    }
    SettingRow(t("cull.pickMode"), if (project.pickMode == "best") t("cull.pickModeBestDesc") else t("cull.pickModeDiverseDesc"), below = {
        Segmented(listOf("diverse" to t("cull.pickModeDiverse"), "best" to t("cull.pickModeBest")), project.pickMode ?: "diverse", { v -> Projects.update(project.id) { it.copy(pickMode = v) } }, block = true)
    })
    SettingRow(t("pset.sort")) {
        Select(project.sort ?: "group", listOf("group", "order", "time", "score").map { it to t("cull.sort.$it") }, { v -> Projects.update(project.id) { it.copy(sort = v) } })
    }
    if (styles.size > 1) SettingRow(t("nav.styles"), below = {
        StylePicker(project.style ?: "", styles) { v -> Projects.update(project.id) { it.copy(style = v.ifEmpty { null }) } }
    })
}
