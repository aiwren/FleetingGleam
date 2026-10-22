package com.fleetinggleam.android.ui.project

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import com.fleetinggleam.android.ui.components.IconBtn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import com.fleetinggleam.android.core.Project
import com.fleetinggleam.android.core.Projects
import com.fleetinggleam.android.core.baseName
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgButton
import com.fleetinggleam.android.ui.components.FgDialog
import com.fleetinggleam.android.ui.components.FgField
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.components.Icons
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.delay

@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
/** Everything about one project in one place: its name, folders, how it suggests keepers, re-analysing, and deleting it (the Mac's ProjectSettings.tsx). */
@Composable
fun ProjectSettings(open: Boolean, project: Project, onClose: () -> Unit, onAddFolder: () -> Unit, onRemoveFolder: (String) -> Unit, onTrash: () -> Unit, onForceRescan: () -> Unit) {
    val c = LocalFg.current
    var name by remember(project.id, open) { mutableStateOf(project.name) }
    var armed by remember(project.id, open) { mutableStateOf(false) }
    LaunchedEffect(armed) { if (armed) { delay(3000); armed = false } }
    val commit = { name.trim().takeIf { it.isNotEmpty() && it != project.name }?.let { v -> Projects.update(project.id) { it.copy(name = v) } } }
    if (!open) return
    val sheet = androidx.compose.material3.rememberModalBottomSheetState(skipPartiallyExpanded = true)
    // 手机上是从底部推上来的一整页：左上 ✕ 关掉（作者 2026-10-22：对话框没有关闭按钮），右上"完成"
    androidx.compose.material3.ModalBottomSheet(onDismissRequest = { commit(); onClose() }, sheetState = sheet, containerColor = c.canvas, dragHandle = null) {
        Box(Modifier.fillMaxWidth().height(52.dp).padding(horizontal = 8.dp)) {
            IconBtn(Icons.CLOSE, t("viewer.close"), { commit(); onClose() }, Modifier.align(Alignment.CenterStart), plain = true)
            Text(t("pset.title"), style = Type.navTitle.copy(color = c.ink), modifier = Modifier.align(Alignment.Center))
            FgButton(t("organize.done"), { commit(); onClose() }, Modifier.align(Alignment.CenterEnd).padding(end = 4.dp), primary = true, small = true)
        }
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(horizontal = 16.dp).padding(top = 4.dp, bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
            FgField(name, { name = it }, label = t("pset.name"), onDone = { commit() })
            Column {
                Text(t("pset.folders"), style = Type.overline.copy(color = c.ash), modifier = Modifier.padding(bottom = 6.dp))
                project.folders.forEach { f ->
                    Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text(baseName(f), style = Type.bodyStrong.copy(color = c.ink), maxLines = 1, overflow = Ellipsis)
                            Text(f, style = Type.monoSmall.copy(color = c.ash), maxLines = 1, overflow = Ellipsis)
                        }
                        Box(Modifier.size(32.dp).clip(RoundedCornerShape(8.dp)).clickable { onRemoveFolder(f) }, contentAlignment = Alignment.Center) { FgIcon(Icons.CLOSE, size = 14.dp, color = c.ash) }
                    }
                }
                Row(Modifier.padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    FgButton(t("cull.addFolder"), onAddFolder, small = true)
                }
                Text(t("cull.folderHint"), style = Type.small.copy(color = c.ash), modifier = Modifier.padding(top = 6.dp))
            }
            Column {
                Text(t("pset.prefs"), style = Type.overline.copy(color = c.ash), modifier = Modifier.padding(bottom = 10.dp))
                ProjectPrefs(project)
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Column(Modifier.weight(1f)) {
                    Text(t("pset.rescan"), style = Type.bodyStrong.copy(color = c.ink))
                    Text(t("pset.rescanDesc"), style = Type.small.copy(color = c.ash))
                }
                FgButton(t("pset.rescanBtn"), onForceRescan, small = true)
            }
            Box(Modifier.fillMaxWidth().height(1.dp).background(c.line))
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Column(Modifier.weight(1f)) {
                    Text(t("pset.danger"), style = Type.bodyStrong.copy(color = c.ink))
                    Text(t("pset.dangerDesc"), style = Type.small.copy(color = c.ash))
                }
                FgButton(if (armed) t("pset.confirmBin") else t("bin.toBin"), { if (armed) onTrash() else armed = true }, small = true, on = armed)
            }
        }
    }
}
