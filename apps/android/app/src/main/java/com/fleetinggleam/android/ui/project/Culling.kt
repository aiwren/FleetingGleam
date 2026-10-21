package com.fleetinggleam.android.ui.project

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.unit.dp
import com.fleetinggleam.android.core.Photo
import com.fleetinggleam.android.core.Project
import com.fleetinggleam.android.core.Session

/** 照片按项目的排序方式排好（和桌面端一样：按相似分组时，组保持在文件夹里的位置，组里最好的在前）。 */
fun ordered(photos: List<Photo>, sortBy: String): List<Photo> = when (sortBy) {
    "score" -> photos.sortedByDescending { it.score }
    "time" -> photos.sortedBy { it.takenAt ?: 0L }
    "group" -> {
        val first = HashMap<Int, Int>()
        photos.forEachIndexed { i, p -> first.putIfAbsent(p.group, i) }
        photos.sortedWith(compareBy<Photo> { first[it.group] }.thenByDescending { it.score })
    }
    else -> photos
}

/** 分析好的项目：照片墙，点一张选中（对焦框和小坞），小坞上留下 / 淘汰 / 放大。 */
@Composable
fun Culling(project: Project, onOpenStyles: () -> Unit) {
    val photos by Session.photos.collectAsState()
    val decisions by Session.decisions.collectAsState()
    val celebrate by Session.celebrate.collectAsState()
    val sortBy = project.sort ?: "group"
    val shown = remember(photos, sortBy) { ordered(photos, sortBy) }
    var selected by rememberSaveable(project.id) { mutableStateOf<String?>(null) }
    val items = remember(shown) { shown.map { WallItem(it.path, it.ratio) } }
    val state = rememberLazyListState()
    Wall(items, "justified", state, PaddingValues(start = 12.dp, end = 12.dp, top = 12.dp, bottom = 40.dp)) { i, w, h ->
        val p = shown[i]
        val d = decisions[p.path]
        val st = d?.state ?: ""
        Tile(p.path, w, h, p, null, st, selected = selected == p.path, reveal = celebrate, dim = st == "reject", manual = d?.manual == true, showScore = sortBy == "score", index = i, interactive = true,
            onSelect = { selected = it }, onOpen = { selected = it }, onDecide = { path, s -> Session.decide(path, s) })
    }
}
