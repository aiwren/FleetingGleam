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
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.layout.layout
import androidx.compose.ui.input.pointer.pointerInput
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.Type
import androidx.compose.runtime.setValue
import androidx.compose.material3.Text
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.Alignment
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.border
import androidx.compose.ui.draw.shadow
import androidx.compose.foundation.clickable
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Column
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
    val blur: Dp = 2.dp, val lensHeight: Dp = 24.dp, val lensAmount: Dp = 56.dp, val shadow: Boolean = true, val dark: Boolean = false,
)

/**
 * 浅色下玻璃怎么才看得出来（照 iOS 26 在浅色界面上的做法）：玻璃本身不是白的，而是带一点灰（Apple 在纯白上的玻璃约等于 #F0F0F3），
 * 边缘有一道很亮的镜面高光（左上最亮），下面一道大而柔的投影把它托起来，内侧一圈淡淡的暗边给出厚度；内容滚过去时边缘把它弯一下。
 * `dark` 为 null 时跟随主题；查看器那种暗房间里传 true。
 */
@Composable
fun glassStyle(strong: Boolean = false, dark: Boolean? = null): GlassStyle {
    val c = LocalFg.current
    return if (dark ?: c.dark) GlassStyle(
        top = Color(0xFF2A2A30).copy(alpha = if (strong) 0.55f else 0.32f), bottom = Color(0xFF0E0E10).copy(alpha = if (strong) 0.6f else 0.38f),
        rim = Color.White.copy(alpha = 0.14f), dark = true,
    ) else GlassStyle(
        // 浅色的页面底是 iOS 那种浅灰（#F2F2F7），玻璃比它亮一点、边上一圈高光，就像 iOS 浅色下的样子
        top = Color.White.copy(alpha = if (strong) 0.72f else 0.55f), bottom = Color(0xFFF4F4F7).copy(alpha = if (strong) 0.62f else 0.36f),
        rim = Color.Black.copy(alpha = 0.07f),
    )
}

/**
 * 一块玻璃。`press` 在 0..1 之间：按下时玻璃像一滴水那样微微鼓起（放大一点、边缘折射更强），松手弹回。
 */
/** 全局开关：设置 → 外观 → 液态玻璃。关掉时玻璃件退成普通磨砂片（只有底色、描边和投影，不采样背后内容）。 */
val LocalGlassOn = staticCompositionLocalOf { true }

fun Modifier.glass(backdrop: Backdrop, shape: CornerBasedShape, style: GlassStyle, press: () -> Float = { 0f }, on: Boolean = true): Modifier =
    if (!on) this
        .graphicsLayer { val s = 1f + 0.06f * press(); scaleX = s; scaleY = s }
        .then(if (style.shadow) Modifier.shadow(12.dp, shape, ambientColor = Color.Black.copy(alpha = 0.18f), spotColor = Color.Black.copy(alpha = 0.22f)) else Modifier)
        .clip(shape)
        .background(if (style.dark) Color(0xFF1C1C20).copy(alpha = 0.94f) else Color.White.copy(alpha = 0.94f))
        .border(1.dp, style.rim, shape)
    else drawBackdrop(
        backdrop = backdrop,
        shape = { shape },
        effects = {
            vibrancy()
            blur(style.blur.toPx())
            val p = press()
            lens((style.lensHeight * (1f + 0.35f * p)).toPx(), (style.lensAmount * (1f + 0.6f * p)).toPx(), depthEffect = true, chromaticAberration = true)
        },
        // 一圈随角度变亮的镜面高光（左上、右下最亮），比默认的更宽更亮
        // 真机上 1.5dp 满白的高光是一圈很粗的白边（作者 2026-10-22）：收成 0.8dp、半透明，只在斜角上亮
        highlight = { Highlight(width = 0.8.dp, blurRadius = 0.6.dp, alpha = if (style.dark) 0.45f else 0.5f, style = HighlightStyle.Default(color = Color.White, angle = 45f, falloff = 2.2f)) },
        shadow = if (style.shadow) ({ Shadow(radius = 22.dp, offset = DpOffset(0.dp, 10.dp), color = Color.Black.copy(alpha = if (style.dark) 0.5f else 0.17f)) }) else null,
        // 内阴影：边缘向里一点点暗下去，玻璃有了厚度
        innerShadow = { InnerShadow(radius = 8.dp, offset = DpOffset(0.dp, (-3).dp), color = Color.Black.copy(alpha = if (style.dark) 0.35f else 0.09f)) },
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
    Box(modifier.glass(backdrop, shape, style, press, on = LocalGlassOn.current), content = content)
}

// ---------------------------------------------------------------- 覆盖层：菜单这类浮在最上面的玻璃件
//
// Popup / DropdownMenu 是另一个窗口，拿不到它背后的内容，画不出折射；所以菜单画在 AppShell 根部的覆盖层里，
// 和标签栏一样在内容层之外、能折射整页。

data class MenuItem(val text: String, val enabled: Boolean = true, val action: () -> Unit)

class OverlayState {
    var content by androidx.compose.runtime.mutableStateOf<(@Composable () -> Unit)?>(null)
    fun dismiss() { content = null }
    /** 从 `anchor`（根坐标）下方弹出一个玻璃菜单；放不下就弹在上方。`dark` 强制深色玻璃（暗房间里用）。 */
    fun showMenu(anchor: androidx.compose.ui.geometry.Rect, items: List<MenuItem>, alignEnd: Boolean = true, dark: Boolean? = null) {
        content = { GlassMenu(anchor, items, alignEnd, dark) { content = null } }
    }
}

val LocalOverlay = staticCompositionLocalOf { OverlayState() }

/** 记下一个控件在根坐标里的位置，给 `showMenu` 当锚点。 */
@Composable
fun rememberAnchor(): androidx.compose.runtime.MutableState<androidx.compose.ui.geometry.Rect> = remember { androidx.compose.runtime.mutableStateOf(androidx.compose.ui.geometry.Rect.Zero) }

fun Modifier.anchor(state: androidx.compose.runtime.MutableState<androidx.compose.ui.geometry.Rect>): Modifier =
    onGloballyPositioned { state.value = it.boundsInRoot() }

@Composable
private fun GlassMenu(anchor: androidx.compose.ui.geometry.Rect, items: List<MenuItem>, alignEnd: Boolean, dark: Boolean?, onDismiss: () -> Unit) {
    val c = LocalFg.current
    val style = glassStyle(strong = true, dark = dark)
    val ink = if (dark == true) com.fleetinggleam.android.ui.theme.Dark.ink else c.ink
    val ash = if (dark == true) com.fleetinggleam.android.ui.theme.Dark.ash else c.ash
    val line = if (dark == true) com.fleetinggleam.android.ui.theme.Dark.line else c.line
    val a = remember { Animatable(0f) }
    LaunchedEffect(Unit) { a.animateTo(1f, spring(dampingRatio = 0.78f, stiffness = 520f)) }
    androidx.activity.compose.BackHandler(onBack = onDismiss)
    Box(
        Modifier.fillMaxSize().pointerInput(Unit) { detectTapGestures { onDismiss() } },
    ) {
        Box(
            Modifier
                .layout { m, cs ->
                    val pl = m.measure(cs.copy(minWidth = 0, minHeight = 0, maxWidth = (cs.maxWidth * 0.7f).toInt()))
                    val gap = 6.dp.roundToPx(); val edge = 10.dp.roundToPx()
                    val x = (if (alignEnd) anchor.right - pl.width else anchor.left).toInt().coerceIn(edge, cs.maxWidth - pl.width - edge)
                    val below = anchor.bottom.toInt() + gap
                    val y = if (below + pl.height <= cs.maxHeight - edge) below else (anchor.top.toInt() - gap - pl.height).coerceAtLeast(edge)
                    layout(cs.maxWidth, cs.maxHeight) { pl.placeRelative(x, y) }
                }
                .graphicsLayer {
                    // 从锚点那一角长出来
                    transformOrigin = androidx.compose.ui.graphics.TransformOrigin(if (alignEnd) 1f else 0f, 0f)
                    val s = 0.88f + 0.12f * a.value; scaleX = s; scaleY = s; alpha = a.value.coerceIn(0f, 1f)
                }
                .glass(LocalBackdrop.current, RoundedCornerShape(16.dp), style, on = LocalGlassOn.current),
        ) {
            Column(Modifier.widthIn(min = 168.dp)) {
                items.forEachIndexed { i, it ->
                    if (i > 0) Box(Modifier.fillMaxWidth().height(0.5.dp).background(line))
                    Text(
                        it.text, style = Type.rowTitle.copy(color = if (it.enabled) ink else ash), maxLines = 1,
                        modifier = Modifier.fillMaxWidth().clickable(enabled = it.enabled) { onDismiss(); it.action() }.padding(horizontal = 16.dp, vertical = 12.dp),
                    )
                }
            }
        }
    }
}

// ---------------------------------------------------------------- 玻璃的顶栏（iOS 26：按钮是浮在内容上的玻璃胶囊，内容从下面滚过去）

/** 圆形的玻璃图标按钮。 */
@Composable
fun GlassIconBtn(icon: String, label: String, onClick: () -> Unit, modifier: Modifier = Modifier, on: Boolean = false, enabled: Boolean = true, box: Float = 16f, dot: Boolean = false) {
    val c = LocalFg.current
    val src = remember { MutableInteractionSource() }
    val press = rememberPress(src)
    Box(modifier) {
        GlassBox(Modifier.size(40.dp).graphicsLayer { alpha = if (enabled) 1f else 0.45f }, shape = androidx.compose.foundation.shape.CircleShape, press = press) {
            Box(
                Modifier.fillMaxSize().clip(androidx.compose.foundation.shape.CircleShape).background(if (on) c.ink else Color.Transparent)
                    .clickable(interactionSource = src, indication = null, enabled = enabled, onClick = onClick)
                    .semantics { contentDescription = label },
                contentAlignment = Alignment.Center,
            ) { FgIcon(icon, size = 19.dp, box = box, stroke = 1.5f, color = if (on) c.onInk else c.ink) }
        }
        if (dot) Box(Modifier.align(Alignment.TopEnd).padding(top = 4.dp, end = 4.dp).size(8.dp).clip(androidx.compose.foundation.shape.CircleShape).background(c.ink))
    }
}

/**
 * 浮在内容上的顶栏：左边一粒"‹ 上一页"的玻璃胶囊，中间标题，右边几粒玻璃圆钮。放在内容的 Box 里、内容自己留出顶部的高度。
 */
@Composable
fun GlassNavBar(title: String, back: String?, onBack: (() -> Unit)?, modifier: Modifier = Modifier, onTitle: (() -> Unit)? = null, trailing: (@Composable RowScope.() -> Unit)? = null) {
    val c = LocalFg.current
    Box(modifier.fillMaxWidth().height(52.dp).padding(horizontal = 10.dp)) {
        if (onBack != null) {
            val src = remember { MutableInteractionSource() }
            val press = rememberPress(src)
            GlassBox(Modifier.align(Alignment.CenterStart), press = press) {
                Row(
                    Modifier.clip(RoundedCornerShape(50)).clickable(interactionSource = src, indication = null, onClick = onBack).padding(start = 10.dp, end = 14.dp, top = 9.dp, bottom = 9.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    FgIcon(Icons.BACK, size = 16.dp, box = 12f, stroke = 1.6f)
                    if (back != null) Text(back, style = Type.rowTitle.copy(color = c.ink, fontWeight = FontWeight.Medium), modifier = Modifier.padding(start = 2.dp), maxLines = 1)
                }
            }
        }
        Text(
            title, style = Type.navTitle.copy(color = c.ink), maxLines = 1, overflow = Ellipsis,
            modifier = Modifier.align(Alignment.Center).padding(horizontal = 104.dp).clip(RoundedCornerShape(8.dp)).then(if (onTitle != null) Modifier.clickable(onClick = onTitle) else Modifier).padding(horizontal = 6.dp, vertical = 4.dp),
        )
        if (trailing != null) Row(Modifier.align(Alignment.CenterEnd), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp), content = trailing)
    }
}
