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

val Light = FgColors(
    canvas = Color(0xFFFFFFFF), panel = Color(0xFFF7F7F8), surface = Color(0xFFEFEFF1), surface2 = Color(0xFFE4E4E7),
    ink = Color(0xFF111113), ash = Color(0xFF6C6C74), line = Color(0x17000000), onInk = Color(0xFFFFFFFF),
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

/** 字号阶梯：桌面端 13px 的正文在手机上放大到 15sp，其余按比例。 */
object Type {
    val display = TextStyle(fontFamily = Geist, fontWeight = FontWeight.SemiBold, fontSize = 30.sp, lineHeight = 36.sp, letterSpacing = (-0.03).em)
    val title = TextStyle(fontFamily = Geist, fontWeight = FontWeight.SemiBold, fontSize = 24.sp, lineHeight = 30.sp, letterSpacing = (-0.02).em)
    val heading = TextStyle(fontFamily = Geist, fontWeight = FontWeight.SemiBold, fontSize = 17.sp, lineHeight = 22.sp, letterSpacing = (-0.01).em)
    val body = TextStyle(fontFamily = Geist, fontWeight = FontWeight.Normal, fontSize = 15.sp, lineHeight = 22.sp)
    val bodyStrong = body.copy(fontWeight = FontWeight.Medium)
    val small = TextStyle(fontFamily = Geist, fontWeight = FontWeight.Normal, fontSize = 13.sp, lineHeight = 19.sp)
    val label = TextStyle(fontFamily = Geist, fontWeight = FontWeight.Medium, fontSize = 14.sp, lineHeight = 18.sp)
    /** 设置分组的小标题：大写、字距放开。 */
    val overline = TextStyle(fontFamily = Geist, fontWeight = FontWeight.Medium, fontSize = 11.5.sp, letterSpacing = 0.06.em)
    /**
     * 设置行的标题与说明：比正文小一号。Geist 没有中文字形，中文回落到系统黑体，同样字号看起来比西文大，
     * 满屏中文的设置页用正文字号会显得挤、显得大（作者 2026-10-21 反馈），所以单独收紧。
     */
    val rowTitle = TextStyle(fontFamily = Geist, fontWeight = FontWeight.Medium, fontSize = 14.sp, lineHeight = 19.sp)
    val rowDesc = TextStyle(fontFamily = Geist, fontWeight = FontWeight.Normal, fontSize = 12.5.sp, lineHeight = 17.sp)
    val mono = TextStyle(fontFamily = GeistMono, fontWeight = FontWeight.Normal, fontSize = 13.sp, lineHeight = 18.sp)
    val monoSmall = mono.copy(fontSize = 11.5.sp, lineHeight = 15.sp)
}

/** 动效语言（DESIGN.md 第 10 节，和桌面端 src/motion.ts 一样的数）：expo-out 缓动、四个时长、四种弹簧。 */
object Motion {
    val easeOut = CubicBezierEasing(0.16f, 1f, 0.3f, 1f)
    const val FAST = 160; const val BASE = 320; const val SLOW = 600; const val DEVELOP = 900
    // motion.js 的 stiffness / damping 换算成阻尼比：ζ = damping / (2·√stiffness)
    /** 布局重排、选中框、胶囊。 */
    fun <T> glide() = spring<T>(dampingRatio = 0.976f, stiffness = 420f)
    /** 有分量的入场。 */
    fun <T> soft() = spring<T>(dampingRatio = 0.997f, stiffness = 170f)
    /** 照片 ↔ 大图。 */
    fun <T> view() = spring<T>(dampingRatio = 0.989f, stiffness = 230f)
    /** 按下的反馈。 */
    fun <T> tap() = spring<T>(dampingRatio = 0.653f, stiffness = Spring.StiffnessHigh)
}

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
