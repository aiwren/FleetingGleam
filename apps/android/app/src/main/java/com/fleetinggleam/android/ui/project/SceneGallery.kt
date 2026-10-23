package com.fleetinggleam.android.ui.project

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.foundation.layout.Column
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import com.fleetinggleam.android.core.Photo
import com.fleetinggleam.android.core.Progress
import com.fleetinggleam.android.core.Project
import com.fleetinggleam.android.core.Scenes
import com.fleetinggleam.android.core.Session
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgButton
import com.fleetinggleam.android.ui.components.riseIn
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import java.io.File

/**
 * 全部场景（桌面端 SceneGallery）：每个场景大到认得出来——最好的一张大图，下面一排同场景的几张。大的场景在前。
 * 点一个就把照片墙筛到它、回到照片。
 */
@Composable
fun SceneGallery(project: Project, data: CullData, cull: CullState, describing: Progress?, canDescribe: Boolean, onDescribe: () -> Unit, topPad: Dp, bottomPad: Dp) {
    val c = LocalFg.current
    // 导航在项目页的玻璃顶栏里（‹ 项目名 · 场景，设计审查 A11.3）；这里只有网格
    Column(Modifier.fillMaxSize()) {
    val all by Session.photos.collectAsState()
    val decisions by Session.decisions.collectAsState()
    val texts by Scenes.texts.collectAsState()
    // 每个场景里符合状态与人物筛选的照片，分高的在前
    val groups = remember(all, decisions, cull.mode, cull.personFilter, texts) {
        all.filter { p -> (cull.mode == "all" || decisions[p.path]?.state == cull.mode) && (cull.personFilter == null || p.people.contains(cull.personFilter)) }
            .groupBy { it.scene }.map { (id, ps) -> Triple(id, ps.sortedByDescending { it.score }, texts[id] ?: "") }.sortedByDescending { it.second.size }
    }
    LazyVerticalGrid(GridCells.Fixed(2), contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = topPad + 8.dp, bottom = bottomPad), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(16.dp), modifier = Modifier.fillMaxSize()) {
        itemsIndexed(groups, key = { _, g -> g.first }) { i, (id, photos, text) ->
            val on = cull.sceneFilter == id
            // 卡只一层圆角 10；选中 = 画在外侧 2dp 的墨色描边，不嵌套（A11.1）
            Column(Modifier.riseIn(i).then(if (on) Modifier.drawBehind {
                val o = 2.dp.toPx()
                drawRoundRect(c.ink, topLeft = androidx.compose.ui.geometry.Offset(-o * 2, -o * 2), size = androidx.compose.ui.geometry.Size(size.width + o * 4, size.height + o * 4), cornerRadius = androidx.compose.ui.geometry.CornerRadius(12.dp.toPx()), style = androidx.compose.ui.graphics.drawscope.Stroke(o))
            } else Modifier).clickable { cull.sceneFilter = id; cull.sceneView = false }) {
                Box(Modifier.fillMaxWidth().aspectRatio(1.25f).clip(RoundedCornerShape(10.dp))) {
                    AsyncImage(File(photos[0].thumb), null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
                    Box(Modifier.fillMaxSize().background(Brush.verticalGradient(0.5f to Color.Transparent, 1f to Color(0xB0000000))))
                    if (text.isNotEmpty()) Text(text, style = Type.rowDesc.copy(color = Color.White), maxLines = 2, overflow = Ellipsis, modifier = Modifier.align(Alignment.BottomStart).padding(start = 10.dp, end = 10.dp, bottom = 26.dp))
                    Text(t("cull.sceneCount", "count" to photos.size), style = Type.monoSmall.copy(color = Color.White.copy(alpha = 0.85f)), modifier = Modifier.align(Alignment.BottomStart).padding(10.dp))
                }
                if (photos.size > 1) Row(Modifier.padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    photos.drop(1).take(4).forEach { p ->
                        AsyncImage(File(p.thumb), null, Modifier.weight(1f).height(38.dp).clip(RoundedCornerShape(6.dp)), contentScale = ContentScale.Crop)
                    }
                    repeat(maxOf(0, 4 - (photos.size - 1))) { Box(Modifier.weight(1f)) }
                }
            }
        }
    }
    }
}
