package com.fleetinggleam.android.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.layout.layout
import com.fleetinggleam.android.core.I18n
import com.fleetinggleam.android.core.Projects
import com.fleetinggleam.android.core.StyleInfo
import com.fleetinggleam.android.core.Styles
import com.fleetinggleam.android.core.baseName
import com.fleetinggleam.android.core.fmtDate
import com.fleetinggleam.android.core.fmtList
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.Actions
import com.fleetinggleam.android.ui.components.FgButton
import com.fleetinggleam.android.ui.components.FgDialog
import com.fleetinggleam.android.ui.components.FgField
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.components.FgSwitch
import com.fleetinggleam.android.ui.components.Icons
import com.fleetinggleam.android.ui.components.anchor
import com.fleetinggleam.android.ui.components.Ktag
import com.fleetinggleam.android.ui.components.LearnMeter
import com.fleetinggleam.android.ui.components.Segmented
import com.fleetinggleam.android.ui.components.riseIn
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.launch

/** A custom style's id is `s<millis-since-epoch>`, so its creation time is already in there; nothing to store twice. */
private fun createdAt(id: String): Long? = if (id == "default" || !id.startsWith("s")) null else id.drop(1).toLongOrNull()

/** Full-page style management (the Mac's Styles.tsx): several learned tastes, each with its own labels, reference photos and model. */
@Composable
fun StylesScreen(from: String, onBack: () -> Unit) {
    val c = LocalFg.current
    val styles by Styles.list.collectAsState()
    val store by Projects.store.collectAsState()
    val scope = rememberCoroutineScope()
    var busy by remember { mutableStateOf<Pair<String, Float>?>(null) }
    var failed by remember { mutableStateOf<String?>(null) }
    var query by remember { mutableStateOf("") }
    var sort by remember { mutableStateOf("name") }
    var confirming by remember { mutableStateOf<StyleInfo?>(null) }
    var creating by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { Styles.refresh() }
    val projects = store.projects.filter { it.trashed == null }
    // Who actually uses each style: an unset project.style means "whichever style is default", not a fixed id.
    val usedBy = remember(projects, styles) {
        val map = HashMap<String, MutableList<String>>()
        for (p in projects) { val id = p.style ?: styles.firstOrNull { it.isDefault }?.id ?: continue; map.getOrPut(id) { mutableListOf() }.add(p.name) }
        map
    }
    val display = { s: StyleInfo -> s.name.trim().ifEmpty { t("styles.defaultName") } }
    val shown = remember(styles, query, sort, usedBy) {
        val q = query.trim().lowercase()
        val list = styles.filter { s -> q.isEmpty() || display(s).lowercase().contains(q) || usedBy[s.id].orEmpty().any { it.lowercase().contains(q) } }
        val rest = list.filter { !it.isDefault }.sortedWith(when (sort) {
            "recent" -> compareByDescending { createdAt(it.id) ?: 0L }
            "usage" -> compareByDescending<StyleInfo> { usedBy[it.id]?.size ?: 0 }.thenBy { display(it) }
            else -> compareBy { display(it) }
        })
        listOfNotNull(list.firstOrNull { it.isDefault }) + rest
    }
    val learn: (String) -> Unit = { id ->
        scope.launch {
            val dir = Actions.pickFolder() ?: return@launch
            failed = null; busy = id to 0f
            val ok = Styles.learnFromFolder(dir, id) { busy = id to it }
            busy = null
            if (!ok) failed = id
        }
    }

    LazyColumn(Modifier.fillMaxSize().statusBarsPadding(), contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 60.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item {
            Column {
                com.fleetinggleam.android.ui.components.NavBar(
                    t("styles.title"), t(if (from == "settings") "settings.title" else "home.title"), onBack,
                    Modifier.layout { m, cs -> val pl = m.measure(cs.copy(maxWidth = cs.maxWidth + 32.dp.roundToPx(), minWidth = 0)); layout(cs.maxWidth, pl.height) { pl.placeRelative(-16.dp.roundToPx(), 0) } },
                ) {
                    Box(Modifier.padding(end = 4.dp).size(32.dp).clip(CircleShape).background(c.ink).clickable { creating = true }, contentAlignment = Alignment.Center) { FgIcon(Icons.PLUS, size = 16.dp, box = 18f, stroke = 1.7f, color = c.onInk) }
                }
                Text(t("styles.pageDesc") + " · " + t("styles.count", "count" to styles.size), style = Type.rowDesc.copy(color = c.ash), modifier = Modifier.padding(top = 2.dp, bottom = 4.dp))
                if (styles.size > 1) {
                    FgField(query, { query = it }, placeholder = t("styles.search"), modifier = Modifier.padding(top = 10.dp))
                    Segmented(listOf("name" to t("styles.sortName"), "recent" to t("styles.sortRecent"), "usage" to t("styles.sortUsage")), sort, { sort = it }, block = true, modifier = Modifier.padding(top = 10.dp))
                }
            }
        }
        if (shown.isEmpty()) item {
            Column(Modifier.padding(top = 20.dp)) {
                Text(t("styles.searchNone", "query" to query), style = Type.bodyStrong.copy(color = c.ink))
                Text(t("styles.searchNoneBody"), style = Type.rowDesc.copy(color = c.ash))
            }
        }
        itemsIndexed(shown, key = { _, s -> s.id }) { i, s ->
            val used = usedBy[s.id].orEmpty()
            val created = createdAt(s.id)
            val hasData = s.info.labels + s.info.refs > 0
            var name by remember(s.id, s.name) { mutableStateOf(s.name) }
            Column(Modifier.riseIn(i).animateItem().fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(c.panel).padding(14.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Box(Modifier.size(34.dp).clip(CircleShape).background(c.surface2), contentAlignment = Alignment.Center) { Text(display(s).take(1).uppercase(), style = Type.bodyStrong.copy(color = c.ink)) }
                    FgField(name, { name = it }, placeholder = t("styles.defaultName"), modifier = Modifier.weight(1f), onDone = { name.trim().takeIf { it.isNotEmpty() && it != s.name }?.let { v -> scope.launch { Styles.rename(s.id, v) } } })
                    val overlay = com.fleetinggleam.android.ui.components.LocalOverlay.current
                    val anchor = com.fleetinggleam.android.ui.components.rememberAnchor()
                    Box(Modifier.size(36.dp).clip(RoundedCornerShape(10.dp)).then(Modifier.anchor(anchor)).clickable {
                        overlay.showMenu(anchor.value, buildList {
                            if (!s.isDefault) add(com.fleetinggleam.android.ui.components.MenuItem(t("styles.makeDefault")) { scope.launch { Styles.setDefault(s.id) } })
                            if (hasData) add(com.fleetinggleam.android.ui.components.MenuItem(t("styles.reset")) { scope.launch { Styles.reset(s.id) } })
                            if (!s.isDefault) add(com.fleetinggleam.android.ui.components.MenuItem(t("styles.delete")) { confirming = s })
                        })
                    }, contentAlignment = Alignment.Center) { FgIcon(Icons.MORE, size = 18.dp, stroke = 1.5f) }
                }
                Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    if (s.isDefault) Ktag(t("styles.isDefault"))
                    if (hasData) Ktag(if (s.info.active) t("styles.active") else t("styles.warming"), on = s.info.active)
                }
                Text(
                    buildString {
                        append(if (hasData) t("styles.stats", "labels" to s.info.labels, "refs" to s.info.refs) else t("styles.empty"))
                        if (s.info.refs > 0 && s.info.refsFrom.isNotEmpty()) append(" · ${baseName(s.info.refsFrom)}")
                        if (used.isNotEmpty()) append(" · ${t("styles.usedBy", "count" to used.size)}")
                        if (created != null) append(" · ${t("styles.createdOn", "date" to fmtDate(created))}")
                        if (failed == s.id) append(" · ${t("styles.failed")}")
                    },
                    style = Type.rowDesc.copy(color = c.ash),
                )
                LearnMeter(s.info, compact = true)
                FgButton(busy?.takeIf { it.first == s.id }?.let { t("styles.learning", "pct" to Math.round(it.second * 100)) } ?: t("styles.learnFolder"), { learn(s.id) }, small = true, enabled = busy == null)
            }
        }
    }

    if (creating) {
        var name by remember { mutableStateOf("") }
        var learnNow by remember { mutableStateOf(true) }
        FgDialog(true, { creating = false }, t("styles.new"), footer = {
            FgButton(t("organize.cancel"), { creating = false })
            FgButton(if (learnNow) t("styles.createAndLearn") else t("styles.create"), {
                val v = name.trim()
                creating = false
                scope.launch { val id = Styles.create(v); if (id != null && learnNow) learn(id) }
            }, primary = true, enabled = name.isNotBlank())
        }) {
            Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
                FgField(name, { if (it.length <= 40) name = it }, label = t("styles.newName"), placeholder = t("styles.newPlaceholder"))
                Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    I18n.list("styles.newExamples").forEach { x ->
                        Text(x, style = Type.rowDesc.copy(color = c.ink), modifier = Modifier.clip(CircleShape).background(c.surface).clickable { name = x }.padding(horizontal = 12.dp, vertical = 6.dp))
                    }
                }
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    Column(Modifier.weight(1f)) {
                        Text(t("styles.newLearn"), style = Type.bodyStrong.copy(color = c.ink))
                        Text(t("styles.newLearnDesc"), style = Type.rowDesc.copy(color = c.ash))
                    }
                    FgSwitch(learnNow, { learnNow = it }, t("styles.newLearn"))
                }
            }
        }
    }
    confirming?.let { s ->
        val used = usedBy[s.id].orEmpty()
        FgDialog(true, { confirming = null }, t("styles.confirmDeleteTitle", "name" to s.name.trim().ifEmpty { t("styles.defaultName") }), footer = {
            FgButton(t("organize.cancel"), { confirming = null })
            FgButton(t("styles.confirmDeleteAction"), { confirming = null; scope.launch { Styles.delete(s.id) } }, on = true)
        }) {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(t("styles.confirmDeleteBody", "labels" to s.info.labels, "refs" to s.info.refs), style = Type.body.copy(color = c.ink))
                if (used.isNotEmpty()) Text(t("styles.confirmDeleteUsed", "count" to used.size, "names" to fmtList(used)), style = Type.rowDesc.copy(color = c.ash))
            }
        }
    }
}
