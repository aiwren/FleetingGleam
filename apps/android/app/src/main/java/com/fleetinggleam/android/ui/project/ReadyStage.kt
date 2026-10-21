package com.fleetinggleam.android.ui.project

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import com.fleetinggleam.android.core.Project
import com.fleetinggleam.android.core.ProbeFile
import com.fleetinggleam.android.core.Projects
import com.fleetinggleam.android.core.baseName
import com.fleetinggleam.android.core.duration
import com.fleetinggleam.android.core.fmtBytes
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgButton
import com.fleetinggleam.android.ui.components.FgField
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.components.Icons
import com.fleetinggleam.android.ui.components.Ticker
import com.fleetinggleam.android.ui.components.riseIn
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.delay
import java.io.File

/** Where each print lands on the table (fractions of its width/height), and how big it is (% of table height) — the Mac's SLOTS. */
private data class Slot(val x: Float, val y: Float, val h: Float)
private val SLOTS = listOf(Slot(0.2f, 0.26f, 38f), Slot(0.52f, 0.18f, 32f), Slot(0.82f, 0.3f, 36f), Slot(0.28f, 0.7f, 34f), Slot(0.58f, 0.6f, 40f), Slot(0.88f, 0.74f, 30f), Slot(0.08f, 0.5f, 26f), Slot(0.44f, 0.92f, 26f))
/** deterministic "tossed" angles, about -8..8 degrees */
private fun tilt(i: Int) = (((i * 37) % 25) - 12) * 0.7f

/**
 * The screen before analysis (the Mac's ReadyStage.tsx): the photos are tossed onto a light table; dragging over the table tilts the prints with
 * parallax and a warm light follows the finger. Name, count, size and time are above; folders, picking preferences and a note below; Start at the bottom.
 */
@Composable
fun ReadyStage(project: Project, count: Int, bytes: Long, etaSeconds: Int?, files: List<ProbeFile>, thumbs: Map<String, String>, onStart: () -> Unit, onAddFolder: () -> Unit, onRemoveFolder: (String) -> Unit) {
    val c = LocalFg.current
    var name by remember(project.id) { mutableStateOf(project.name) }
    var note by remember(project.id) { mutableStateOf(project.note ?: "") }
    // typed into local state; written once the typing pauses (every character would otherwise rewrite the projects file)
    LaunchedEffect(note) { delay(500); if ((project.note ?: "") != note) Projects.update(project.id) { it.copy(note = note) } }
    var n by remember { mutableIntStateOf(0) }
    LaunchedEffect(count) { delay(450); n = count }
    Column(Modifier.fillMaxSize()) {
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 20.dp).padding(top = 8.dp, bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            LightTable(files.take(SLOTS.size), thumbs)
            Column(Modifier.riseIn(1, scaleFrom = 1f)) {
                FgField(name, { name = it }, onDone = { name.trim().takeIf { it.isNotEmpty() && it != project.name }?.let { v -> Projects.update(project.id) { it.copy(name = v) } } })
                Row(Modifier.padding(top = 12.dp), verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Ticker(n)
                    Text(t("cull.readyUnit"), style = Type.body.copy(color = c.ink), modifier = Modifier.padding(bottom = 4.dp))
                }
                Text(listOfNotNull(t("cull.readySize", "size" to fmtBytes(bytes)), etaSeconds?.let { t("cull.readyEta", "time" to duration(it)) }).joinToString(" · "), style = Type.monoSmall.copy(color = c.ash), modifier = Modifier.padding(top = 2.dp))
            }
            Card(2, t("pset.folders")) {
                project.folders.forEach { f ->
                    Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text(baseName(f), style = Type.bodyStrong.copy(color = c.ink), maxLines = 1, overflow = Ellipsis)
                            Text(f, style = Type.monoSmall.copy(color = c.ash), maxLines = 1, overflow = Ellipsis)
                        }
                        if (project.folders.size > 1) Box(Modifier.size(32.dp).clip(RoundedCornerShape(8.dp)).clickable { onRemoveFolder(f) }, contentAlignment = Alignment.Center) { FgIcon(Icons.CLOSE, size = 14.dp, color = c.ash) }
                    }
                }
                FgButton(t("cull.addFolder"), onAddFolder, small = true, modifier = Modifier.padding(top = 6.dp))
            }
            Card(3, t("pset.prefs")) { ProjectPrefs(project) }
            Card(4, t("pset.note")) { FgField(note, { note = it }, placeholder = t("pset.notePh"), singleLine = false, minLines = 3) }
        }
        Column(Modifier.fillMaxWidth().background(c.panel).padding(horizontal = 20.dp, vertical = 14.dp).navigationBarsPadding()) {
            Text(t("cull.readyBody"), style = Type.small.copy(color = c.ash))
            FgButton(t("cull.start"), onStart, primary = true, modifier = Modifier.padding(top = 12.dp).fillMaxWidth())
        }
    }
}

@Composable
private fun Card(i: Int, title: String, content: @Composable () -> Unit) {
    val c = LocalFg.current
    Column(Modifier.riseIn(i, scaleFrom = 1f).fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(c.panel).padding(16.dp)) {
        Text(title.uppercase(), style = Type.overline.copy(color = c.ash), modifier = Modifier.padding(bottom = 10.dp))
        content()
    }
}

/** 灯箱：照片像冲好的相片一样被扔在桌上；手指在桌上拖，相片随深度视差移动，暖光跟着手指走。 */
@Composable
private fun LightTable(files: List<ProbeFile>, thumbs: Map<String, String>) {
    val c = LocalFg.current
    var tx by remember { mutableStateOf(0f) }
    var ty by remember { mutableStateOf(0f) }
    var glow by remember { mutableStateOf(Offset(0.5f, 0.4f)) }
    val sx = remember { Animatable(0f) }
    val sy = remember { Animatable(0f) }
    LaunchedEffect(tx, ty) { sx.animateTo(tx, spring(dampingRatio = 1.05f, stiffness = 90f)) }
    LaunchedEffect(tx, ty) { sy.animateTo(ty, spring(dampingRatio = 1.05f, stiffness = 90f)) }
    BoxWithConstraints(
        Modifier.fillMaxWidth().aspectRatio(1.35f).clip(RoundedCornerShape(18.dp)).background(c.panel)
            .pointerInput(Unit) {
                detectDragGestures(onDragEnd = { tx = 0f; ty = 0f }, onDragCancel = { tx = 0f; ty = 0f }) { ch, _ ->
                    val x = ch.position.x / size.width; val y = ch.position.y / size.height
                    tx = (x - 0.5f) * 2; ty = (y - 0.5f) * 2; glow = Offset(x, y)
                }
            }
            .drawBehind { drawRect(Brush.radialGradient(listOf(c.gleam.copy(alpha = if (c.dark) 0.16f else 0.22f), Color.Transparent), center = Offset(glow.x * size.width, glow.y * size.height), radius = size.maxDimension * 0.55f)) },
    ) {
        val W = maxWidth; val H = maxHeight
        files.forEachIndexed { i, f ->
            val slot = SLOTS[i % SLOTS.size]
            val depth = 0.4f + (i % 4) * 0.22f
            val ratio = f.width.toFloat() / f.height.coerceAtLeast(1)
            val h = H * (slot.h / 100f) * 1.25f
            val w = h * ratio
            val a = remember { Animatable(0f) }
            LaunchedEffect(Unit) { delay(120L + i * 75L); a.animateTo(1f, spring(dampingRatio = 0.997f, stiffness = 170f)) }
            val rot = tilt(i)
            Box(
                Modifier.offset(x = W * slot.x - w / 2, y = H * slot.y - h / 2).size(w, h)
                    .graphicsLayer {
                        val v = a.value
                        translationX = sx.value * 18f * depth * density + (1f - v) * 70f * density
                        translationY = sy.value * 13f * depth * density + (1f - v) * 150f * density
                        rotationZ = rot + (1f - v) * (rot + 22f)
                        val s = 0.78f + 0.22f * v; scaleX = s; scaleY = s
                        alpha = v.coerceIn(0f, 1f)
                    }
                    .shadow(10.dp, RoundedCornerShape(4.dp))
                    .background(Color.White)
                    .padding(4.dp),
            ) {
                AsyncImage(File(thumbs[f.path] ?: f.path), null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
            }
        }
        // 取景框的四个角
        val k = 14.dp
        listOf(Alignment.TopStart, Alignment.TopEnd, Alignment.BottomStart, Alignment.BottomEnd).forEach { al ->
            Box(Modifier.align(al).padding(12.dp).size(k).drawBehind {
                val sw = 1.5.dp.toPx()
                val left = al == Alignment.TopStart || al == Alignment.BottomStart
                val top = al == Alignment.TopStart || al == Alignment.TopEnd
                val x = if (left) 0f else size.width; val y = if (top) 0f else size.height
                drawLine(c.ash.copy(alpha = 0.6f), Offset(x, y), Offset(if (left) size.width else 0f, y), sw)
                drawLine(c.ash.copy(alpha = 0.6f), Offset(x, y), Offset(x, if (top) size.height else 0f), sw)
            })
        }
    }
}
