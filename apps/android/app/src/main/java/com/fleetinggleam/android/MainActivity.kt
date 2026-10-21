package com.fleetinggleam.android

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Color as AColor
import android.graphics.Paint
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
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
import dev.chrisbanes.haze.HazeState
import dev.chrisbanes.haze.hazeEffect
import dev.chrisbanes.haze.hazeSource
import dev.chrisbanes.haze.materials.ExperimentalHazeMaterialsApi
import dev.chrisbanes.haze.materials.HazeMaterials
import dev.chrisbanes.haze.rememberHazeState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.fleetinggleam.DownloadProgress
import uniffi.fleetinggleam.FgException
import uniffi.fleetinggleam.PackStatus
import uniffi.fleetinggleam.PhotoVerdict
import uniffi.fleetinggleam.ScanInfo
import uniffi.fleetinggleam.ScanProgress
import uniffi.fleetinggleam.analyzeFolder
import uniffi.fleetinggleam.coreVersion
import uniffi.fleetinggleam.init
import uniffi.fleetinggleam.machineInfo
import uniffi.fleetinggleam.nasProbe
import uniffi.fleetinggleam.packCancel
import uniffi.fleetinggleam.packDownload
import uniffi.fleetinggleam.packRemove
import uniffi.fleetinggleam.packsStatus
import java.io.File
import kotlin.random.Random

/**
 * 阶段 B 第 1、2 步的冒烟界面：证明 Kotlin 能加载 Rust 核心库并调用它——分析应用内生成的示例照片、下载与管理模型包、列出一台 NAS 的共享。
 * 导航是底部悬浮的玻璃标签栏（作者要求：像 iOS 那种悬浮的 tab bar）；正式的选片界面在第 4 步按 DESIGN.md 重写，这里只借用它的颜色
 * （墨色、琥珀只给"保留"）。
 */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // 系统的全局 HTTP 代理（设置里配的，或模拟器的 -http-proxy）：Java 的 HttpsURLConnection 自己会走，Rust 的客户端要告诉它
        val proxyHost = System.getProperty("https.proxyHost")?.takeIf { it.isNotBlank() } ?: System.getProperty("http.proxyHost")?.takeIf { it.isNotBlank() }
        val proxyPort = System.getProperty("https.proxyPort") ?: System.getProperty("http.proxyPort")
        val proxy = proxyHost?.let { "http://$it:${proxyPort ?: "80"}" }
        init(File(cacheDir, "scratch").absolutePath, proxy)
        setContent { GleamTheme { App(filesDir, cacheDir) } }
    }
}

// DESIGN.md 的色板：Ink / Ash / Canvas / Panel，琥珀只表示"保留"
private val Amber = Color(0xFFDEA13F)
private val AmberDark = Color(0xFFE8B45C)
private val LightScheme = lightColorScheme(primary = Color(0xFF111113), onPrimary = Color(0xFFFFFFFF), background = Color(0xFFFFFFFF), surface = Color(0xFFF7F7F8), surfaceVariant = Color(0xFFEFEFF1), onSurface = Color(0xFF111113), onSurfaceVariant = Color(0xFF6C6C74), secondary = Color(0xFF6C6C74), outline = Color(0x17000000))
private val DarkScheme = darkColorScheme(primary = Color(0xFFEDEDEF), onPrimary = Color(0xFF0E0E10), background = Color(0xFF0E0E10), surface = Color(0xFF151517), surfaceVariant = Color(0xFF1D1D20), onSurface = Color(0xFFEDEDEF), onSurfaceVariant = Color(0xFF8F8F98), secondary = Color(0xFF8F8F98), outline = Color(0x17FFFFFF))

@Composable
fun GleamTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = if (isSystemInDarkTheme()) DarkScheme else LightScheme, content = content)
}

/** 页签：照片、设置（模型、NAS 都在设置里；作者 2026-10-21：底部菜单该有"设置"，NAS 不单独占一格）。 */
private enum class Tab(val label: Int) { Photos(R.string.tab_photos), Settings(R.string.tab_settings) }

/** 悬浮标签栏自己的高度加上它离底边的距离：内容区留出这么多，最后一行才不会被挡住。 */
private val TabBarInset = 96.dp

@OptIn(ExperimentalHazeMaterialsApi::class)
@Composable
fun App(filesDir: File, cacheDir: File) {
    var tab by remember { mutableStateOf(Tab.Photos) }
    val haze = rememberHazeState()
    val modelsDir = remember { File(filesDir, "models") }
    Box(modifier = Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
        // 内容是玻璃后面的东西：标签栏对它做实时模糊
        Box(modifier = Modifier.fillMaxSize().hazeSource(haze)) {
            when (tab) {
                Tab.Photos -> PhotosScreen(filesDir, cacheDir, modelsDir)
                Tab.Settings -> SettingsScreen(modelsDir)
            }
        }
        FloatingTabBar(haze, tab, onSelect = { tab = it }, modifier = Modifier.align(Alignment.BottomCenter))
    }
}

/**
 * 底部悬浮的胶囊标签栏，液态玻璃的第一版：Haze 对后面的内容实时模糊并轻微着色（Android 12+；更早的系统退为半透明），
 * 玻璃边缘一圈随光线变化的高光描边（上亮下暗，像有厚度），选中的页签是一粒墨色胶囊。折射（AGSL 着色器，Android 13+）在第 4 步加。
 */
@OptIn(ExperimentalHazeMaterialsApi::class)
@Composable
private fun FloatingTabBar(haze: HazeState, current: Tab, onSelect: (Tab) -> Unit, modifier: Modifier = Modifier) {
    val scheme = MaterialTheme.colorScheme
    val dark = isSystemInDarkTheme()
    val shape = RoundedCornerShape(32.dp)
    // 高光：顶边偏白、底边偏暗，玻璃的厚度感来自这一圈
    val rim = Brush.verticalGradient(listOf(Color.White.copy(alpha = if (dark) 0.35f else 0.9f), Color.White.copy(alpha = 0.08f), Color.Black.copy(alpha = if (dark) 0.35f else 0.08f)))
    Box(modifier = modifier.navigationBarsPadding().padding(bottom = 12.dp)) {
        Row(
            modifier = Modifier
                .shadow(elevation = 18.dp, shape = shape, ambientColor = Color.Black.copy(alpha = 0.25f), spotColor = Color.Black.copy(alpha = 0.3f))
                .clip(shape)
                .hazeEffect(state = haze, style = HazeMaterials.ultraThin(scheme.surface)) {
                    blurRadius = 28.dp
                    noiseFactor = 0.02f
                }
                .border(1.dp, rim, shape)
                .padding(6.dp),
            horizontalArrangement = Arrangement.spacedBy(4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Tab.entries.forEach { t ->
                val selected = t == current
                Surface(
                    onClick = { onSelect(t) },
                    shape = CircleShape,
                    color = if (selected) scheme.primary else Color.Transparent,
                    contentColor = if (selected) scheme.onPrimary else scheme.onSurface,
                ) {
                    Text(stringResource(t.label), style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(horizontal = 18.dp, vertical = 10.dp))
                }
            }
        }
    }
}

@Composable
private fun PrimaryButton(text: String, enabled: Boolean, onClick: () -> Unit) {
    Button(enabled = enabled, onClick = onClick, colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary, contentColor = MaterialTheme.colorScheme.onPrimary)) { Text(text) }
}

@Composable
private fun ScreenTitle(title: String, subtitle: String? = null) {
    Text(title, style = MaterialTheme.typography.headlineMedium)
    subtitle?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant) }
}

// ---------------------------------------------------------------- 照片

@Composable
fun PhotosScreen(filesDir: File, cacheDir: File, modelsDir: File) {
    val scope = rememberCoroutineScope()
    val machine = remember { machineInfo() }
    var busy by remember { mutableStateOf(false) }
    var progress by remember { mutableStateOf<Pair<Int, Int>?>(null) }
    var result by remember { mutableStateOf<ScanInfo?>(null) }
    var error by remember { mutableStateOf<String?>(null) }

    LazyColumn(
        modifier = Modifier.fillMaxSize().statusBarsPadding(),
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 16.dp, bottom = TabBarInset),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item {
            ScreenTitle(stringResource(R.string.app_name), stringResource(R.string.core_version, coreVersion(), machine.memoryGb.toLong(), machine.cores.toLong(), machine.accelerator))
            Spacer(Modifier.height(8.dp))
            Text(stringResource(R.string.section_analyze), style = MaterialTheme.typography.titleMedium)
            Text(stringResource(R.string.analyze_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        item {
            PrimaryButton(stringResource(R.string.analyze_button), enabled = !busy) {
                busy = true; error = null; result = null; progress = null
                scope.launch {
                    try {
                        val out = withContext(Dispatchers.IO) {
                            val photos = sampleFolder(filesDir)
                            analyzeFolder(photos.absolutePath, File(cacheDir, "thumbs").absolutePath, modelsDir.absolutePath, "fast", 0.5f, object : ScanProgress {
                                override fun onPhoto(done: UInt, total: UInt) { progress = done.toInt() to total.toInt() }
                            })
                        }
                        result = out
                    } catch (e: FgException) {
                        error = e.message
                    } finally {
                        busy = false
                    }
                }
            }
        }
        progress?.let { (done, total) ->
            if (busy) item {
                Text(stringResource(R.string.analyzing, done, total), style = MaterialTheme.typography.bodySmall)
                LinearProgressIndicator(progress = { if (total == 0) 0f else done.toFloat() / total }, modifier = Modifier.fillMaxWidth(), color = MaterialTheme.colorScheme.primary)
            }
        }
        error?.let { item { Text(stringResource(R.string.error_prefix, it)) } }
        result?.let { r ->
            item {
                val clip = r.clipModel?.let { "$it · ${r.clipBackend}" } ?: stringResource(R.string.scan_no_clip)
                Text(stringResource(R.string.scan_info, clip, if (r.faces) "✓" else "–", if (r.people) "✓" else "–"), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            items(r.photos, key = { it.path }) { VerdictRow(it) }
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
    val details = buildList {
        add("%.2f".format(v.score))
        add("${v.width}×${v.height}")
        add("#${v.group}")
        v.clipScore?.let { add("AI %.2f".format(it)) }
        if (v.faceCount > 0u) add(stringResource(R.string.faces_n, v.faceCount.toLong()) + (v.eyesOpen?.let { " · " + stringResource(R.string.eyes_v, it) } ?: ""))
        addAll(v.flags)
    }
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
                Text(details.joinToString(" · "), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Text(stateText, style = MaterialTheme.typography.labelLarge, color = if (v.state == "keep") (if (dark) AmberDark else Amber) else MaterialTheme.colorScheme.onSurface)
        }
    }
}

// ---------------------------------------------------------------- 设置（模型、NAS）

@Composable
fun SettingsScreen(modelsDir: File) {
    var section by remember { mutableStateOf(0) }
    Column(modifier = Modifier.fillMaxSize().statusBarsPadding()) {
        Column(modifier = Modifier.padding(horizontal = 20.dp).padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            ScreenTitle(stringResource(R.string.tab_settings))
            // 分组：模型 / NAS（备份、外观、语言等在第 3 步加）
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf(R.string.tab_models, R.string.section_nas).forEachIndexed { i, label ->
                    val selected = i == section
                    Surface(onClick = { section = i }, shape = CircleShape, color = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceVariant, contentColor = if (selected) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurface) {
                        Text(stringResource(label), style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(horizontal = 14.dp, vertical = 8.dp))
                    }
                }
            }
        }
        when (section) {
            0 -> ModelsScreen(modelsDir)
            else -> NasScreen()
        }
    }
}

// ---------------------------------------------------------------- 模型

private val packNames = mapOf(
    "standard" to R.string.pack_standard, "high" to R.string.pack_high, "ultra" to R.string.pack_ultra, "search" to R.string.pack_search,
    "eyes" to R.string.pack_eyes, "faces" to R.string.pack_faces, "detect" to R.string.pack_detect, "scenetext" to R.string.pack_scenetext,
)

private fun mb(bytes: Long): String = if (bytes >= 1_000_000_000L) "%.1f GB".format(bytes / 1e9) else "%.0f MB".format(bytes / 1e6)

@Composable
fun ModelsScreen(modelsDir: File) {
    val scope = rememberCoroutineScope()
    var packs by remember { mutableStateOf<List<PackStatus>>(emptyList()) }
    var downloading by remember { mutableStateOf<String?>(null) }
    var progress by remember { mutableStateOf<Triple<String, Long, Long>?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var refresh by remember { mutableIntStateOf(0) }
    LaunchedEffect(refresh) { packs = withContext(Dispatchers.IO) { packsStatus(modelsDir.absolutePath) } }

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 12.dp, bottom = TabBarInset),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item { Text(stringResource(R.string.models_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
        error?.let { item { Text(stringResource(R.string.error_prefix, it)) } }
        items(packs, key = { it.id }) { p ->
            val isThis = downloading == p.id
            Surface(color = MaterialTheme.colorScheme.surface, shape = RoundedCornerShape(10.dp), modifier = Modifier.fillMaxWidth()) {
                Column(modifier = Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(stringResource(packNames[p.id] ?: R.string.pack_unknown, p.id), style = MaterialTheme.typography.bodyMedium)
                            Text(if (p.installed) stringResource(R.string.pack_installed) else stringResource(R.string.pack_size, mb(p.bytes.toLong())), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        if (p.installed) {
                            OutlinedButton(enabled = downloading == null, onClick = {
                                scope.launch { withContext(Dispatchers.IO) { runCatching { packRemove(modelsDir.absolutePath, p.id) } }; refresh++ }
                            }) { Text(stringResource(R.string.pack_remove)) }
                        } else if (isThis) {
                            OutlinedButton(onClick = { packCancel() }) { Text(stringResource(R.string.pack_cancel)) }
                        } else {
                            PrimaryButton(stringResource(R.string.pack_download), enabled = downloading == null) {
                                downloading = p.id; error = null; progress = null
                                scope.launch {
                                    try {
                                        withContext(Dispatchers.IO) {
                                            packDownload(modelsDir.absolutePath, p.id, object : DownloadProgress {
                                                override fun onProgress(pack: String, file: String, done: ULong, total: ULong) { progress = Triple(file, done.toLong(), total.toLong()) }
                                            })
                                        }
                                    } catch (e: FgException) {
                                        error = e.message
                                    } finally {
                                        downloading = null; progress = null; refresh++
                                    }
                                }
                            }
                        }
                    }
                    if (isThis) progress?.let { (file, done, total) ->
                        Text("$file · ${mb(done)} / ${mb(total)}", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        LinearProgressIndicator(progress = { if (total == 0L) 0f else done.toFloat() / total }, modifier = Modifier.fillMaxWidth(), color = MaterialTheme.colorScheme.primary)
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------- NAS

@Composable
fun NasScreen() {
    val scope = rememberCoroutineScope()
    var busy by remember { mutableStateOf(false) }
    var host by remember { mutableStateOf("") }
    var user by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var shares by remember { mutableStateOf<List<String>?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 12.dp, bottom = TabBarInset),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item { Text(stringResource(R.string.nas_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
        item { OutlinedTextField(value = host, onValueChange = { host = it }, label = { Text(stringResource(R.string.nas_host)) }, singleLine = true, modifier = Modifier.fillMaxWidth()) }
        item { OutlinedTextField(value = user, onValueChange = { user = it }, label = { Text(stringResource(R.string.nas_user)) }, singleLine = true, modifier = Modifier.fillMaxWidth()) }
        item { OutlinedTextField(value = password, onValueChange = { password = it }, label = { Text(stringResource(R.string.nas_password)) }, singleLine = true, visualTransformation = PasswordVisualTransformation(), modifier = Modifier.fillMaxWidth()) }
        item {
            PrimaryButton(stringResource(R.string.nas_button), enabled = !busy && host.isNotBlank()) {
                busy = true; error = null; shares = null
                scope.launch {
                    try {
                        shares = withContext(Dispatchers.IO) { nasProbe(host, user, password) }
                    } catch (e: FgException) {
                        error = e.message
                    } finally {
                        busy = false
                    }
                }
            }
        }
        shares?.let { item { Text(stringResource(R.string.nas_shares, it.joinToString(", "))) } }
        error?.let { item { Text(stringResource(R.string.error_prefix, it)) } }
    }
}

// ---------------------------------------------------------------- 示例照片

/**
 * 应用私有目录里的一组示例照片（第一次用时生成）：三张相近的"连拍"、一张糊的、一张很暗的，足够看到分组与取舍在动。
 * 不需要相册权限；真正读相册与 SAF 文件夹在阶段 B 第 3 步。
 */
private fun sampleFolder(filesDir: File): File {
    val dir = File(filesDir, "sample-photos")
    if (dir.isDirectory && (dir.list()?.size ?: 0) >= 5) return dir
    dir.mkdirs()
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
        p.color = AColor.rgb(30, 30, 36)
        c.drawRect(0f, h * 0.72f, w.toFloat(), h.toFloat(), p)
        if (!blur) return bmp
        // 糊的那张：缩小再放大
        val small = Bitmap.createScaledBitmap(bmp, w / 12, h / 12, true)
        return Bitmap.createScaledBitmap(small, w, h, true)
    }
    val shots = listOf(
        "IMG_0001.jpg" to scene(1, blur = false, dark = false),
        "IMG_0002.jpg" to scene(1, blur = false, dark = false),
        "IMG_0003.jpg" to scene(1, blur = true, dark = false),
        "IMG_0004.jpg" to scene(2, blur = false, dark = false),
        "IMG_0005.jpg" to scene(3, blur = false, dark = true),
    )
    for ((name, bmp) in shots) {
        File(dir, name).outputStream().use { bmp.compress(Bitmap.CompressFormat.JPEG, 92, it) }
    }
    return dir
}
