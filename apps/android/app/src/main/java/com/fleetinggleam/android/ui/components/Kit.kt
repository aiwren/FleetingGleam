package com.fleetinggleam.android.ui.components

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.fleetinggleam.android.core.PrefsInfo
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Motion
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

// ---------------------------------------------------------------- 图标：和桌面端同一份 SVG 路径（16 或 12 / 14 / 18 的视框），描边画出

object Icons {
    const val CULL = "M2 4.5A1.5 1.5 0 013.5 3h9A1.5 1.5 0 0114 4.5v7a1.5 1.5 0 01-1.5 1.5h-9A1.5 1.5 0 012 11.5zM2.5 11l3.2-3.2a1 1 0 011.4 0L10 10.7M9.4 10l1.2-1.2a1 1 0 011.4 0l1.5 1.5"
    const val SETTINGS = "M8 5.4a2.6 2.6 0 100 5.2 2.6 2.6 0 000-5.2zM8 1.8l.8 1.6 1.7.4 1.5-.9 1.2 1.2-.9 1.5.4 1.7 1.6.8v1.7l-1.6.8-.4 1.7.9 1.5-1.2 1.2-1.5-.9-1.7.4-.8 1.6H6.3l-.8-1.6-1.7-.4-1.5.9-1.2-1.2.9-1.5-.4-1.7L.4 9.3V7.6L2 6.8l.4-1.7-.9-1.5 1.2-1.2 1.5.9 1.7-.4.8-1.6z"
    /** 12 视框 */
    const val BACK = "M7.5 2.5L4 6l3.5 3.5"
    /** 14 视框 */
    const val TRASH = "M2.5 4h9M5.5 4V2.6h3V4M3.8 4l.5 7.4h5.4L10.2 4M5.8 6.2v3.2M8.2 6.2v3.2"
    /** 18 视框 */
    const val PLUS = "M9 3v12M3 9h12"
    const val EDIT = "M11.3 2.7l2 2L5.5 12.5l-2.8.8.8-2.8zM10 4l2 2"
    const val REMOVE = "M2.5 4.5h11M6 4.5V3a.5.5 0 01.5-.5h3A.5.5 0 0110 3v1.5M4 4.5l.6 8.1a1 1 0 001 .9h4.8a1 1 0 001-.9l.6-8.1M6.5 7v4M9.5 7v4"
    const val FOLDER = "M1.5 4.5A1.5 1.5 0 013 3h3.2l1.6 1.6H13A1.5 1.5 0 0114.5 6v6A1.5 1.5 0 0113 13.5H3A1.5 1.5 0 011.5 12z"
    const val SHARE = "M1.5 5A1.5 1.5 0 013 3.5h10A1.5 1.5 0 0114.5 5v6A1.5 1.5 0 0113 12.5H3A1.5 1.5 0 011.5 11zM1.5 8h13M4.5 10.2h.01M4.5 5.8h.01"
    const val DISK = "M2.5 11.5h11a1 1 0 001-1V8.6a1 1 0 00-.1-.4l-1.9-4A1 1 0 0011.6 3.5H4.4a1 1 0 00-.9.6l-1.9 4a1 1 0 00-.1.4v1.9a1 1 0 001 1zM2.5 8h11M11 9.8h.01"
    const val NAS = "M3 2.5h10a1 1 0 011 1v9a1 1 0 01-1 1H3a1 1 0 01-1-1v-9a1 1 0 011-1zM2 6.3h12M2 9.7h12M5 4.4h.01M5 8h.01M5 11.6h.01"
    const val CHEVRON = "M6 3.5l4.5 4.5L6 12.5"
    const val ALBUM = "M2.5 3.5h11a1 1 0 011 1v7a1 1 0 01-1 1h-11a1 1 0 01-1-1v-7a1 1 0 011-1zM1.5 10l3.4-3.1a1 1 0 011.3 0L9 9.5M8.4 8.9l1.7-1.5a1 1 0 011.3 0l3.1 2.8M10.5 5.6h.01"
    const val SEARCH = "M7 2.5a4.5 4.5 0 100 9 4.5 4.5 0 000-9zM10.3 10.3L13.5 13.5"
    const val SLIDERS = "M3 4.5h6.5M12.5 4.5H13M3 11.5h1M7 11.5h6M12.6 4.5a1.6 1.6 0 11-3.2 0 1.6 1.6 0 013.2 0zM7.1 11.5a1.6 1.6 0 11-3.2 0 1.6 1.6 0 013.2 0z"
    const val FILTERS = "M3.7 2.7h8.6a2 2 0 012 2v6.6a2 2 0 01-2 2H3.7a2 2 0 01-2-2V4.7a2 2 0 012-2zM6 3v10"
    /** 顺时针转：四分之三个圆加一个箭头。 */
    const val ROTATE = "M12.9 9.6A5 5 0 1 1 11.5 4.5M13.5 2.5v3.5H10"
    const val INSPECTOR = "M3.7 2.7h8.6a2 2 0 012 2v6.6a2 2 0 01-2 2H3.7a2 2 0 01-2-2V4.7a2 2 0 012-2zM10 3v10"
    const val CLOSE = "M4 4l8 8M12 4l-8 8"
    const val MORE = "M2.4 8a.8.8 0 1 0 1.6 0a.8.8 0 1 0-1.6 0M7.2 8a.8.8 0 1 0 1.6 0a.8.8 0 1 0-1.6 0M12 8a.8.8 0 1 0 1.6 0a.8.8 0 1 0-1.6 0"
    const val INFO = "M8 2a6 6 0 100 12A6 6 0 008 2zM6.4 6.2a1.7 1.7 0 013.2.6c0 1.1-1.6 1.4-1.6 2.4M8 11.4h.01"
    const val STYLE = "M8 2.2l1.6 3.4 3.7.4-2.8 2.5.8 3.6L8 10.3l-3.3 1.8.8-3.6L2.7 6l3.7-.4z"
}

/** 一个描边图标：桌面端的 SVG 路径原样画出（`box` 是它的视框边长）。 */
@Composable
fun FgIcon(d: String, modifier: Modifier = Modifier, size: Dp = 18.dp, box: Float = 16f, stroke: Float = 1.4f, color: Color = LocalFg.current.ink) {
    val path = remember(d) { PathParser().parsePathString(svgArcs(d)).toPath() }
    Canvas(modifier.size(size)) {
        val k = this.size.minDimension / box
        scale(k, pivot = Offset.Zero) {
            drawPath(path, color, style = Stroke(width = stroke, cap = StrokeCap.Round, join = StrokeJoin.Round))
        }
    }
}

/**
 * 把 SVG 路径里紧凑写法的圆弧标志拆开：浏览器认 `a6 6 0 100 12`（两个标志各一位，后面紧跟坐标），
 * Compose 的 PathParser 会把 `100` 当成一个数，圆就画坏了。这里把每段圆弧的两个标志单独读成一位。
 */
internal fun svgArcs(d: String): String {
    if (d.none { it == 'a' || it == 'A' }) return d
    val out = StringBuilder()
    var i = 0
    var arc = false
    var slot = 0 // 圆弧参数里的第几个：0 rx、1 ry、2 旋转、3 大弧标志、4 方向标志、5 x、6 y
    fun skip() { while (i < d.length && (d[i] == ' ' || d[i] == ',' || d[i] == '\n' || d[i] == '\t')) i++ }
    while (true) {
        skip()
        if (i >= d.length) break
        val ch = d[i]
        if (ch.isLetter() && ch != 'e' && ch != 'E') {
            out.append(ch).append(' ')
            arc = ch == 'a' || ch == 'A'
            slot = 0
            i++
            continue
        }
        if (arc && (slot == 3 || slot == 4)) {
            out.append(ch).append(' ')
            i++
        } else {
            val start = i
            if (d[i] == '-' || d[i] == '+') i++
            var dot = false
            while (i < d.length) {
                val c = d[i]
                if (c.isDigit()) i++
                else if (c == '.' && !dot) { dot = true; i++ }
                else if ((c == 'e' || c == 'E') && i + 1 < d.length) { i++; if (d[i] == '-' || d[i] == '+') i++ }
                else break
            }
            if (i == start) { i++; continue } // 读不懂的字符跳过，免得死循环
            out.append(d, start, i).append(' ')
        }
        if (arc) slot = (slot + 1) % 7
    }
    return out.toString().trim()
}

// ---------------------------------------------------------------- 按钮

/** 这一片界面里的按钮默认用小号（设置页：满屏的行，常规按钮显得大）。 */
val LocalSmallButtons = androidx.compose.runtime.staticCompositionLocalOf { false }

@Composable
fun FgButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    primary: Boolean = false,
    on: Boolean = false,
    small: Boolean = LocalSmallButtons.current,
    enabled: Boolean = true,
    leading: (@Composable () -> Unit)? = null,
) {
    val c = LocalFg.current
    val src = remember { MutableInteractionSource() }
    val pressed by src.collectIsPressedAsState()
    val dy by animateDpAsState(if (pressed && enabled) 1.dp else 0.dp, Motion.tap(), label = "press")
    val solid = primary || on
    val bg = if (solid) c.ink else c.surface
    val fg = if (solid) c.onInk else c.ink
    // 移动端：不描边、实心浅灰底；小号是 30dp 的胶囊（行内操作），常规 44dp、圆角 12（对话框底部、整行的主按钮）
    val shape = if (small) CircleShape else RoundedCornerShape(12.dp)
    val s by animateFloatAsState(if (pressed && enabled) 0.96f else 1f, Motion.tap(), label = "pressScale")
    Row(
        modifier
            .graphicsLayer { alpha = if (enabled) 1f else 0.4f; scaleX = s; scaleY = s; translationY = dy.toPx() * 0f }
            .clip(shape)
            .background(bg)
            .clickable(interactionSource = src, indication = null, enabled = enabled, role = Role.Button, onClick = onClick)
            .heightIn(min = if (small) 30.dp else 44.dp)
            .padding(horizontal = if (small) 12.dp else 18.dp, vertical = if (small) 5.dp else 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally),
    ) {
        leading?.invoke()
        Text(text, style = (if (small) Type.small else Type.label).copy(color = fg, fontWeight = if (solid) FontWeight.SemiBold else FontWeight.Medium), maxLines = 1, overflow = Ellipsis)
    }
}

/** 只有图标的按钮（工具栏）。`on` 时实心墨色。 */
@Composable
fun IconBtn(d: String, label: String, onClick: () -> Unit, modifier: Modifier = Modifier, on: Boolean = false, enabled: Boolean = true, box: Float = 16f, plain: Boolean = false) {
    val c = LocalFg.current
    Box(
        modifier
            .size(40.dp)
            .graphicsLayer { alpha = if (enabled) 1f else 0.4f }
            .clip(CircleShape)
            .background(if (on) c.ink else if (plain) Color.Transparent else c.surface)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        FgIcon(d, size = 20.dp, box = box, stroke = 1.5f, color = if (on) c.onInk else c.ink)
    }
}

// ---------------------------------------------------------------- 开关、分段、步进

@Composable
fun FgSwitch(checked: Boolean, onChange: (Boolean) -> Unit, label: String, enabled: Boolean = true) {
    val c = LocalFg.current
    val x by animateDpAsState(if (checked) 20.dp else 0.dp, Motion.glide(), label = "switch")
    Box(
        Modifier
            .size(width = 46.dp, height = 26.dp)
            .graphicsLayer { alpha = if (enabled) 1f else 0.45f }
            .clip(CircleShape)
            .background(if (checked) c.ink else c.surface2)
            .clickable(enabled = enabled, role = Role.Switch) { onChange(!checked) }
            .semantics { contentDescription = label }
            .padding(3.dp),
    ) {
        Box(Modifier.offset(x = x).size(20.dp).clip(CircleShape).background(if (checked) c.onInk else c.ash))
    }
}

/** 分段控件：选中的那一格下面有一块随手势滑过去的底（桌面端的 glide 胶囊）。 */
@Composable
fun <T> Segmented(options: List<Pair<T, String>>, value: T, onChange: (T) -> Unit, modifier: Modifier = Modifier, block: Boolean = false) {
    val c = LocalFg.current
    val index = options.indexOfFirst { it.first == value }.coerceAtLeast(0)
    BoxWithConstraints(
        modifier
            .then(if (block) Modifier.fillMaxWidth() else Modifier)
            .clip(RoundedCornerShape(10.dp))
            .background(c.surface)
            .padding(2.dp),
    ) {
        if (block) {
            val w = maxWidth / options.size
            val x by animateDpAsState(w * index, Motion.glide(), label = "seg")
            Box(Modifier.offset(x = x).width(w).height(30.dp).shadow(1.dp, RoundedCornerShape(8.dp)).clip(RoundedCornerShape(8.dp)).background(c.canvas))
            Row(Modifier.fillMaxWidth()) {
                options.forEach { (v, label) ->
                    Box(Modifier.weight(1f).height(30.dp).clip(RoundedCornerShape(8.dp)).clickable { onChange(v) }, contentAlignment = Alignment.Center) {
                        Text(label, style = Type.small.copy(color = if (v == value) c.ink else c.ash, fontWeight = FontWeight.Medium), maxLines = 1, overflow = Ellipsis)
                    }
                }
            }
        } else {
            Row {
                options.forEach { (v, label) ->
                    val sel = v == value
                    Box(
                        Modifier.height(30.dp).then(if (sel) Modifier.shadow(1.dp, RoundedCornerShape(8.dp)) else Modifier).clip(RoundedCornerShape(8.dp)).background(if (sel) c.canvas else Color.Transparent)
                            .clickable { onChange(v) }.padding(horizontal = 12.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(label, style = Type.small.copy(color = if (sel) c.ink else c.ash, fontWeight = FontWeight.Medium), maxLines = 1)
                    }
                }
            }
        }
    }
}

/** 左右带 − / + 的数字框；超出范围夹回 min / max。 */
@Composable
fun Stepper(value: Int, onChange: (Int) -> Unit, label: String, min: Int = 1, max: Int = 1000, enabled: Boolean = true) {
    val c = LocalFg.current
    Row(
        Modifier.graphicsLayer { alpha = if (enabled) 1f else 0.4f }.clip(CircleShape).background(c.surface)
            .semantics { contentDescription = label },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(32.dp).clickable(enabled = enabled && value > min) { onChange((value - 1).coerceIn(min, max)) }, contentAlignment = Alignment.Center) { FgIcon("M2.5 6h7", size = 14.dp, box = 12f, stroke = 1.6f) }
        Text("$value", style = Type.mono.copy(color = c.ink), textAlign = androidx.compose.ui.text.style.TextAlign.Center, modifier = Modifier.widthIn(min = 28.dp))
        Box(Modifier.size(32.dp).clickable(enabled = enabled && value < max) { onChange((value + 1).coerceIn(min, max)) }, contentAlignment = Alignment.Center) { FgIcon("M2.5 6h7M6 2.5v7", size = 14.dp, box = 12f, stroke = 1.6f) }
    }
}

// ---------------------------------------------------------------- 设置的分组与行

@Composable
fun SettingGroup(title: String, content: @Composable () -> Unit) {
    val c = LocalFg.current
    Column(Modifier.fillMaxWidth()) {
        if (title.isNotEmpty()) Text(title, style = Type.overline.copy(color = c.ash), modifier = Modifier.padding(start = 16.dp, bottom = 6.dp))
        Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(c.panel)) { content() }
    }
}

/** 一项设置：标题加一句平实的话；技术细节放到"?"后面。行与行之间一条细线。 */
@Composable
fun SettingRow(label: String, desc: String? = null, info: String? = null, first: Boolean = false, below: (@Composable () -> Unit)? = null, onClick: (() -> Unit)? = null, icon: String? = null, control: (@Composable RowScope.() -> Unit)? = null) {
    val c = LocalFg.current
    Column(Modifier.fillMaxWidth()) {
        // 行与行之间一条从文字起头处开始的细线（iOS 分组列表的样子）
        if (!first) Box(Modifier.padding(start = if (icon != null) 56.dp else 16.dp).fillMaxWidth().height(0.5.dp).background(c.line))
        Row(Modifier.fillMaxWidth().then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier).heightIn(min = 50.dp).padding(horizontal = 16.dp, vertical = 11.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            if (icon != null) Box(Modifier.size(28.dp).clip(RoundedCornerShape(8.dp)).background(c.ink), contentAlignment = Alignment.Center) { FgIcon(icon, size = 16.dp, stroke = 1.5f, color = c.onInk) }
            Column(Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(label, style = Type.rowTitle.copy(color = c.ink))
                    if (info != null) InfoPop(label, info)
                }
                if (desc != null) Text(desc, style = Type.rowDesc.copy(color = c.ash), modifier = Modifier.padding(top = 2.dp))
            }
            if (control != null) Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp), content = control)
        }
        if (below != null) Box(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp)) { below() }
    }
}

/** "?"：点开是一段说明（手机上没有悬停）。 */
@Composable
fun InfoPop(title: String, text: String) {
    val c = LocalFg.current
    var open by remember { mutableStateOf(false) }
    Box(Modifier.padding(start = 4.dp).size(22.dp).clip(CircleShape).clickable { open = true }.semantics { contentDescription = title }, contentAlignment = Alignment.Center) {
        FgIcon(Icons.INFO, size = 14.dp, color = c.ash, stroke = 1.3f)
    }
    FgDialog(open, onDismiss = { open = false }, title = title, footer = { FgButton(t("more.close"), { open = false }, primary = true, small = true) }) {
        Text(text, style = Type.body.copy(color = c.ash))
    }
}

/** 一个小标签（"已安装"、"已生效"）；`on` 时实心。 */
@Composable
fun Ktag(text: String, on: Boolean = false, gleam: Boolean = false) {
    val c = LocalFg.current
    Text(
        text,
        style = Type.monoSmall.copy(color = if (gleam) c.gleamInk else if (on) c.onInk else c.ash),
        modifier = Modifier.clip(CircleShape).background(if (gleam) c.gleam else if (on) c.ink else c.surface).border(1.dp, if (on || gleam) Color.Transparent else c.line, CircleShape).padding(horizontal = 9.dp, vertical = 3.dp),
        maxLines = 1,
    )
}

/** 连接状态的小圆点加文字（NAS、备份目标）。 */
@Composable
fun StateDot(text: String, on: Boolean) {
    val c = LocalFg.current
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(7.dp).clip(CircleShape).background(if (on) c.ink else c.surface2).border(1.dp, c.line, CircleShape))
        Text(text, style = Type.small.copy(color = c.ash), modifier = Modifier.padding(start = 6.dp), maxLines = 1)
    }
}

// ---------------------------------------------------------------- 进度线：细，墨色，前沿带一点暖光（"bar live"）

@Composable
fun LiveBar(fraction: Float, modifier: Modifier = Modifier, glint: Boolean = true) {
    val c = LocalFg.current
    val f by animateFloatAsState(fraction.coerceIn(0f, 1f), tween(500, easing = Motion.easeOut), label = "bar")
    Canvas(modifier.fillMaxWidth().height(4.dp)) {
        val r = androidx.compose.ui.geometry.CornerRadius(size.height / 2)
        drawRoundRect(c.surface2, cornerRadius = r)
        val w = maxOf(size.height, size.width * f)
        if (f <= 0f) return@Canvas
        // 已走过的一段：墨色，前端一小段渐变到暖光（只在这一段里，和进度条一样高，两端圆头）
        val gw = minOf(w, 40.dp.toPx())
        val brush = if (glint && f < 1f) Brush.horizontalGradient(0f to c.ink, ((w - gw) / w).coerceIn(0f, 1f) to c.ink, 1f to c.gleam, startX = 0f, endX = w) else androidx.compose.ui.graphics.SolidColor(c.ink)
        drawRoundRect(brush, size = size.copy(width = w), cornerRadius = r)
    }
}

/** 滚动的数字（进度与计数），每一位单独滚。 */
@Composable
fun Ticker(value: Int, style: TextStyle = Type.title) {
    Row {
        value.toString().forEachIndexed { i, ch ->
            AnimatedContent(ch, transitionSpec = { (slideInVertically { it } + fadeIn()) togetherWith (slideOutVertically { -it } + fadeOut()) }, label = "digit$i") { d ->
                Text(d.toString(), style = style.copy(fontFamily = com.fleetinggleam.android.ui.theme.GeistMono))
            }
        }
    }
}

// ---------------------------------------------------------------- 输入框

@Composable
fun FgField(
    value: String,
    onChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    label: String? = null,
    placeholder: String = "",
    password: Boolean = false,
    singleLine: Boolean = true,
    minLines: Int = 1,
    keyboard: KeyboardType = KeyboardType.Text,
    onDone: (() -> Unit)? = null,
    enabled: Boolean = true,
) {
    val c = LocalFg.current
    Column(modifier) {
        if (label != null) Text(label, style = Type.small.copy(color = c.ash), modifier = Modifier.padding(bottom = 6.dp))
        Box(Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).background(c.panel).border(1.dp, c.line, RoundedCornerShape(10.dp)).padding(horizontal = 12.dp, vertical = 11.dp)) {
            if (value.isEmpty()) Text(placeholder, style = Type.body.copy(color = c.ash.copy(alpha = 0.7f)), maxLines = if (singleLine) 1 else 4)
            BasicTextField(
                value, onChange, enabled = enabled, singleLine = singleLine, minLines = minLines,
                textStyle = Type.body.copy(color = c.ink), cursorBrush = SolidColor(c.ink),
                visualTransformation = if (password) PasswordVisualTransformation() else VisualTransformation.None,
                keyboardOptions = KeyboardOptions(keyboardType = if (password) KeyboardType.Password else keyboard, imeAction = if (onDone != null) ImeAction.Done else ImeAction.Default, autoCorrectEnabled = false),
                keyboardActions = KeyboardActions(onDone = { onDone?.invoke() }),
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

// ---------------------------------------------------------------- 对话框：标题、正文、底部按钮（桌面端的 Modal）

@Composable
fun FgDialog(open: Boolean, onDismiss: () -> Unit, title: String, locked: Boolean = false, footer: (@Composable RowScope.() -> Unit)? = null, content: @Composable () -> Unit) {
    if (!open) return
    val c = LocalFg.current
    Dialog(onDismissRequest = { if (!locked) onDismiss() }, properties = DialogProperties(usePlatformDefaultWidth = false, dismissOnBackPress = !locked, dismissOnClickOutside = !locked)) {
        Column(
            Modifier.padding(20.dp).widthIn(max = 520.dp).fillMaxWidth().clip(RoundedCornerShape(18.dp)).background(c.panel),
        ) {
            Text(title, style = Type.heading.copy(color = c.ink), modifier = Modifier.padding(start = 20.dp, end = 20.dp, top = 20.dp, bottom = 10.dp))
            Column(Modifier.weight(1f, fill = false).verticalScroll(rememberScrollState()).padding(horizontal = 20.dp)) {
                content()
                Spacer(Modifier.height(16.dp))
            }
            if (footer != null) {
                Box(Modifier.fillMaxWidth().height(1.dp).background(c.line))
                Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp), horizontalArrangement = Arrangement.spacedBy(10.dp, Alignment.End), verticalAlignment = Alignment.CenterVertically, content = footer)
            }
        }
    }
}

// ---------------------------------------------------------------- 浮在底部的一句话（带"撤销"之类的按钮）

data class ToastMsg(val text: String, val action: String? = null, val onAction: (() -> Unit)? = null, val id: Long = System.nanoTime())

object Toasts {
    private val _msg = MutableStateFlow<ToastMsg?>(null)
    val msg: StateFlow<ToastMsg?> = _msg
    fun show(text: String, action: String? = null, onAction: (() -> Unit)? = null) { _msg.value = ToastMsg(text, action, onAction) }
    fun clear(id: Long) { if (_msg.value?.id == id) _msg.value = null }
}

@Composable
fun ToastHost(bottom: Dp) {
    val c = LocalFg.current
    val m by Toasts.msg.collectAsState()
    LaunchedEffect(m?.id) { val id = m?.id ?: return@LaunchedEffect; delay(6000); Toasts.clear(id) }
    Box(Modifier.fillMaxSize().navigationBarsPadding().padding(bottom = bottom), contentAlignment = Alignment.BottomCenter) {
        AnimatedVisibility(m != null, enter = fadeIn() + slideInVertically { it / 2 }, exit = fadeOut() + slideOutVertically { it / 3 }) {
            val msg = m ?: return@AnimatedVisibility
            Row(
                Modifier.padding(horizontal = 20.dp).clip(RoundedCornerShape(14.dp)).background(c.ink).padding(start = 16.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(msg.text, style = Type.small.copy(color = c.onInk), modifier = Modifier.padding(vertical = 8.dp).weight(1f, fill = false), maxLines = 2, overflow = Ellipsis)
                if (msg.action != null) Text(msg.action, style = Type.label.copy(color = c.onInk, fontWeight = FontWeight.SemiBold, textDecoration = TextDecoration.Underline), modifier = Modifier.clip(RoundedCornerShape(8.dp)).clickable { msg.onAction?.invoke(); Toasts.clear(msg.id) }.padding(horizontal = 12.dp, vertical = 8.dp))
                else Spacer(Modifier.width(10.dp))
            }
        }
    }
}

// ---------------------------------------------------------------- 学习进度：一条轨道、三个刻度、一句还差什么（桌面端的 LearnMeter）

@Composable
fun LearnMeter(info: PrefsInfo, compact: Boolean = false) {
    val c = LocalFg.current
    val min = info.minPerClass.takeIf { it > 0 } ?: 3
    val full = info.fullAt.takeIf { it > 0 } ?: 81
    val half = info.halfAt.takeIf { it > 0 } ?: 29
    val start = 2 * min
    val imported = info.imported
    val have = maxOf(info.trained, info.labels + info.refs + imported)
    val keeps = info.keeps + info.refs + imported
    val rejects = info.rejects + imported
    val needKeep = maxOf(0, min - keeps)
    val needReject = maxOf(0, min - rejects)
    val status = when {
        info.active -> t("meter.active", "pct" to Math.round(info.strength * 100), "max" to Math.round(info.maxStrength * 100)) + if (have < full) " · " + t("meter.toFull", "n" to full - have) else ""
        needKeep > 0 && needReject > 0 -> t("meter.needBoth", "keep" to needKeep, "reject" to needReject, "min" to min)
        needKeep > 0 -> t("meter.needKeep", "keep" to needKeep, "min" to min)
        needReject > 0 -> t("meter.needReject", "reject" to needReject, "min" to min)
        else -> t("meter.pending")
    }
    val marks = listOf(start to t("meter.markStart"), half to t("meter.markHalf"), full to t("meter.markFull"))
    Column(Modifier.fillMaxWidth().semantics { contentDescription = t("meter.aria", "have" to have, "full" to full) }) {
        BoxWithConstraints(Modifier.fillMaxWidth().height(if (compact) 26.dp else 32.dp)) {
            val w = maxWidth
            Box(Modifier.fillMaxWidth().height(4.dp).clip(CircleShape).background(c.surface2))
            Box(Modifier.width(w * (have.toFloat() / full).coerceIn(0f, 1f)).height(4.dp).clip(CircleShape).background(c.ink))
            marks.forEachIndexed { i, (at, label) ->
                val x = w * (at.toFloat() / full)
                val reached = have >= at
                Box(Modifier.offset(x = x - 3.dp, y = (-2).dp).size(8.dp).clip(CircleShape).background(if (reached) c.ink else c.canvas).border(1.5.dp, if (reached) c.ink else c.surface2, CircleShape))
                if (!compact) {
                    val align = when (i) { 0 -> Alignment.TopStart; 1 -> Alignment.TopCenter; else -> Alignment.TopEnd }
                    Box(Modifier.fillMaxWidth().offset(y = 10.dp), contentAlignment = align) {
                        Text("$at · $label", style = Type.monoSmall.copy(color = if (reached) c.ink else c.ash), modifier = if (i == 1) Modifier.offset(x = x - w / 2) else Modifier)
                    }
                }
            }
        }
        Text(status, style = Type.small.copy(color = c.ash), modifier = Modifier.padding(top = 4.dp))
    }
}

/** 页面的标题块：大标题，一句说明。 */
@Composable
fun PageHead(title: String, subtitle: String? = null, modifier: Modifier = Modifier) {
    val c = LocalFg.current
    Column(modifier) {
        Text(title, style = Type.title.copy(color = c.ink))
        if (subtitle != null) Text(subtitle, style = Type.small.copy(color = c.ash), modifier = Modifier.padding(top = 4.dp))
    }
}

/** 内容区四边的留白（手机上 20dp，底部给悬浮标签栏让位）。 */
val PagePadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 110.dp)

// ---------------------------------------------------------------- 移动端的导航件（docs/android-ux.md）

/** 分组列表里通往下一页的一行：左边一个小方块图标，标题，右边当前值和 ›。 */
@Composable
fun NavRow(icon: String, title: String, value: String? = null, first: Boolean = false, box: Float = 16f, onClick: () -> Unit) {
    val c = LocalFg.current
    Column(Modifier.fillMaxWidth()) {
        if (!first) Box(Modifier.padding(start = 56.dp).fillMaxWidth().height(0.5.dp).background(c.line))
        Row(Modifier.fillMaxWidth().clickable(onClick = onClick).heightIn(min = 52.dp).padding(horizontal = 16.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(28.dp).clip(RoundedCornerShape(8.dp)).background(c.ink), contentAlignment = Alignment.Center) { FgIcon(icon, size = 16.dp, box = box, stroke = 1.5f, color = c.onInk) }
            Text(title, style = Type.rowTitle.copy(color = c.ink), modifier = Modifier.weight(1f).padding(start = 12.dp), maxLines = 1, overflow = Ellipsis)
            if (value != null) Text(value, style = Type.small.copy(color = c.ash), maxLines = 1, overflow = Ellipsis, modifier = Modifier.padding(start = 8.dp).widthIn(max = 180.dp))
            FgIcon(Icons.CHEVRON, size = 14.dp, stroke = 1.6f, color = c.ash, modifier = Modifier.padding(start = 6.dp))
        }
    }
}

/** 推进页的顶栏：左边 ‹ 和上一页的名字，中间这一页的标题，右边可放按钮。48dp 高。 */
@Composable
fun NavBar(title: String, back: String?, onBack: (() -> Unit)?, modifier: Modifier = Modifier, onTitle: (() -> Unit)? = null, trailing: (@Composable RowScope.() -> Unit)? = null) {
    val c = LocalFg.current
    Box(modifier.fillMaxWidth().height(48.dp).padding(horizontal = 6.dp)) {
        if (onBack != null) Row(
            Modifier.align(Alignment.CenterStart).clip(CircleShape).clickable(onClick = onBack).padding(start = 6.dp, end = 10.dp, top = 8.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            FgIcon(Icons.BACK, size = 18.dp, box = 12f, stroke = 1.5f)
            if (back != null) Text(back, style = Type.rowTitle.copy(color = c.ink), modifier = Modifier.padding(start = 2.dp), maxLines = 1)
        }
        Text(title, style = Type.navTitle.copy(color = c.ink), maxLines = 1, overflow = Ellipsis,
            modifier = Modifier.align(Alignment.Center).padding(horizontal = 96.dp).clip(RoundedCornerShape(8.dp)).then(if (onTitle != null) Modifier.clickable(onClick = onTitle) else Modifier).padding(horizontal = 6.dp, vertical = 4.dp))
        if (trailing != null) Row(Modifier.align(Alignment.CenterEnd), verticalAlignment = Alignment.CenterVertically, content = trailing)
    }
}

/** ⋯ 按钮带一个玻璃菜单；`items` 是 (文字, 是否可点, 动作)。`dot`：有新东西（比如偏好变了可以重新评估）。`glass`：按钮本身也是玻璃（浮在内容上的顶栏里用）。 */
@Composable
fun MenuButton(label: String, items: List<Triple<String, Boolean, () -> Unit>>, dot: Boolean = false, glass: Boolean = false) {
    val c = LocalFg.current
    val overlay = LocalOverlay.current
    val anchor = rememberAnchor()
    val show = { overlay.showMenu(anchor.value, items.map { (t, e, a) -> MenuItem(t, e, a) }, alignEnd = true) }
    if (glass) GlassIconBtn(Icons.MORE, label, show, Modifier.anchor(anchor), dot = dot)
    else Box(Modifier.anchor(anchor)) {
        IconBtn(Icons.MORE, label, show, plain = true)
        if (dot) Box(Modifier.align(Alignment.TopEnd).padding(top = 9.dp, end = 9.dp).size(7.dp).clip(CircleShape).background(c.ink))
    }
}
