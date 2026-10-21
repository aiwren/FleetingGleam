package com.fleetinggleam.android.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.dp
import com.fleetinggleam.android.core.StyleInfo
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type

/** 下拉选择（桌面端的 <Select>）：显示当前项，点开是一列选项。 */
@Composable
fun <T> Select(value: T, options: List<Pair<T, String>>, onChange: (T) -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true) {
    val c = LocalFg.current
    var open by remember { mutableStateOf(false) }
    Box(modifier) {
        Row(
            Modifier.graphicsLayer { alpha = if (enabled) 1f else 0.4f }.clip(CircleShape).background(c.surface)
                .clickable(enabled = enabled) { open = true }.padding(start = 12.dp, end = 8.dp, top = 9.dp, bottom = 9.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(options.firstOrNull { it.first == value }?.second ?: "", style = Type.small.copy(color = c.ink), maxLines = 1, overflow = Ellipsis)
            FgIcon("M4 6.5l4 3.5 4-3.5", size = 14.dp, color = c.ash, modifier = Modifier.padding(start = 6.dp))
        }
        DropdownMenu(open, { open = false }, containerColor = c.canvas) {
            options.forEach { (v, label) ->
                DropdownMenuItem(text = { Text(label, style = Type.body.copy(color = c.ink)) }, onClick = { open = false; onChange(v) },
                    trailingIcon = if (v == value) ({ FgIcon("M3.5 8.5l3 3 6-7", size = 16.dp) }) else null)
            }
        }
    }
}

/** "选片风格"一行：跟随默认风格，或指定一个。 */
@Composable
fun StylePicker(value: String, styles: List<StyleInfo>, onChange: (String) -> Unit) {
    val c = LocalFg.current
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Column(Modifier.weight(1f)) {
            Text(t("styles.project"), style = Type.bodyStrong.copy(color = c.ink))
            Text(t("styles.projectDesc"), style = Type.small.copy(color = c.ash))
        }
        Select(value, listOf("" to t("styles.followDefault")) + styles.map { it.id to it.name.ifBlank { t("styles.defaultName") } }, onChange)
    }
}
