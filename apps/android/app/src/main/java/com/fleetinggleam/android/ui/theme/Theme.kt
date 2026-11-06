package com.fleetinggleam.android.ui.theme

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import com.fleetinggleam.android.R

/**
 * 设计规则在 DESIGN.md：两套调色板（纯黑白，没有色相，层次只来自明暗与字重），琥珀（Gleam Amber）是唯一的强调色、只表示"保留"，
 * 不用红色，不把淘汰的照片变黑白。数值和桌面端 App.css 的 :root 一模一样。
 */
@Immutable
data class FgColors(
    val canvas: Color,
    val panel: Color,
    val surface: Color,
    val surface2: Color,
    val ink: Color,
    val ash: Color,
    val line: Color,
    val onInk: Color,
    /** 只给"保留"。 */
    val gleam: Color,
    val gleamInk: Color,
    val dark: Boolean,
)

/**
 * 浅色（2026-10-22 按 iOS 浅色界面重调）：页面底是 iOS 的 systemGroupedBackground（#F2F2F7）那种浅灰，卡片和对话框是白的，
 * 芯片与输入框是再深一点的灰（iOS 的 tertiarySystemFill）。白底上液态玻璃看不出来，灰底上才有——作者 2026-10-22 的反馈。
 */
val Light = FgColors(
    canvas = Color(0xFFF2F2F7), panel = Color(0xFFFFFFFF), surface = Color(0xFFE6E6EB), surface2 = Color(0xFFD8D8DE),
    ink = Color(0xFF111113), ash = Color(0xFF6E6E76), line = Color(0x1A000000), onInk = Color(0xFFFFFFFF),
    gleam = Color(0xFFDEA13F), gleamInk = Color(0xFF241703), dark = false,
)
val Dark = FgColors(
    canvas = Color(0xFF0E0E10), panel = Color(0xFF151517), surface = Color(0xFF1D1D20), surface2 = Color(0xFF28282C),
    ink = Color(0xFFEDEDEF), ash = Color(0xFF8F8F98), line = Color(0x17FFFFFF), onInk = Color(0xFF0E0E10),
    gleam = Color(0xFFE8B45C), gleamInk = Color(0xFF241703), dark = true,
)

/** 照片墙上的金色亮点（logo 里那颗四角星的渐变，和桌面端 #fg-gold 一样）。 */
val GoldTop = Color(0xFFFFF3C4)
val GoldMid = Color(0xFFFFD87A)
val GoldEnd = Color(0xFFFFB84D)

val LocalFg = staticCompositionLocalOf { Light }

/** Geist（界面与标题，标题收紧字距）与 Geist Mono（每一个分数、计数和尺寸，等宽数字）。不用 Inter，不用衬线体。 */
val Geist = FontFamily(
    Font(R.font.geist_regular, FontWeight.Normal),
    Font(R.font.geist_medium, FontWeight.Medium),
    Font(R.font.geist_semibold, FontWeight.SemiBold),
    Font(R.font.geist_bold, FontWeight.Bold),
)
val GeistMono = FontFamily(
    Font(R.font.geist_mono_regular, FontWeight.Normal),
    Font(R.font.geist_mono_medium, FontWeight.Medium),
)

/**
 * 字号阶梯（移动端）：七档，没有 .5 的字号。
 * 28 大标题 · 20 标题 · 15 页签 / 导航 · 14 正文 · 12 说明 · 11 数字（等宽）· 10 角标。另有两档等宽数字：进度 28、分数 24。
 * 中文回落到系统黑体，同字号比 Geist 显大，所以整体比桌面端收一档。
 */
object Type {
    /** 根页的大标题（首页"项目"、设置）。 */
    val largeTitle = TextStyle(fontFamily = Geist, fontWeight = FontWeight.Bold, fontSize = 28.sp, lineHeight = 34.sp, letterSpacing = (-0.02).em)
    /** 空状态的标语：和大标题同一档。 */
    val display = largeTitle
    /** 页内标题、空状态标语、对话框标题。 */
    val title = TextStyle(fontFamily = Geist, fontWeight = FontWeight.SemiBold, fontSize = 20.sp, lineHeight = 26.sp, letterSpacing = (-0.01).em)
    /** 推进页顶栏的标题、弹层标题。 */
    val navTitle = TextStyle(fontFamily = Geist, fontWeight = FontWeight.SemiBold, fontSize = 15.sp, lineHeight = 20.sp)
    val heading = navTitle
    /** 状态页签（选中时换成 SemiBold）。 */
    val tab = TextStyle(fontFamily = Geist, fontWeight = FontWeight.Normal, fontSize = 15.sp, lineHeight = 20.sp)
    val body = TextStyle(fontFamily = Geist, fontWeight = FontWeight.Normal, fontSize = 14.sp, lineHeight = 20.sp)
    val bodyStrong = body.copy(fontWeight = FontWeight.Medium)
    /** 说明：行说明、提示、分组标题。 */
    val small = TextStyle(fontFamily = Geist, fontWeight = FontWeight.Normal, fontSize = 12.sp, lineHeight = 17.sp)
    /** 按钮文字、文字入口：正文字号、中等字重。 */
    val label = TextStyle(fontFamily = Geist, fontWeight = FontWeight.Medium, fontSize = 14.sp, lineHeight = 18.sp)
    /** 分组标题：灰、说明字号、中等字重；中文没有大小写，不加字距、不大写。 */
    val overline = TextStyle(fontFamily = Geist, fontWeight = FontWeight.Medium, fontSize = 12.sp, lineHeight = 16.sp)
    val rowTitle = body
    val rowDesc = small
    /** 照片上的角标、标签栏与底栏的文字。 */
    val caption = TextStyle(fontFamily = Geist, fontWeight = FontWeight.SemiBold, fontSize = 10.sp, lineHeight = 12.sp)
    val mono = TextStyle(fontFamily = GeistMono, fontWeight = FontWeight.Normal, fontSize = 12.sp, lineHeight = 16.sp)
    val monoSmall = mono.copy(fontSize = 11.sp, lineHeight = 14.sp)
    /** 检查器里的综合分。 */
    val score = mono.copy(fontSize = 24.sp, lineHeight = 28.sp, fontWeight = FontWeight.Medium)
    /** 进度卡上的大数字。 */
    val bigNumber = mono.copy(fontSize = 28.sp, lineHeight = 32.sp, fontWeight = FontWeight.Medium)
}

/**
 * 动效语言（DESIGN.md 第 10 节，和桌面端 src/motion.ts 一样的数）：expo-out 缓动、四个时长、四种弹簧。
 * 时长只用这四档（设计审查 R8）：悬停与颜色 FAST；菜单、弹层、页签切换、进度条 BASE；翻页、改判的光效、光点 SLOW；对焦、背景交叉淡入 DEVELOP。
 * 弹簧的阻尼比不低于 0.65（R7：不弹跳）。
 */
object Motion {
    val easeOut = CubicBezierEasing(0.16f, 1f, 0.3f, 1f)
    const val FAST = 160; const val BASE = 320; const val SLOW = 600; const val DEVELOP = 900
    /** 四档时长的 tween，都用 expo-out。 */
    fun <T> fast() = androidx.compose.animation.core.tween<T>(FAST, easing = easeOut)
    fun <T> base() = androidx.compose.animation.core.tween<T>(BASE, easing = easeOut)
    fun <T> slow() = androidx.compose.animation.core.tween<T>(SLOW, easing = easeOut)
    fun <T> develop() = androidx.compose.animation.core.tween<T>(DEVELOP, easing = easeOut)
    // motion.js 的 stiffness / damping 换算成阻尼比：ζ = damping / (2·√stiffness)
    /** 布局重排、选中框、下划线、胶囊、底栏宽度。 */
    fun <T> glide() = spring<T>(dampingRatio = 0.976f, stiffness = 420f)
    /** 有分量的入场。 */
    fun <T> soft() = spring<T>(dampingRatio = 0.997f, stiffness = 170f)
    /** 照片 ↔ 大图（进入和关闭都用它）。 */
    fun <T> view() = spring<T>(dampingRatio = 0.989f, stiffness = 230f)
    /** 按下的反馈、星的出现、玻璃鼓起。 */
    fun <T> tap() = spring<T>(dampingRatio = 0.653f, stiffness = Spring.StiffnessHigh)
    /** 入场错开：每项 30 ms，最多 8 项，之后的一起出现（R6）。 */
    fun stagger(index: Int): Long = 40L + minOf(index, 8) * 30L
}

/** 设置里"界面动效"选了克制：判定的星光、光圈不播，提示类浮层更淡。 */
val LocalCalm = androidx.compose.runtime.staticCompositionLocalOf { false }

@Composable
fun FleetingGleamTheme(theme: String, content: @Composable () -> Unit) {
    val dark = when (theme) { "light" -> false; "dark" -> true; else -> isSystemInDarkTheme() }
    val c = if (dark) Dark else Light
    val scheme = if (dark) darkColorScheme(
        primary = c.ink, onPrimary = c.onInk, background = c.canvas, onBackground = c.ink, surface = c.canvas, onSurface = c.ink,
        surfaceVariant = c.surface, onSurfaceVariant = c.ash, outline = c.line, secondary = c.ash, surfaceContainer = c.panel, surfaceContainerHigh = c.surface,
    ) else lightColorScheme(
        primary = c.ink, onPrimary = c.onInk, background = c.canvas, onBackground = c.ink, surface = c.canvas, onSurface = c.ink,
        surfaceVariant = c.surface, onSurfaceVariant = c.ash, outline = c.line, secondary = c.ash, surfaceContainer = c.panel, surfaceContainerHigh = c.surface,
    )
    val typography = Typography(
        displayLarge = Type.display, headlineMedium = Type.title, titleMedium = Type.heading, bodyLarge = Type.body, bodyMedium = Type.body,
        bodySmall = Type.small, labelLarge = Type.label, labelMedium = Type.small, labelSmall = Type.monoSmall,
    )
    // 状态栏与导航条的图标跟着应用自己的深浅走（用户可以在设置里选和系统不同的外观；切系统深浅时活动也不重建）
    val view = androidx.compose.ui.platform.LocalView.current
    if (!view.isInEditMode) androidx.compose.runtime.SideEffect {
        val window = (view.context as? android.app.Activity)?.window ?: return@SideEffect
        androidx.core.view.WindowCompat.getInsetsController(window, view).apply {
            isAppearanceLightStatusBars = !dark
            isAppearanceLightNavigationBars = !dark
        }
    }
    CompositionLocalProvider(LocalFg provides c) {
        MaterialTheme(colorScheme = scheme, typography = typography, content = content)
    }
}

val Ellipsis = TextOverflow.Ellipsis
