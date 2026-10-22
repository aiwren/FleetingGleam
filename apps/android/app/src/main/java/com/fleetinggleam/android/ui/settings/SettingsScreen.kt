package com.fleetinggleam.android.ui.settings

import android.util.Base64
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CheckboxDefaults
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.offset
import androidx.compose.ui.layout.layout
import androidx.compose.runtime.saveable.rememberSaveable
import com.fleetinggleam.android.core.Backup
import com.fleetinggleam.android.core.BackupMeta
import com.fleetinggleam.android.core.BackupTarget
import com.fleetinggleam.android.core.Core
import com.fleetinggleam.android.core.I18n
import com.fleetinggleam.android.core.Media
import com.fleetinggleam.android.core.Models
import com.fleetinggleam.android.core.Nas
import com.fleetinggleam.android.core.Pickers
import com.fleetinggleam.android.core.PrefsInfo
import com.fleetinggleam.android.core.Settings
import com.fleetinggleam.android.core.Share
import com.fleetinggleam.android.core.Strategy
import com.fleetinggleam.android.core.Styles
import com.fleetinggleam.android.core.TasteSummary
import com.fleetinggleam.android.core.args
import com.fleetinggleam.android.core.baseName
import com.fleetinggleam.android.core.fmtBytes
import com.fleetinggleam.android.core.fmtWhen
import com.fleetinggleam.android.core.Answer
import com.fleetinggleam.android.core.Fg
import com.fleetinggleam.android.core.rememberLang
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.core.tastePair
import com.fleetinggleam.android.ui.Actions
import com.fleetinggleam.android.ui.components.FgButton
import com.fleetinggleam.android.ui.components.FgDialog
import com.fleetinggleam.android.ui.components.FgField
import com.fleetinggleam.android.ui.components.FgSwitch
import com.fleetinggleam.android.ui.components.IconBtn
import com.fleetinggleam.android.ui.components.Icons
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.components.Ktag
import com.fleetinggleam.android.ui.components.LearnMeter
import com.fleetinggleam.android.ui.components.Segmented
import com.fleetinggleam.android.ui.components.Select
import com.fleetinggleam.android.ui.components.SettingGroup
import com.fleetinggleam.android.ui.components.SettingRow
import com.fleetinggleam.android.ui.components.StateDot
import com.fleetinggleam.android.ui.components.Stepper
import com.fleetinggleam.android.ui.components.Toasts
import com.fleetinggleam.android.ui.components.riseIn
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject

/**
 * 设置（桌面端 Settings.tsx 的手机版，DESIGN.md 第 14 节）：一页，按分组排；顶上一排可以点的分组名，点了滚到那里。
 * 通用（语言、外观）· 分析 · 个性化（学习喜好、从文件夹学、风格）· 数据（模型、缩略图缓存、隐私）· 备份 · NAS · 关于。
 */
@Composable
fun SettingsScreen(onOpenStyles: () -> Unit) {
    rememberLang()
    val c = LocalFg.current
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val s by Settings.state.collectAsState()
    val info by Settings.info.collectAsState()
    val packs by Models.packs.collectAsState()
    val shares by Nas.shares.collectAsState()
    val backup by Backup.status.collectAsState()
    var prefs by remember { mutableStateOf<PrefsInfo?>(null) }
    var cache by remember { mutableStateOf<Long?>(null) }
    var cleared by remember { mutableStateOf(false) }
    var prefsReset by remember { mutableStateOf(false) }
    var refBusy by remember { mutableStateOf<Float?>(null) }
    var refFailed by remember { mutableStateOf(false) }
    var tasteBusy by remember { mutableStateOf(false) }
    var tasteMsg by remember { mutableStateOf<String?>(null) }
    val version = remember { runCatching { ctx.packageManager.getPackageInfo(ctx.packageName, 0).versionName }.getOrNull() ?: "dev" }
    val list = rememberLazyListState()
    /** 打开的分区（null = 根页的分级列表）。 */
    var section by rememberSaveable { mutableStateOf<String?>(null) }
    androidx.activity.compose.BackHandler(section != null) { section = null }
    // 别的页面要求打开某个分区（起始屏的"连接 NAS"）
    val wanted by com.fleetinggleam.android.core.Nav.settings.collectAsState()
    LaunchedEffect(wanted) { if (wanted != null) { section = wanted; com.fleetinggleam.android.core.Nav.consumed() } }

    LaunchedEffect(Unit) {
        Styles.refresh(); Nas.refresh(); Backup.refresh(); Models.refresh()
        prefs = Styles.prefsInfo()
        cache = runCatching { Core.get<Long>("cache_size") }.getOrNull()
    }

    // ---- 对话框的状态（备份目标、NAS）
    var bkForm by remember { mutableStateOf<BkForm?>(null) }
    var bkRemoving by remember { mutableStateOf<BackupTarget?>(null) }
    var bkPick by remember { mutableStateOf(false) }
    var bkConfirm by remember { mutableStateOf<BackupTarget?>(null) }
    var bkBusy by remember { mutableStateOf<String?>(null) }
    var bkMsg by remember { mutableStateOf<String?>(null) }
    var nasForm by remember { mutableStateOf<NasForm?>(null) }
    var nasRemoving by remember { mutableStateOf<Share?>(null) }
    var nasBusy by remember { mutableStateOf<String?>(null) }
    var nasMsg by remember { mutableStateOf<Pair<String, String>?>(null) }

    val learnStatus = prefs?.let { if (it.labels == 0) t("settings.learnNone") else t("settings.learnStatus", "n" to it.labels, "k" to it.keeps) } ?: t("settings.learnNone")
    val clipLabel = info?.let { i -> if (i.clip == null) t("settings.off") else if (i.clip.startsWith("gpu") || i.clip.startsWith("coreml") || i.clip.startsWith("nnapi")) t("android.settings.clipNpu") else t("settings.clipCpu") } ?: "—"
    val bkSummary = run {
        val b = backup
        when {
            bkMsg != null -> bkMsg!!
            b == null || !b.configured -> t("settings.backupDesc")
            b.targets.any { it.lastError != null } -> b.targets.first { it.lastError != null }.let { t("settings.backupLastError", "error" to "${it.name}: ${it.lastError}") }
            (b.targets.maxOfOrNull { it.lastOk } ?: 0L) > 0 -> t("settings.backupLast", "when" to fmtWhen(b.targets.maxOf { it.lastOk }))
            else -> t("settings.backupNever")
        }
    }

    fun adoptCarried(carried: JsonObject?, r: TasteSummary) {
        Settings.adopt(carried)
        scope.launch { Styles.refresh(); Styles.bumpRevision(); prefs = Styles.prefsInfo() }
        tasteMsg = t("settings.importTasteDone", "styles" to r.styles, "examples" to r.examples, "refs" to r.refs)
    }

    val sections = listOf("general" to t("settings.general"), "analysis" to t("settings.analysis"), "personal" to t("settings.personal"), "data" to t("settings.data"), "backup" to t("settings.backup"), "nas" to t("settings.nas"), "about" to t("settings.about"))
    // 设置页的按钮统一用小号（满屏的行，常规按钮显得大）
    androidx.compose.runtime.CompositionLocalProvider(com.fleetinggleam.android.ui.components.LocalSmallButtons provides true) {
    LazyColumn(Modifier.fillMaxSize().statusBarsPadding(), state = list, contentPadding = androidx.compose.foundation.layout.PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 110.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        item {
            if (section == null) {
                // 根页：大标题，下面一张分级列表（像 iOS 设置）
                Text(t("settings.title"), style = Type.largeTitle.copy(color = c.ink), modifier = Modifier.padding(start = 4.dp, top = 10.dp, bottom = 12.dp))
                val rows = listOf(
                    Triple("general", Icons.SETTINGS, t("settings.general")) to (I18n.LANGS.firstOrNull { it.code == I18n.lang.value }?.label ?: t("nav.systemLang")),
                    Triple("analysis", "M2 8h2.5l1.5-4 2 8 1.5-4H14", t("settings.analysis")) to (s.tier?.let { t(if (it == "ultra") "settings.qualityUltra" else if (it == "high") "settings.qualityHigh" else "settings.qualityFast") } ?: t("settings.auto")),
                    Triple("personal", Icons.STYLE, t("settings.personal")) to (if (s.learn) t("android.settings.on") else t("android.settings.off")),
                    Triple("data", "M2.5 4.5h11v3h-11zM2.5 8.5h11v3h-11zM4.5 6h.01M4.5 10h.01", t("android.settings.modelsStorage")) to "${packs.count { it.installed }} / ${packs.size}",
                    Triple("backup", "M8 2.5v7M5 6.5l3 3 3-3M3 10.5v2a1 1 0 001 1h8a1 1 0 001-1v-2", t("settings.backup")) to (backup?.let { if (it.configured && it.enabled) t("android.settings.on") else t("android.settings.off") } ?: ""),
                    Triple("nas", Icons.NAS, t("settings.nas")) to (if (shares.isEmpty()) "" else "${shares.size}"),
                    Triple("about", Icons.INFO, t("settings.about")) to "v$version",
                )
                Column(Modifier.clip(RoundedCornerShape(14.dp)).background(c.panel)) {
                    rows.forEachIndexed { i, (r, value) -> com.fleetinggleam.android.ui.components.NavRow(r.second, r.third, value, first = i == 0) { section = r.first } }
                }
            } else {
                com.fleetinggleam.android.ui.components.NavBar(if (section == "data") t("android.settings.modelsStorage") else sections.firstOrNull { it.first == section }?.second ?: "", t("settings.title"), { section = null }, Modifier.layout { m, cs -> val pl = m.measure(cs.copy(maxWidth = cs.maxWidth + 32.dp.roundToPx(), minWidth = 0)); layout(cs.maxWidth, pl.height) { pl.placeRelative(-16.dp.roundToPx(), 0) } }.padding(bottom = 6.dp))
            }
        }
        if (section == "general") item {
            SettingGroup("") {
                SettingRow(t("settings.language"), t("settings.languageDesc"), first = true) {
                    Select(I18n.pref(), listOf("system" to t("nav.systemLang")) + I18n.LANGS.map { it.code to it.label }, { I18n.setPref(it) })
                }
                SettingRow(t("settings.theme"), t("settings.themeDesc"), below = {
                    Segmented(listOf("system" to t("nav.systemLang"), "light" to t("settings.light"), "dark" to t("settings.dark")), s.theme, { v -> Settings.set { it.copy(theme = v) } }, block = true)
                })
                SettingRow(t("android.settings.glass"), t("android.settings.glassDesc")) { FgSwitch(s.glass, { v -> Settings.set { it.copy(glass = v) } }, t("android.settings.glass")) }
            }
        }
        if (section == "analysis") item {
            SettingGroup("") {
                val how = buildString {
                    append(t("android.settings.analysisHow")); append("\n\n"); append(t("settings.lastRun")); append("\n")
                    val i = info
                    if (i == null) append(t("settings.lastRunNone")) else {
                        append("${t("settings.modelUsed")}: ${i.model?.let { m -> t(if ("SigLIP" in m) "settings.qualityUltra" else if ("L/14" in m) "settings.qualityHigh" else "settings.qualityFast") } ?: "—"}\n")
                        append("${t("settings.backendClip")}: $clipLabel\n")
                        append("${t("android.settings.faces")}: ${if (i.vision) t("android.settings.facesOn") else t("settings.off")}\n")
                        append("${t("settings.backendCores")}: ${i.cores}")
                    }
                }
                SettingRow(t("settings.analysisAuto"), t("settings.analysisAutoDesc"), info = how, first = true) {
                    Text(s.tier?.let { t(if (it == "ultra") "settings.qualityUltra" else if (it == "high") "settings.qualityHigh" else "settings.qualityFast") } ?: t("settings.auto"), style = Type.mono.copy(color = c.ash))
                }
            }
        }
        if (section == "analysis") item {
            // 模型档位（作者 2026-10-22："怎么只剩标准的了"）：自动 / 标准 / 高质量 / 极致，没装的选了就下载；换了之后正在看的项目重新评估
            SettingGroup(t("android.settings.tierTitle")) {
                val options = listOf(null to "auto", "fast" to "fast", "high" to "high", "ultra" to "ultra")
                options.forEachIndexed { i, (tier, id) ->
                    val pack = if (tier == null) null else packs.firstOrNull { it.id == (if (tier == "high") "high" else if (tier == "ultra") "ultra" else "standard") }
                    val installed = tier == null || pack?.installed == true
                    val title = if (tier == null) t("android.settings.tierAuto") else t(if (tier == "ultra") "settings.qualityUltra" else if (tier == "high") "settings.qualityHigh" else "settings.qualityFast")
                    val desc = if (tier == null) t("android.settings.tierAutoDesc") else t("android.settings.tierDesc.$id") + " · " + (if (installed) t("android.settings.tierInstalled") else t("android.settings.tierNeeds", "size" to fmtBytes(pack?.bytes ?: 0)))
                    val on = s.tier == tier
                    SettingRow(title, desc, first = i == 0, onClick = {
                        scope.launch {
                            if (tier == null) { Settings.set { it.copy(tier = null, tierRev = it.tierRev + 1) }; return@launch }
                            if (Models.ensurePacks(Strategy.tierPacks(tier)) == Answer.Cancelled) return@launch
                            Settings.set { it.copy(tier = tier, tierRev = it.tierRev + 1) }
                        }
                    }) {
                        Box(Modifier.size(20.dp).clip(CircleShape).border(1.5.dp, if (on) c.ink else c.line, CircleShape), contentAlignment = Alignment.Center) {
                            if (on) Box(Modifier.size(10.dp).clip(CircleShape).background(c.ink))
                        }
                    }
                }
            }
            Text(t("android.settings.tierHint"), style = Type.rowDesc.copy(color = c.ash), modifier = Modifier.padding(start = 16.dp, top = 6.dp))
        }
        if (section == "personal") item {
            SettingGroup("") {
                SettingRow(t("settings.learn"), t("settings.learnShort"), info = "${t("settings.learnDesc")}\n\n$learnStatus", first = true, below = prefs?.let { p -> { LearnMeter(p) } }) {
                    FgSwitch(s.learn, { v -> Settings.set { it.copy(learn = v) } }, t("settings.learn"))
                }
                SettingRow(
                    t("styles.fromFolder"),
                    if (refFailed) t("styles.failed") else prefs?.takeIf { it.refs > 0 }?.let { "${t("styles.fromFolderDone", "count" to it.refs)} · ${baseName(it.refsFrom)}" } ?: t("styles.fromFolderDesc"),
                    info = t("styles.fromFolderInfo"),
                ) {
                    FgButton(refBusy?.let { t("styles.learning", "pct" to Math.round(it * 100)) } ?: t("styles.chooseFolder"), {
                        scope.launch {
                            val dir = Actions.pickFolder() ?: return@launch
                            refFailed = false; refBusy = 0f
                            // 镜像里的文件夹（NAS、系统文件夹）先同步，学的是里面的照片
                            withContext(Dispatchers.IO) { runCatching { Media.syncMirrors(ctx, listOf(dir), { _, _, _ -> }, { false }); Core.callBlocking("probe_folder", args("dirs" to listOf(dir))) } }
                            val ok = Styles.learnFromFolder(dir, null) { refBusy = it }
                            refBusy = null
                            if (!ok) refFailed = true else prefs = Styles.prefsInfo()
                        }
                    }, small = true, enabled = refBusy == null)
                }
                SettingRow(t("styles.title"), t("styles.desc")) { FgButton(t("styles.open"), onOpenStyles, small = true) }
                prefs?.takeIf { it.labels > 0 || it.refs > 0 }?.let { p ->
                    SettingRow(t("settings.learnReset"), t("settings.learned", "count" to p.labels + p.refs, "n" to p.labels + p.refs)) {
                        FgButton(if (prefsReset) t("settings.learnResetDone") else t("settings.learnReset"), {
                            scope.launch { runCatching { prefs = Core.get("prefs_reset"); prefsReset = true; Styles.bumpRevision() } }
                        }, small = true)
                    }
                }
            }
        }
        if (section == "data") item {
            SettingGroup("") {
                SettingRow(t("models.title"), t("models.titleDesc"), first = true)
                packs.forEach { p ->
                    SettingRow(t("models.name.${p.id}"), "${if (p.installed) t("models.have") else t("models.none") + if (p.bytes > 0) " · " + fmtBytes(p.bytes) else ""} · ${t("models.desc.${p.id}")}") {
                        if (p.installed) FgButton(t("models.remove"), { scope.launch { runCatching { Models.remove(p.id) }; Strategy.release(p.id) } }, small = true)
                        else {
                            // 正在（后台）下载的这一包：显示进度，点一下把下载对话框叫回来
                            val gate by Models.gate.collectAsState()
                            val g = gate?.takeIf { it.phase == "downloading" && (it.id == p.id || it.ids?.contains(p.id) == true) }
                            if (g != null) FgButton("${if (g.total > 0) Math.round(g.done * 100f / g.total) else 0}%", { Models.showGate() }, small = true)
                            else FgButton(t("models.get"), { scope.launch { if (Models.ensurePack(p.id) != Answer.Cancelled) Strategy.adopt(p.id) } }, small = true)
                        }
                    }
                }
                SettingRow(t("settings.cache"), t("settings.cacheDesc")) {
                    Text(cache?.let { fmtBytes(it) } ?: "—", style = Type.mono.copy(color = c.ash))
                    FgButton(if (cleared) t("settings.cleared") else t("settings.clear"), {
                        scope.launch { runCatching { Core.call("clear_cache"); cache = 0; cleared = true; Settings.bumpCacheRev() } }
                    }, small = true, enabled = (cache ?: 0) > 0)
                }
                SettingRow(t("settings.privacy"), t("android.settings.privacyBody"))
            }
        }
        if (section == "backup") item {
            SettingGroup("") {
                SettingRow(t("settings.exportTaste"), tasteMsg ?: t("settings.exportTasteDesc"), info = t("settings.exportTasteInfo"), first = true, below = {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        FgButton(t("settings.exportTasteBtn"), {
                            scope.launch {
                                val uri = Pickers.createJson("fleetinggleam-taste.json") ?: return@launch
                                tasteBusy = true; tasteMsg = null
                                try {
                                    val raw = Core.call("taste_export_bytes", args("settings" to Settings.asJson()))
                                    val obj = Fg.json.parseToJsonElement(raw).jsonObject
                                    val bytes = Base64.decode(obj["base64"].toString().trim('"'), Base64.DEFAULT)
                                    withContext(Dispatchers.IO) { ctx.contentResolver.openOutputStream(uri, "wt")?.use { it.write(bytes) } }
                                    val r = Fg.json.decodeFromJsonElement(TasteSummary.serializer(), obj["summary"]!!)
                                    tasteMsg = t("settings.exportTasteDone", "styles" to r.styles, "examples" to r.examples, "refs" to r.refs) + if (r.skipped > 0) " " + t("settings.exportTasteSkipped", "n" to r.skipped) else ""
                                } catch (e: Exception) { tasteMsg = t("settings.tasteFailed", "error" to e.message) }
                                tasteBusy = false
                            }
                        }, small = true, enabled = !tasteBusy)
                        FgButton(t("settings.importTasteBtn"), {
                            scope.launch {
                                val uri = Pickers.openJson() ?: return@launch
                                tasteBusy = true; tasteMsg = null
                                try {
                                    val bytes = withContext(Dispatchers.IO) { ctx.contentResolver.openInputStream(uri)?.use { it.readBytes() } } ?: throw IllegalStateException("cannot read the file")
                                    val (r, carried) = tastePair(Core.call("taste_import_bytes", args("base64" to Base64.encodeToString(bytes, Base64.NO_WRAP))))
                                    adoptCarried(carried, r)
                                } catch (e: Exception) { tasteMsg = t("settings.tasteFailed", "error" to e.message) }
                                tasteBusy = false
                            }
                        }, small = true, enabled = !tasteBusy)
                    }
                })
                val b = backup
                SettingRow(t("settings.backupTitle"), bkSummary, info = t("settings.backupInfo"), below = if (b?.configured == true) ({
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        FgButton(if (bkBusy == "now") t("settings.backupRunning") else t("settings.backupNow"), {
                            scope.launch { bkBusy = "now"; bkMsg = null; try { Backup.now(); bkMsg = t("settings.backupDone") } catch (e: Exception) { bkMsg = t("settings.backupFailed", "error" to e.message); Backup.refresh() }; bkBusy = null }
                        }, small = true, enabled = bkBusy == null)
                        FgButton(if (bkBusy == "restore") t("settings.backupRestoring") else t("settings.backupRestore"), {
                            val l = b.targets
                            if (l.size == 1) bkConfirm = l[0] else if (l.size > 1) bkPick = true
                        }, small = true, enabled = bkBusy == null)
                    }
                }) else null) {
                    FgSwitch(b?.configured == true && b.enabled, { scope.launch { bkBusy = "toggle"; runCatching { Backup.setOptions(enabled = !(b?.enabled ?: false)) }.onFailure { bkMsg = it.message }; bkBusy = null } }, t("settings.backupAuto"), enabled = b?.configured == true && bkBusy == null)
                }
                SettingRow(t("settings.backupKeep"), t("settings.backupKeepHint")) {
                    Stepper(b?.keep ?: 30, { n -> scope.launch { runCatching { Backup.setOptions(keep = n) }.onFailure { bkMsg = it.message } } }, t("settings.backupKeep"), enabled = b?.configured == true)
                }
                SettingRow(t("settings.backupTargets"), t("settings.backupTargetsDesc")) { FgButton(t("settings.backupAddTarget"), { bkForm = BkForm() }, small = true, enabled = bkBusy == null) }
                (b?.targets ?: emptyList()).forEach { tg ->
                    SettingRow(tg.name, "Cloudflare R2 · ${tg.bucket}", below = {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            StateDot(if (tg.lastError != null) t("settings.backupErrored") else if (tg.lastOk > 0) t("settings.backupLast", "when" to fmtWhen(tg.lastOk)) else t("settings.backupReady"), tg.lastError == null)
                            Spacer(Modifier.weight(1f))
                            IconBtn(Icons.EDIT, t("settings.backupEdit"), { bkForm = BkForm(tg.id, tg.name, tg.account, tg.bucket, tg.accessKey, "", tg.hasSecret) }, enabled = bkBusy == null)
                            IconBtn(Icons.REMOVE, t("settings.backupRemove"), { bkRemoving = tg }, enabled = bkBusy == null)
                        }
                    })
                }
            }
        }
        if (section == "nas") item {
            SettingGroup("") {
                SettingRow(t("settings.nasTitle"), t("settings.nasDesc"), info = t("settings.nasInfo"), first = true) { FgButton(t("settings.nasAdd"), { nasForm = NasForm() }, small = true) }
                shares.forEach { sh ->
                    SettingRow(sh.name, "${sh.host} · ${sh.user.ifEmpty { t("settings.nasGuest") }}" + (nasMsg?.takeIf { it.first == sh.id }?.let { " · ${it.second}" } ?: ""), below = {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            StateDot(if (!sh.enabled) t("settings.nasDisabled") else if (sh.connected) t("settings.nasConnected") else if (nasBusy == sh.id) t("settings.nasConnecting") else t("settings.nasOffline"), sh.enabled && sh.connected)
                            Spacer(Modifier.weight(1f))
                            FgButton(t("settings.nasNewProject"), { scope.launch { if (Actions.newProjectFromDialog(sh.id)) Toasts.show(t("android.settings.projectMade")) } }, small = true, enabled = sh.enabled)
                            IconBtn(Icons.EDIT, t("settings.nasEdit"), { nasForm = NasForm(sh.id, sh.name, sh.host, sh.user, "", true, editingUser = sh.user, editingHost = sh.host) }, enabled = nasBusy != sh.id)
                            IconBtn(Icons.REMOVE, t("settings.nasRemove"), { nasRemoving = sh }, enabled = nasBusy != sh.id)
                        }
                    }) {
                        FgSwitch(sh.enabled, { v -> scope.launch { nasBusy = sh.id; nasMsg = null; runCatching { Nas.setEnabled(sh.id, v) }.onFailure { nasMsg = sh.id to (it.message ?: "") }; nasBusy = null } }, t("settings.nasEnabled"), enabled = nasBusy != sh.id)
                    }
                }
            }
        }
        if (section == "about") item {
            // 关于：应用介绍、几条链接（官网、源代码、隐私、条款、反馈、第三方许可）、版本（作者 2026-10-22 要求）
            val zh = I18n.isZh
            val site = "https://fleetinggleam.fleetinggleam-web.workers.dev"; val repo = "https://github.com/lcopilot/FleetingGleam"
            val openUrl = { url: String -> runCatching { ctx.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(url)).addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)) } }
            // 隐私政策、使用条款：在应用里从底部推上来的半屏浏览器（Custom Tabs 的局部高度）里看，不跳出去；没有支持的浏览器就照常打开
            LaunchedEffect(Unit) { WebSheet.warm(ctx) }
            val openSheet = { url: String -> WebSheet.open(ctx, url) { openUrl(url) } }
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(c.panel).padding(16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    androidx.compose.foundation.Image(androidx.compose.ui.res.painterResource(com.fleetinggleam.android.R.drawable.app_icon), null, Modifier.size(64.dp).clip(RoundedCornerShape(16.dp)))
                    Text(t("brand.name"), style = Type.title.copy(color = c.ink), modifier = Modifier.padding(top = 10.dp))
                    if (zh) Text(t("brand.latin"), style = Type.monoSmall.copy(color = c.ash))
                    Text(t("brand.tagline"), style = Type.rowDesc.copy(color = c.ash), textAlign = androidx.compose.ui.text.style.TextAlign.Center, modifier = Modifier.padding(top = 6.dp))
                    Text(t("settings.aboutIntro"), style = Type.body.copy(color = c.ink), textAlign = androidx.compose.ui.text.style.TextAlign.Center, modifier = Modifier.padding(top = 12.dp))
                }
                SettingGroup(t("settings.aboutLinks")) {
                    // (图标, 标题 to 说明, 地址, 是否在应用内半屏看)
                    data class Link(val icon: String, val title: String, val desc: String, val url: String, val sheet: Boolean)
                    val rows = listOf(
                        Link(Icons.CULL, t("settings.aboutSite"), t("settings.aboutSiteDesc"), if (zh) "$site/zh" else site, false),
                        Link("M5.5 2.5L2 8l3.5 5.5M10.5 2.5L14 8l-3.5 5.5M9 2l-2 12", t("settings.aboutSource"), t("settings.aboutSourceDesc"), repo, false),
                        Link("M3.5 2.5h9a1 1 0 011 1v9a1 1 0 01-1 1h-9a1 1 0 01-1-1v-9a1 1 0 011-1zM5 5.5h6M5 8h6M5 10.5h3", t("settings.aboutPrivacy"), "", if (zh) "$site/zh/privacy" else "$site/privacy", true),
                        Link("M4 2.5h8v11H4zM6 5.5h4M6 8h4", t("settings.aboutTerms"), "", if (zh) "$site/zh/terms" else "$site/terms", true),
                        Link("M8 2.5a5.5 5.5 0 100 11 5.5 5.5 0 000-11zM8 5v3.5M8 11h.01", t("settings.aboutIssues"), t("settings.aboutIssuesDesc"), "$repo/issues", false),
                    )
                    rows.forEachIndexed { i, l ->
                        SettingRow(l.title, l.desc.ifEmpty { null }, first = i == 0, control = { FgIcon(Icons.CHEVRON, size = 14.dp, stroke = 1.6f, color = c.ash) }, onClick = { if (l.sheet) openSheet(l.url) else openUrl(l.url) }, icon = l.icon)
                    }
                }
                SettingGroup("") {
                    SettingRow(t("settings.aboutVersion"), null, first = true) { Text("v$version", style = Type.mono.copy(color = c.ash)) }
                }
            }
        }
    }
    }

    // ---- 备份目标：添加 / 编辑
    bkForm?.let { f0 ->
        var f by remember(f0) { mutableStateOf(f0) }
        var err by remember(f0) { mutableStateOf<String?>(null) }
        val saving = bkBusy == "save"
        FgDialog(true, { if (!saving) bkForm = null }, if (f.id != null) t("settings.backupEditTitle", "name" to f.name) else t("settings.backupAddTitle"), locked = saving, footer = {
            FgButton(t("settings.nasCancel"), { bkForm = null }, enabled = !saving)
            FgButton(if (saving) t("settings.backupTesting") else t("settings.backupSave"), {
                scope.launch {
                    bkBusy = "save"; err = null
                    try { Backup.saveTarget(f.id, f.name, f.account, f.bucket, f.accessKey, f.secret); bkForm = null; bkMsg = t("settings.backupSaved") } catch (e: Exception) { err = e.message }
                    bkBusy = null
                }
            }, primary = true, enabled = bkBusy == null && f.account.isNotBlank() && f.bucket.isNotBlank() && f.accessKey.isNotBlank() && (f.secret.isNotEmpty() || f.hasSecret))
        }) {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                FgField(f.name, { f = f.copy(name = it) }, label = t("settings.backupName"), placeholder = t("settings.backupNameHint"))
                Text(t("settings.backupHow"), style = Type.rowDesc.copy(color = c.ash))
                FgField(f.account, { f = f.copy(account = it) }, label = t("settings.backupAccount"), placeholder = t("settings.backupAccountHint"))
                FgField(f.bucket, { f = f.copy(bucket = it) }, label = t("settings.backupBucket"), placeholder = "fleetinggleam")
                FgField(f.accessKey, { f = f.copy(accessKey = it) }, label = "Access Key ID")
                FgField(f.secret, { f = f.copy(secret = it) }, label = "Secret Access Key", password = true, placeholder = if (f.hasSecret) t("settings.backupSecretKept") else "")
                err?.let { Text(t("settings.backupFailed", "error" to it), style = Type.rowDesc.copy(color = c.ink)) }
            }
        }
    }
    FgDialog(bkRemoving != null, { bkRemoving = null }, t("settings.backupRemoveTitle", "name" to (bkRemoving?.name ?: "")), footer = {
        FgButton(t("settings.nasCancel"), { bkRemoving = null })
        FgButton(t("settings.backupRemoveConfirm"), { val tg = bkRemoving ?: return@FgButton; bkRemoving = null; scope.launch { bkBusy = "remove"; bkMsg = null; runCatching { Backup.removeTarget(tg.id) }.onFailure { bkMsg = it.message }; bkBusy = null } }, primary = true)
    }) { Text(t("settings.backupRemoveBody"), style = Type.body.copy(color = c.ash)) }
    FgDialog(bkPick, { bkPick = false }, t("settings.backupRestorePick"), footer = { FgButton(t("settings.nasCancel"), { bkPick = false }) }) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            (backup?.targets ?: emptyList()).forEach { tg ->
                Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(c.panel).border(1.dp, c.line, RoundedCornerShape(12.dp)).clickable { bkPick = false; bkConfirm = tg }.padding(14.dp)) {
                    Text(tg.name, style = Type.bodyStrong.copy(color = c.ink))
                    Text("Cloudflare R2 · ${tg.bucket}" + if (tg.lastOk > 0) " · ${t("settings.backupLast", "when" to fmtWhen(tg.lastOk))}" else "", style = Type.rowDesc.copy(color = c.ash))
                }
            }
        }
    }
    // 手机和电脑可能备份到同一个桶：恢复前先说清楚这份是谁、什么时候做的
    var bkMeta by remember { mutableStateOf<BackupMeta?>(null) }
    LaunchedEffect(bkConfirm?.id) { bkMeta = null; bkConfirm?.let { tg -> bkMeta = runCatching { Backup.peek(tg.id) }.getOrNull() } }
    FgDialog(bkConfirm != null, { bkConfirm = null }, t("settings.backupRestore"), footer = {
        FgButton(t("settings.nasCancel"), { bkConfirm = null })
        FgButton(t("settings.backupRestore"), {
            val tg = bkConfirm ?: return@FgButton
            bkConfirm = null
            scope.launch {
                bkBusy = "restore"; bkMsg = null
                try { val (r, carried) = Backup.restore(tg.id); adoptCarried(carried, r); bkMsg = tasteMsg } catch (e: Exception) { bkMsg = t("settings.backupFailed", "error" to e.message) }
                bkBusy = null
            }
        }, primary = true)
    }) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(t("settings.backupRestoreConfirm", "name" to (bkConfirm?.name ?: "")), style = Type.body.copy(color = c.ink))
            bkMeta?.let { m -> Text(t("settings.backupFrom", "device" to m.device.ifEmpty { m.platform }, "when" to fmtWhen(m.exportedAt)), style = Type.body.copy(color = c.ash)) }
        }
    }

    // ---- NAS：添加 / 编辑（先"测试连接"，连上才能保存）
    nasForm?.let { f0 ->
        var f by remember(f0) { mutableStateOf(f0) }
        var probed by remember(f0) { mutableStateOf<List<String>?>(null) }
        var err by remember(f0) { mutableStateOf<String?>(null) }
        var busy by remember(f0) { mutableStateOf<String?>(null) }
        val unchanged = f.id != null && f.host.trim() == f.editingHost && f.user.trim() == f.editingUser && f.password.isEmpty()
        FgDialog(true, { if (busy != "new") nasForm = null }, if (f.id != null) t("settings.nasEditTitle", "name" to f.name) else t("settings.nasAddTitle"), locked = busy == "new", footer = {
            FgButton(t("settings.nasCancel"), { nasForm = null }, enabled = busy != "new")
            FgButton(if (busy == "new") t("settings.nasConnecting") else if (f.id != null) t("settings.nasSaveChanges") else t("settings.nasConnectSave"), {
                scope.launch {
                    busy = "new"; err = null
                    try {
                        if (f.id != null) Nas.update(f.id!!, f.name, f.host, f.user, f.password, f.remember) else Nas.add(f.name, f.host, f.user, f.password, f.remember)
                        nasForm = null
                    } catch (e: Exception) { err = e.message }
                    busy = null
                }
            }, primary = true, enabled = busy == null && (probed != null || unchanged))
        }) {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(t("settings.nasHow"), style = Type.rowDesc.copy(color = c.ash))
                FgField(f.host, { f = f.copy(host = it); probed = null }, label = t("settings.nasHost"), placeholder = t("settings.nasHostHint"))
                FgField(f.user, { f = f.copy(user = it); probed = null }, label = t("settings.nasUser"), placeholder = t("settings.nasGuestHint"))
                FgField(f.password, { f = f.copy(password = it); probed = null }, label = t("settings.nasPassword"), password = true, placeholder = if (f.id != null && f.editingUser.isNotEmpty()) t("settings.nasPasswordKept") else "")
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    FgButton(if (busy == "probe") t("settings.nasProbing") else t("settings.nasProbe"), {
                        scope.launch {
                            busy = "probe"; err = null; probed = null
                            try { probed = Nas.probe(f.host, f.user, f.password, f.id) } catch (e: Exception) { err = e.message }
                            busy = null
                        }
                    }, small = true, enabled = busy == null && f.host.isNotBlank())
                    probed?.let { Ktag(if (it.isNotEmpty()) t("settings.nasProbeOk", "count" to it.size) else t("settings.nasProbeNone"), on = it.isNotEmpty()) }
                }
                err?.let { Text(t("settings.nasFailed", "error" to it), style = Type.rowDesc.copy(color = c.ink)) }
                probed?.takeIf { it.isNotEmpty() }?.let { list ->
                    Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) { list.forEach { Ktag(it) } }
                    Text(t("settings.nasSharesHint"), style = Type.rowDesc.copy(color = c.ash))
                }
                FgField(f.name, { f = f.copy(name = it) }, label = t("settings.nasName"), placeholder = t("settings.nasNameHint"))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(f.remember, { f = f.copy(remember = it) }, colors = CheckboxDefaults.colors(checkedColor = c.ink, checkmarkColor = c.onInk, uncheckedColor = c.ash))
                    Text(t("settings.nasRemember"), style = Type.rowDesc.copy(color = c.ink))
                }
            }
        }
    }
    FgDialog(nasRemoving != null, { nasRemoving = null }, t("settings.nasRemoveTitle", "name" to (nasRemoving?.name ?: "")), footer = {
        FgButton(t("settings.nasCancel"), { nasRemoving = null })
        FgButton(t("settings.nasRemoveConfirm"), { val sh = nasRemoving ?: return@FgButton; nasRemoving = null; scope.launch { nasBusy = sh.id; runCatching { Nas.remove(sh.id) }.onFailure { nasMsg = sh.id to (it.message ?: "") }; nasBusy = null } }, primary = true)
    }) { Text(t("settings.nasRemoveBody"), style = Type.body.copy(color = c.ash)) }
}

private data class BkForm(val id: String? = null, val name: String = "", val account: String = "", val bucket: String = "", val accessKey: String = "", val secret: String = "", val hasSecret: Boolean = false)
private data class NasForm(val id: String? = null, val name: String = "", val host: String = "", val user: String = "", val password: String = "", val remember: Boolean = true, val editingUser: String = "", val editingHost: String = "")
