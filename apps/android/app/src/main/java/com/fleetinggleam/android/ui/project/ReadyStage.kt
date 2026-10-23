package com.fleetinggleam.android.ui.project

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import com.fleetinggleam.android.core.ProbeFile
import com.fleetinggleam.android.core.Project
import com.fleetinggleam.android.core.Projects
import com.fleetinggleam.android.core.baseName
import com.fleetinggleam.android.core.duration
import com.fleetinggleam.android.core.fmtBytes
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgButton
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.components.Icons
import com.fleetinggleam.android.ui.components.SettingGroup
import com.fleetinggleam.android.ui.components.SettingRow
import com.fleetinggleam.android.ui.components.Ticker
import com.fleetinggleam.android.ui.components.riseIn
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.delay
import java.io.File

/**
 * 分析前的那一屏（桌面端 ReadyStage.tsx 的手机版；作者 2026-10-22 嫌旧版的灯箱和大段说明丑）：
 * 和首页大卡同一套语言——封面拼图上压着张数、大小和预计时间；下面是 iOS 分组列表：名称、文件夹、偏好、备注；底部只留"开始分析"和一句话。
 */
@Composable
fun ReadyStage(project: Project, count: Int, bytes: Long, etaSeconds: Int?, files: List<ProbeFile>, thumbs: Map<String, String>, onStart: () -> Unit, onAddFolder: () -> Unit, onRemoveFolder: (String) -> Unit, done: Int = 0) {
    val c = LocalFg.current
    var name by remember(project.id) { mutableStateOf(project.name) }
    var note by remember(project.id) { mutableStateOf(project.note ?: "") }
    // 打字先进本地状态，停下来再写：每个字都重写项目文件太浪费
    LaunchedEffect(note) { delay(500); if ((project.note ?: "") != note) Projects.update(project.id) { it.copy(note = note) } }
    LaunchedEffect(name) { delay(500); name.trim().takeIf { it.isNotEmpty() && it != project.name }?.let { v -> Projects.update(project.id) { it.copy(name = v) } } }
    var n by remember { mutableIntStateOf(0) }
    LaunchedEffect(count) { delay(350); n = count }
    Column(Modifier.fillMaxSize()) {
        // 封面卡单独入场；下面的分组一次淡入，不再逐个升起（设计审查 A4.6）；分组之间 24dp（A4.7）
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(top = 12.dp, bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(24.dp)) {
            CoverCollage(files.take(3), thumbs, n, listOfNotNull(fmtBytes(bytes), etaSeconds?.let { t("cull.readyEta", "time" to duration(it)) }).joinToString(" · "))
            Column(Modifier.riseIn(1, rise = 0f, scaleFrom = 1f), verticalArrangement = Arrangement.spacedBy(24.dp)) {
                // 上次分析到一半（离开了、停下了）：分析过的都留着，这次只分析剩下的
                if (done > 0) Text(t("android.ready.resume", "done" to done), style = Type.rowDesc.copy(color = c.ash), modifier = Modifier.padding(horizontal = 4.dp))
                SettingGroup("") { NameRow(name) { name = it } }
                SettingGroup(t("pset.folders")) {
                    project.folders.forEachIndexed { i, f ->
                        SettingRow(baseName(f), shortPath(f), first = i == 0, icon = Icons.FOLDER, control = if (project.folders.size > 1) {
                            { Box(Modifier.size(32.dp).clip(CircleShape).clickable { onRemoveFolder(f) }, contentAlignment = Alignment.Center) { FgIcon(Icons.CLOSE, size = 14.dp, color = c.ash) } }
                        } else null)
                    }
                    SettingRow(t("cull.addFolder"), icon = Icons.PLUS, iconBox = 18f, onClick = onAddFolder, control = { FgIcon(Icons.CHEVRON, size = 14.dp, stroke = 1.6f, color = c.ash) })
                }
                SettingGroup(t("pset.prefs")) { ProjectPrefs(project) }
                SettingGroup(t("pset.note")) {
                    Box(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 14.dp)) {
                        if (note.isEmpty()) Text(t("pset.notePh"), style = Type.body.copy(color = c.ash))
                        BasicTextField(note, { note = it }, textStyle = Type.body.copy(color = c.ink), cursorBrush = SolidColor(c.ink), minLines = 2, modifier = Modifier.fillMaxWidth())
                    }
                }
            }
        }
        // 底部只有按钮：解释句删掉了，预计时间在封面卡上（设计审查 A4.5）
        Box(Modifier.fillMaxWidth().background(c.panel).padding(horizontal = 16.dp, vertical = 12.dp).navigationBarsPadding()) {
            FgButton(t("cull.start"), onStart, primary = true, modifier = Modifier.fillMaxWidth())
        }
    }
}

/** 手机存储的前缀不用看。 */
private fun shortPath(p: String): String = p.substringAfter("/files/", p).removePrefix("/storage/emulated/0/").removePrefix("/storage/emulated/0").ifEmpty { "/" }

/** 名称一行：左边"项目名称"，右边直接改。 */
@Composable
private fun NameRow(name: String, onChange: (String) -> Unit) {
    val c = LocalFg.current
    val focus = LocalFocusManager.current
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 14.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(t("pset.name"), style = Type.rowTitle.copy(color = c.ink))
        // 右对齐的墨色值就表示可改（iOS 的做法），不放铅笔图标（设计审查 A4.2）；空着时灰色占位
        Box(Modifier.weight(1f).padding(start = 16.dp)) {
            if (name.isEmpty()) Text(t("home.namePlaceholder"), style = Type.rowTitle.copy(color = c.ash, textAlign = TextAlign.End), modifier = Modifier.fillMaxWidth())
            BasicTextField(
                name, onChange, singleLine = true, textStyle = Type.rowTitle.copy(color = c.ink, textAlign = TextAlign.End), cursorBrush = SolidColor(c.ink),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done), keyboardActions = KeyboardActions(onDone = { focus.clearFocus() }),
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

/** 封面拼图（和首页大卡一样：左一大右两小），底部渐变上压着张数、大小与预计时间。 */
@Composable
private fun CoverCollage(files: List<ProbeFile>, thumbs: Map<String, String>, count: Int, meta: String) {
    val c = LocalFg.current
    Box(Modifier.riseIn(0, scaleFrom = 0.98f).fillMaxWidth().aspectRatio(16f / 10f).clip(RoundedCornerShape(16.dp)).background(c.surface)) {
        val src = { f: ProbeFile -> File(thumbs[f.path] ?: f.path) }
        if (files.isNotEmpty()) Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
            AsyncImage(src(files[0]), null, Modifier.weight(1.6f).fillMaxSize(), contentScale = ContentScale.Crop)
            if (files.size > 1) Column(Modifier.weight(1f).fillMaxSize(), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                files.drop(1).take(2).forEach { AsyncImage(src(it), null, Modifier.weight(1f).fillMaxWidth(), contentScale = ContentScale.Crop) }
            }
        }
        Box(Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(96.dp).background(Brush.verticalGradient(listOf(Color.Transparent, Color(0xA6000000)))))
        Column(Modifier.align(Alignment.BottomStart).padding(16.dp)) {
            Row(verticalAlignment = Alignment.Bottom) {
                Ticker(count, Type.bigNumber.copy(color = Color.White))
                Text(t("cull.readyUnit"), style = Type.body.copy(color = Color.White.copy(alpha = 0.85f)), modifier = Modifier.padding(start = 8.dp, bottom = 5.dp))
            }
            Text(meta, style = Type.monoSmall.copy(color = Color.White.copy(alpha = 0.75f)), maxLines = 1, overflow = Ellipsis)
        }
    }
}
