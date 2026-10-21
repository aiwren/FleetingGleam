package com.fleetinggleam.android.core

import android.icu.text.ListFormatter
import android.icu.text.MeasureFormat
import android.icu.util.Measure
import android.icu.util.MeasureUnit
import java.text.DateFormat
import java.util.Date
import java.util.Locale

private fun locale(): Locale = if (I18n.lang.value.startsWith("zh")) Locale.SIMPLIFIED_CHINESE else Locale.ENGLISH

/** A size in bytes the way the language writes it, through ICU (the same CLDR data as the Mac's `Intl`), not hand-written units. */
fun fmtBytes(b: Long): String {
    val (unit, div, digits) = when {
        b >= 1_000_000_000L -> Triple(MeasureUnit.GIGABYTE, 1e9, 1)
        b >= 1_000_000L -> Triple(MeasureUnit.MEGABYTE, 1e6, if (b >= 10_000_000L) 0 else 1)
        else -> Triple(MeasureUnit.KILOBYTE, 1e3, 0)
    }
    val nf = java.text.NumberFormat.getInstance(locale()).apply { maximumFractionDigits = digits }
    return MeasureFormat.getInstance(locale(), MeasureFormat.FormatWidth.SHORT, android.icu.text.NumberFormat.getInstance(locale()).apply { maximumFractionDigits = digits })
        .format(Measure(maxOf(b, 1000L) / div, unit)).ifEmpty { nf.format(maxOf(b, 1000L) / div) }
}

/** A rough length of time in words: "about 40 seconds", "about 12 minutes", "about 3 hours". */
fun duration(s: Int): String = when {
    s < 90 -> t("cull.etaSeconds", "n" to maxOf(5, Math.round(s / 5f) * 5))
    s < 5400 -> t("cull.etaMinutes", "n" to Math.round(s / 60f))
    else -> t("cull.etaHours", "n" to Math.round(s / 360f) / 10f)
}

/** 日期加时间（备份的"上次备份"）。 */
fun fmtWhen(secs: Long): String = DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT, locale()).format(Date(secs * 1000))

/** 只有日期（风格的"创建于"）。 */
fun fmtDate(ms: Long): String = DateFormat.getDateInstance(DateFormat.MEDIUM, locale()).format(Date(ms))

/** "甲、乙和丙" / "A, B, and C"。 */
fun fmtList(items: List<String>): String = ListFormatter.getInstance(locale()).format(items)
