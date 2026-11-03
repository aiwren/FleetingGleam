package com.fleetinggleam.android.ui.settings

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import com.fleetinggleam.android.core.ReleaseNotes
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.HttpURLConnection
import java.net.URL

/**
 * 图文版更新说明（底部弹层里）：小标题、配图、段落和列表。和桌面端 Settings.tsx 的 NotesView 同样的排法。
 * 配图直接用 HttpURLConnection 取（应用里没有网络图片库，为几张图不值得多带一个 OkHttp），解码后缓存在内存里；取不到就不占位。
 */
@Composable
fun ReleaseNotesSheet(title: String, blocks: List<ReleaseNotes.Block>, footer: @Composable () -> Unit) {
    val c = LocalFg.current
    Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(horizontal = 20.dp).padding(bottom = 20.dp)) {
        Text(title, style = Type.title.copy(color = c.ink), modifier = Modifier.padding(top = 4.dp, bottom = 8.dp))
        blocks.forEach { b ->
            when (b) {
                is ReleaseNotes.Block.Heading -> Text(bold(b.text), style = Type.navTitle.copy(color = c.ink), modifier = Modifier.padding(top = 18.dp, bottom = 8.dp))
                is ReleaseNotes.Block.Image -> NoteImage(b.src, b.alt)
                is ReleaseNotes.Block.Para -> Text(bold(b.text), style = Type.body.copy(color = c.ink), modifier = Modifier.padding(bottom = 8.dp))
                is ReleaseNotes.Block.Bullets -> Column(Modifier.padding(bottom = 8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    b.items.forEach { item ->
                        Row {
                            Text("•", style = Type.body.copy(color = c.ash), modifier = Modifier.width(16.dp))
                            Text(bold(item), style = Type.body.copy(color = c.ink))
                        }
                    }
                }
            }
        }
        Box(Modifier.fillMaxWidth().padding(top = 12.dp)) { footer() }
    }
}

/** 行内只有 **加粗**：按 ** 切开，奇数段加粗。 */
private fun bold(s: String): AnnotatedString = buildAnnotatedString {
    s.split("**").forEachIndexed { i, part -> if (i % 2 == 1) withStyle(SpanStyle(fontWeight = FontWeight.SemiBold)) { append(part) } else append(part) }
}

private val imageCache = HashMap<String, ImageBitmap>()

@Composable
private fun NoteImage(src: String, alt: String) {
    val c = LocalFg.current
    val img by produceState(imageCache[src], src) {
        if (value != null) return@produceState
        value = withContext(Dispatchers.IO) {
            runCatching {
                val conn = (URL(src).openConnection() as HttpURLConnection).apply { connectTimeout = 15_000; readTimeout = 30_000; instanceFollowRedirects = true }
                try {
                    if (conn.responseCode !in 200..299) return@runCatching null
                    // 说明里的截图最大 1200 宽：直接解码不缩放；按 2 倍降采样留给极端大的图
                    val bytes = conn.inputStream.use { it.readBytes() }
                    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }.also { BitmapFactory.decodeByteArray(bytes, 0, bytes.size, it) }
                    val opts = BitmapFactory.Options().apply { inSampleSize = if (bounds.outWidth > 2400) 2 else 1 }
                    BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)?.asImageBitmap()
                } finally { conn.disconnect() }
            }.getOrNull()
        }?.also { imageCache[src] = it }
    }
    val bmp = img ?: return
    // 横图铺满宽度；竖的手机截图别铺满整屏：最高 420dp，按比例算宽度、居中
    androidx.compose.foundation.layout.BoxWithConstraints(Modifier.fillMaxWidth().padding(top = 4.dp, bottom = 10.dp), contentAlignment = Alignment.Center) {
        val ratio = bmp.width.toFloat() / bmp.height.coerceAtLeast(1)
        val w = minOf(maxWidth, 420.dp * ratio)
        Image(bmp, alt, contentScale = ContentScale.Fit,
            modifier = Modifier.width(w).aspectRatio(ratio).clip(RoundedCornerShape(10.dp)).border(0.5.dp, c.line, RoundedCornerShape(10.dp)).background(c.surface))
    }
}
