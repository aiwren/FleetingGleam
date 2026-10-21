package com.fleetinggleam.android.ui.project

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import com.fleetinggleam.android.core.Answer
import com.fleetinggleam.android.core.Models
import com.fleetinggleam.android.core.Project
import com.fleetinggleam.android.core.Search
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.components.Icons
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * 项目里的搜索（桌面端 Culling.tsx 的搜索部分）：索引在后台为这个项目的文件夹建好；打字停下 250 毫秒后才去搜。
 */
@Composable
fun SearchEffects(project: Project, cull: CullState, done: Boolean) {
    val dirs = if (done && project.search != false) project.folders else emptyList()
    LaunchedEffect(dirs, cull.searchRev) { Search.keep(dirs, cull.searchRev) }
    LaunchedEffect(cull.query, dirs) {
        val q = cull.query.trim()
        if (q.isEmpty() || dirs.isEmpty()) { cull.rawHits = null; return@LaunchedEffect }
        delay(250)
        cull.rawHits = runCatching { Search.photos(dirs, q) }.getOrDefault(emptyList())
    }
}

/** 第一次来搜时下载搜索要的模型（问一次），然后索引自己在后台建。 */
suspend fun ensureSearch(onReady: () -> Unit) {
    if (Search.index.value.available) return
    if (Models.ensurePacks(listOf("search", "standard")) != Answer.Cancelled) onReady()
}

/** 搜索框：索引还在建时图标外有一圈细环慢慢填满，搜到几张写在右边。 */
@Composable
fun SearchField(cull: CullState, modifier: Modifier = Modifier, placeholder: String = t("search.placeholder"), autoFocus: Boolean = true) {
    val c = LocalFg.current
    val index by Search.index.collectAsState()
    val scope = rememberCoroutineScope()
    val focus = remember { FocusRequester() }
    val fm = LocalFocusManager.current
    LaunchedEffect(Unit) { if (autoFocus) runCatching { focus.requestFocus() } }
    Row(
        modifier.fillMaxWidth().height(38.dp).clip(RoundedCornerShape(11.dp)).background(c.surface).padding(start = 8.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
            FgIcon(Icons.SEARCH, size = 15.dp, color = c.ash)
            index.progress?.let { p ->
                Canvas(Modifier.size(24.dp)) {
                    drawArc(c.ink, -90f, 360f * maxOf(0.04f, p), false, style = Stroke(1.6.dp.toPx(), cap = StrokeCap.Round))
                }
            }
        }
        Box(Modifier.weight(1f).padding(horizontal = 8.dp)) {
            if (cull.query.isEmpty()) Text(index.progress?.let { t("search.indexing", "pct" to Math.round(it * 100)) } ?: placeholder, style = Type.body.copy(color = c.ash), maxLines = 1)
            BasicTextField(
                cull.query, { cull.query = it }, singleLine = true, textStyle = Type.body.copy(color = c.ink), cursorBrush = SolidColor(c.ink),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search), keyboardActions = KeyboardActions(onSearch = { fm.clearFocus() }),
                modifier = Modifier.fillMaxWidth().focusRequester(focus).onFocusChanged { if (it.isFocused) scope.launch { ensureSearch { cull.searchRev++ } } },
            )
        }
        if (cull.query.isNotEmpty()) {
            cull.rawHits?.let { Text(t("search.results", "count" to it.size), style = Type.monoSmall.copy(color = c.ash)) }
            Box(Modifier.padding(start = 4.dp).size(32.dp).clip(CircleShape).clickable { cull.query = "" }, contentAlignment = Alignment.Center) {
                FgIcon("M4 4l8 8M12 4l-8 8", size = 11.dp, stroke = 1.8f, color = c.ash)
            }
        }
    }
}
