package com.fleetinggleam.android.ui.project

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.theme.LocalFg

/** 普通的五角星（24 视框），和应用自己的四角"惊鸿"标记（保留 / 待复查）区分开；路径和桌面端 Stars.tsx 一样。 */
private const val FIVE = "M12 2.6l2.9 6.1 6.7.8-4.9 4.6 1.3 6.6L12 17.4l-6 3.3 1.3-6.6L2.4 9.5l6.7-.8z"

/**
 * 手动星级（桌面端 Stars.tsx）：五颗可点的星。点第 n 颗打 n 星，再点同一颗取消；`value` 是现在的星数（0 = 没打）。
 * `star` 是画出来的星的边长，`hit` 是每颗的点按区域（手机上手指要够大）。
 */
@Composable
fun RateStars(value: Int, onChange: (Int?) -> Unit, modifier: Modifier = Modifier, star: Dp = 18.dp, hit: Dp = 36.dp, color: Color = LocalFg.current.ink) {
    val path = remember { PathParser().parsePathString(FIVE).toPath() }
    Row(modifier.selectableGroup().semantics { contentDescription = t("stars.label") }, verticalAlignment = Alignment.CenterVertically) {
        for (n in 1..5) {
            val label = if (value == n) t("stars.clear") else t("stars.set", "count" to n)
            Box(
                Modifier.size(hit).clip(CircleShape).semantics { contentDescription = label; selected = value == n }
                    .clickable(role = Role.RadioButton) { onChange(if (value == n) null else n) },
                contentAlignment = Alignment.Center,
            ) {
                Canvas(Modifier.size(star)) {
                    val k = size.minDimension / 24f
                    scale(k, pivot = Offset.Zero) {
                        if (n <= value) drawPath(path, color)
                        drawPath(path, color, style = Stroke(width = 1.6f, join = StrokeJoin.Round))
                    }
                }
            }
        }
    }
}
