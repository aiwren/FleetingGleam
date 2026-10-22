package com.fleetinggleam.android.ui.models

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import kotlinx.coroutines.launch
import androidx.compose.ui.unit.dp
import com.fleetinggleam.android.core.Models
import com.fleetinggleam.android.core.fmtBytes
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgButton
import com.fleetinggleam.android.ui.components.FgDialog
import com.fleetinggleam.android.ui.components.LiveBar
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type

/** The one dialog for downloading models: says what and how big, shows progress, can be cancelled at any moment (the Mac's ModelGate.tsx). */
@Composable
fun ModelGate() {
    val c = LocalFg.current
    val g by Models.gate.collectAsState()
    val gate = g?.takeIf { !it.hidden } ?: return
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    val pct = if (gate.total > 0) minOf(100, Math.round(gate.done * 100f / gate.total)) else 0
    FgDialog(true, onDismiss = { Models.cancelGate() }, title = t("models.ask.${gate.id}"), locked = gate.phase == "downloading", footer = {
        FgButton(t("organize.cancel"), { Models.cancelGate() })
        // 下载转到后台：对话框收起，通知栏里看进度（先要通知权限）
        if (gate.phase == "downloading") FgButton(t("android.models.background"), { scope.launch { com.fleetinggleam.android.core.Pickers.notificationPermission(ctx); Models.hideGate() } }, primary = true)
        if (gate.phase == "ask") FgButton(t("models.download", "size" to fmtBytes(gate.bytes)), { Models.startDownload() }, primary = true)
        // 可选的包（人脸、眼睛、人物）没下成：可以先跳过，分析照常
        if (gate.phase == "error" && gate.failed != null && gate.failed in gate.optional) FgButton(t("android.models.skip"), { Models.skipGate() })
        if (gate.phase == "error") FgButton(t("models.retry"), { Models.startDownload() }, primary = true)
    }) {
        when (gate.phase) {
            "ask" -> Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(t("models.body.${gate.id}", "size" to fmtBytes(gate.bytes)), style = Type.body.copy(color = c.ink))
                gate.ids?.forEachIndexed { i, id ->
                    Row(Modifier.fillMaxWidth()) {
                        Text(t("models.name.$id"), style = Type.body.copy(color = c.ink), modifier = Modifier.weight(1f))
                        Text(fmtBytes(gate.sizes?.getOrNull(i) ?: 0), style = Type.mono.copy(color = c.ash))
                    }
                }
            }
            "downloading" -> Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                LiveBar(pct / 100f, Modifier.padding(top = 6.dp))
                Text("${fmtBytes(gate.done)} / ${fmtBytes(gate.total)} · $pct%", style = Type.mono.copy(color = c.ash))
            }
            else -> Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(t("models.error"), style = Type.body.copy(color = c.ash))
                Text(gate.error ?: "", style = Type.monoSmall.copy(color = c.ash))
                if (gate.failed != null && gate.failed in gate.optional) Text(t("android.models.skipBody"), style = Type.rowDesc.copy(color = c.ash), modifier = Modifier.padding(top = 6.dp))
            }
        }
    }
}
