package com.fleetinggleam.android.ui.components

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.shape.CornerBasedShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import com.fleetinggleam.android.ui.theme.LocalFg
import com.kyant.backdrop.Backdrop
import com.kyant.backdrop.backdrops.emptyBackdrop
import com.kyant.backdrop.drawBackdrop
import com.kyant.backdrop.effects.blur
import com.kyant.backdrop.effects.lens
import com.kyant.backdrop.effects.vibrancy
import com.kyant.backdrop.highlight.Highlight
import com.kyant.backdrop.highlight.HighlightStyle
import com.kyant.backdrop.shadow.InnerShadow
import androidx.compose.ui.graphics.drawOutline
import com.kyant.backdrop.shadow.Shadow

/**
 * 液态玻璃（作者 2026-10-21 要求：像 iOS 那样）。玻璃后面的内容先录进一层（`layerBackdrop`），玻璃件再把它画出来：
 * 边缘按到边的距离做透镜折射（中间平、边上弯），轻微模糊与提饱和，一圈随角度变化的高光，一道柔和的投影。
 * 折射要 AGSL（Android 13+）；Android 12 上退成模糊玻璃，看起来仍是一块磨砂片。
 */
val LocalBackdrop = staticCompositionLocalOf<Backdrop> { emptyBackdrop() }

/**
 * 玻璃的参数。`top` / `bottom` 是盖在折射内容上的一层由上到下的渐变（浅色模式是很淡的白、底部略灰，深色是半透明的黑），
 * `rim` 是一圈极细的描边：背后是一片纯白时，折射看不出来，玻璃靠这圈边、内阴影和投影读出来（作者 2026-10-21 反馈"效果丢失"）。
 */
data class GlassStyle(
    val top: Color, val bottom: Color, val rim: Color,
    val blur: Dp = 2.dp, val lensHeight: Dp = 20.dp, val lensAmount: Dp = 44.dp, val shadow: Boolean = true, val dark: Boolean = false,
)

@Composable
fun glassStyle(strong: Boolean = false): GlassStyle {
    val c = LocalFg.current
    return if (c.dark) GlassStyle(
        top = Color(0xFF2A2A30).copy(alpha = if (strong) 0.55f else 0.32f), bottom = Color(0xFF0E0E10).copy(alpha = if (strong) 0.6f else 0.38f),
        rim = Color.White.copy(alpha = 0.14f), dark = true,
    ) else GlassStyle(
        top = Color.White.copy(alpha = if (strong) 0.6f else 0.32f), bottom = Color(0xFFE9E9EE).copy(alpha = if (strong) 0.62f else 0.4f),
        rim = Color.Black.copy(alpha = 0.07f),
    )
}

/**
 * 一块玻璃。`press` 在 0..1 之间：按下时玻璃像一滴水那样微微鼓起（放大一点、边缘折射更强），松手弹回。
 */
fun Modifier.glass(backdrop: Backdrop, shape: CornerBasedShape, style: GlassStyle, press: () -> Float = { 0f }): Modifier =
    drawBackdrop(
        backdrop = backdrop,
        shape = { shape },
        effects = {
            vibrancy()
            blur(style.blur.toPx())
            val p = press()
            lens((style.lensHeight * (1f + 0.35f * p)).toPx(), (style.lensAmount * (1f + 0.6f * p)).toPx(), depthEffect = true, chromaticAberration = true)
        },
        // 一圈随角度变亮的镜面高光（左上、右下最亮），比默认的更宽更亮
        highlight = { Highlight(width = 1.2.dp, blurRadius = 0.8.dp, alpha = if (style.dark) 0.7f else 1f, style = HighlightStyle.Default(color = Color.White.copy(alpha = 0.95f), angle = 50f, falloff = 1.6f)) },
        shadow = if (style.shadow) ({ Shadow(radius = 28.dp, offset = DpOffset(0.dp, 10.dp), color = Color.Black.copy(alpha = if (style.dark) 0.45f else 0.14f)) }) else null,
        // 内阴影：边缘向里一点点暗下去，玻璃有了厚度
        innerShadow = { InnerShadow(radius = 10.dp, offset = DpOffset(0.dp, (-3).dp), color = Color.Black.copy(alpha = if (style.dark) 0.35f else 0.06f)) },
        layerBlock = {
            val s = 1f + 0.06f * press()
            scaleX = s; scaleY = s
        },
        onDrawSurface = {
            drawRect(androidx.compose.ui.graphics.Brush.verticalGradient(listOf(style.top, style.bottom)))
            drawOutline(shape.createOutline(size, layoutDirection, this), style.rim, style = androidx.compose.ui.graphics.drawscope.Stroke(1.dp.toPx()))
        },
    )

/** 按压的弹性进度（0 → 1 → 0），给 `glass(press = …)` 用。 */
@Composable
fun rememberPress(src: MutableInteractionSource): () -> Float {
    val pressed by src.collectIsPressedAsState()
    val a = remember { Animatable(0f) }
    LaunchedEffect(pressed) { a.animateTo(if (pressed) 1f else 0f, spring(dampingRatio = if (pressed) 0.8f else 0.45f, stiffness = 380f)) }
    return { a.value }
}

/** 一个玻璃容器：内容在玻璃上面。 */
@Composable
fun GlassBox(
    modifier: Modifier = Modifier,
    shape: CornerBasedShape = RoundedCornerShape(50),
    style: GlassStyle = glassStyle(),
    backdrop: Backdrop = LocalBackdrop.current,
    press: () -> Float = { 0f },
    content: @Composable BoxScope.() -> Unit,
) {
    Box(modifier.glass(backdrop, shape, style, press), content = content)
}
