package com.fleetinggleam.android.ui.components

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * 入场：从下方一点点升起、淡入，按序号错开（每项 30 ms、最多 8 项，之后的一起出现——设计审查 R6）。只在第一次出现时播，回到这里不重播。
 * 和桌面端 `initial={{ opacity: 0, y: 16, scale: .97 }} transition={{ ...SPRING.soft, delay: .04 + min(i, 8) * .024 }}` 一样。
 * 弹层、菜单、对话框里面的内容不要用它：弹层本身已经在动。
 */
fun Modifier.riseIn(index: Int, rise: Float = 16f, scaleFrom: Float = 0.97f): Modifier = composed {
    val px = with(LocalDensity.current) { rise.dp.toPx() }
    val a = remember { Animatable(0f) }
    LaunchedEffect(Unit) {
        delay(com.fleetinggleam.android.ui.theme.Motion.stagger(index))
        a.animateTo(1f, spring(dampingRatio = 0.997f, stiffness = 170f))
    }
    graphicsLayer {
        val v = a.value
        alpha = v.coerceIn(0f, 1f)
        translationY = (1f - v) * px
        val s = scaleFrom + (1f - scaleFrom) * v
        scaleX = s; scaleY = s
    }
}

/** 按下时轻轻收一点（桌面端 whileTap scale .985）。 */
fun Modifier.pressScale(src: MutableInteractionSource, to: Float = 0.985f): Modifier = composed {
    val pressed by src.collectIsPressedAsState()
    val s = remember { Animatable(1f) }
    LaunchedEffect(pressed) { launch { s.animateTo(if (pressed) to else 1f, spring(dampingRatio = 0.653f, stiffness = 600f)) } }
    graphicsLayer { scaleX = s.value; scaleY = s.value }
}

@Composable
fun rememberSource() = remember { MutableInteractionSource() }
