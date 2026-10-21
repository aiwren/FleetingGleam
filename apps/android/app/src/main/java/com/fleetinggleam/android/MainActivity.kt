package com.fleetinggleam.android

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Color as AColor
import android.graphics.Paint
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.Image
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.fleetinggleam.FgException
import uniffi.fleetinggleam.PhotoVerdict
import uniffi.fleetinggleam.ScanProgress
import uniffi.fleetinggleam.analyzeFolder
import uniffi.fleetinggleam.coreVersion
import uniffi.fleetinggleam.machineInfo
import uniffi.fleetinggleam.nasProbe
import java.io.File
import kotlin.random.Random

/**
 * 阶段 B 第 1 步的冒烟界面：证明 Kotlin 能加载 Rust 核心库并调用它——显示库版本、对应用内生成的示例照片做一次分析、列出一台 NAS 的共享。
 * 正式的选片界面在第 4 步按 DESIGN.md 重写；这里只借用它的颜色（墨色、琥珀只给"保留"）。
 */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent { GleamTheme { SmokeScreen(filesDir, cacheDir) } }
    }
}

// DESIGN.md 的色板：Ink / Ash / Canvas / Panel，琥珀只表示"保留"
private val Amber = Color(0xFFDEA13F)
private val AmberDark = Color(0xFFE8B45C)
private val LightScheme = lightColorScheme(primary = Color(0xFF111113), onPrimary = Color(0xFFFFFFFF), background = Color(0xFFFFFFFF), surface = Color(0xFFF7F7F8), surfaceVariant = Color(0xFFEFEFF1), onSurface = Color(0xFF111113), onSurfaceVariant = Color(0xFF6C6C74), secondary = Color(0xFF6C6C74))
private val DarkScheme = darkColorScheme(primary = Color(0xFFEDEDEF), onPrimary = Color(0xFF0E0E10), background = Color(0xFF0E0E10), surface = Color(0xFF151517), surfaceVariant = Color(0xFF1D1D20), onSurface = Color(0xFFEDEDEF), onSurfaceVariant = Color(0xFF8F8F98), secondary = Color(0xFF8F8F98))

@Composable
fun GleamTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = if (isSystemInDarkTheme()) DarkScheme else LightScheme, content = content)
}

@Composable
fun SmokeScreen(filesDir: File, cacheDir: File) {
    val scope = rememberCoroutineScope()
    val machine = remember { machineInfo() }
    var busy by remember { mutableStateOf(false) }
    var progress by remember { mutableStateOf<Pair<Int, Int>?>(null) }
    var verdicts by remember { mutableStateOf<List<PhotoVerdict>>(emptyList()) }
    var error by remember { mutableStateOf<String?>(null) }
    var host by remember { mutableStateOf("") }
    var user by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var shares by remember { mutableStateOf<List<String>?>(null) }
    var nasError by remember { mutableStateOf<String?>(null) }

    Scaffold(containerColor = MaterialTheme.colorScheme.background) { inner ->
        Column(modifier = Modifier.padding(inner).fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp, vertical = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(stringResource(R.string.app_name), style = MaterialTheme.typography.headlineMedium)
            Text(stringResource(R.string.core_version, coreVersion(), machine.memoryGb.toLong(), machine.cores.toLong()), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)

            Spacer(Modifier.height(8.dp))
            Text(stringResource(R.string.section_analyze), style = MaterialTheme.typography.titleMedium)
            Text(stringResource(R.string.analyze_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Button(
                enabled = !busy,
                colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary, contentColor = MaterialTheme.colorScheme.onPrimary),
                onClick = {
                    busy = true; error = null; verdicts = emptyList(); progress = null
                    scope.launch {
                        try {
                            val out = withContext(Dispatchers.IO) {
                                val photos = sampleFolder(filesDir)
                                analyzeFolder(photos.absolutePath, File(cacheDir, "thumbs").absolutePath, File(filesDir, "models").absolutePath, 0.5f, object : ScanProgress {
                                    override fun onPhoto(done: UInt, total: UInt) { progress = done.toInt() to total.toInt() }
                                })
                            }
                            verdicts = out
                        } catch (e: FgException) {
                            error = e.message
                        } finally {
                            busy = false
                        }
                    }
                },
            ) { Text(stringResource(R.string.analyze_button)) }
            progress?.let { (done, total) ->
                if (busy) {
                    Text(stringResource(R.string.analyzing, done, total), style = MaterialTheme.typography.bodySmall)
                    LinearProgressIndicator(progress = { if (total == 0) 0f else done.toFloat() / total }, modifier = Modifier.fillMaxWidth(), color = MaterialTheme.colorScheme.primary)
                }
            }
            error?.let { Text(stringResource(R.string.error_prefix, it), color = MaterialTheme.colorScheme.onSurface) }
            verdicts.forEach { VerdictRow(it) }

            Spacer(Modifier.height(16.dp))
            Text(stringResource(R.string.section_nas), style = MaterialTheme.typography.titleMedium)
            Text(stringResource(R.string.nas_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            OutlinedTextField(value = host, onValueChange = { host = it }, label = { Text(stringResource(R.string.nas_host)) }, singleLine = true, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(value = user, onValueChange = { user = it }, label = { Text(stringResource(R.string.nas_user)) }, singleLine = true, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(value = password, onValueChange = { password = it }, label = { Text(stringResource(R.string.nas_password)) }, singleLine = true, visualTransformation = PasswordVisualTransformation(), modifier = Modifier.fillMaxWidth())
            Button(
                enabled = !busy && host.isNotBlank(),
                colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary, contentColor = MaterialTheme.colorScheme.onPrimary),
                onClick = {
                    busy = true; nasError = null; shares = null
                    scope.launch {
                        try {
                            shares = withContext(Dispatchers.IO) { nasProbe(host, user, password) }
                        } catch (e: FgException) {
                            nasError = e.message
                        } finally {
                            busy = false
                        }
                    }
                },
            ) { Text(stringResource(R.string.nas_button)) }
            shares?.let { Text(stringResource(R.string.nas_shares, it.joinToString(", "))) }
            nasError?.let { Text(stringResource(R.string.error_prefix, it)) }
        }
    }
}

/** 一行结果：缩略图、文件名、分数、组号；"保留"用琥珀，其余用墨色文字（不用红色，不把淘汰的变黑白）。 */
@Composable
private fun VerdictRow(v: PhotoVerdict) {
    val dark = isSystemInDarkTheme()
    val stateText = when (v.state) {
        "keep" -> stringResource(R.string.state_keep)
        "review" -> stringResource(R.string.state_review)
        else -> stringResource(R.string.state_reject)
    }
    val bitmap = remember(v.thumb) { BitmapFactory.decodeFile(v.thumb) }
    Surface(color = MaterialTheme.colorScheme.surface, shape = RoundedCornerShape(10.dp), modifier = Modifier.fillMaxWidth()) {
        Row(modifier = Modifier.padding(10.dp), verticalAlignment = Alignment.CenterVertically) {
            if (bitmap != null) {
                Image(bitmap.asImageBitmap(), contentDescription = null, modifier = Modifier.size(56.dp).clip(RoundedCornerShape(6.dp)))
            } else {
                Spacer(Modifier.size(56.dp))
            }
            Spacer(Modifier.width(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(v.name, style = MaterialTheme.typography.bodyMedium)
                Text("%.2f · %d×%d · #%d%s".format(v.score, v.width.toLong(), v.height.toLong(), v.group.toLong(), if (v.flags.isEmpty()) "" else " · " + v.flags.joinToString(" ")), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Text(stateText, style = MaterialTheme.typography.labelLarge, color = if (v.state == "keep") (if (dark) AmberDark else Amber) else MaterialTheme.colorScheme.onSurface)
        }
    }
}

/**
 * 应用私有目录里的一组示例照片（第一次用时生成）：三张相近的"连拍"、一张糊的、一张很暗的，足够看到分组与取舍在动。
 * 不需要相册权限；真正读相册与 SAF 文件夹在阶段 B 第 3 步。
 */
private fun sampleFolder(filesDir: File): File {
    val dir = File(filesDir, "sample-photos")
    if (dir.isDirectory && (dir.list()?.size ?: 0) >= 5) return dir
    dir.mkdirs()
    val rnd = Random(7)
    fun scene(seed: Int, blur: Boolean, dark: Boolean): Bitmap {
        val w = 960; val h = 720
        val bmp = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
        val c = Canvas(bmp)
        val p = Paint(Paint.ANTI_ALIAS_FLAG)
        c.drawColor(if (dark) AColor.rgb(8, 8, 12) else AColor.rgb(232, 226, 214))
        val r = Random(seed)
        repeat(40) {
            p.color = AColor.rgb(r.nextInt(40, 220), r.nextInt(40, 220), r.nextInt(40, 220))
            if (dark) p.alpha = 40
            val x = r.nextFloat() * w; val y = r.nextFloat() * h
            c.drawCircle(x, y, 20f + r.nextFloat() * 90f, p)
        }
        p.color = if (dark) AColor.rgb(30, 30, 36) else AColor.rgb(30, 30, 36)
        c.drawRect(0f, h * 0.72f, w.toFloat(), h.toFloat(), p)
        if (!blur) return bmp
        // 糊的那张：缩小再放大
        val small = Bitmap.createScaledBitmap(bmp, w / 12, h / 12, true)
        return Bitmap.createScaledBitmap(small, w, h, true)
    }
    val shots = listOf(
        "IMG_0001.jpg" to scene(1, blur = false, dark = false),
        "IMG_0002.jpg" to scene(1 + rnd.nextInt(0, 1), blur = false, dark = false),
        "IMG_0003.jpg" to scene(1, blur = true, dark = false),
        "IMG_0004.jpg" to scene(2, blur = false, dark = false),
        "IMG_0005.jpg" to scene(3, blur = false, dark = true),
    )
    for ((name, bmp) in shots) {
        File(dir, name).outputStream().use { bmp.compress(Bitmap.CompressFormat.JPEG, 92, it) }
    }
    return dir
}
