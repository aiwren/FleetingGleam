package com.fleetinggleam.android.ui.picker

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
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
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Brush
import androidx.compose.foundation.lazy.itemsIndexed
import coil3.compose.AsyncImage
import com.fleetinggleam.android.core.Listing
import com.fleetinggleam.android.core.Media
import com.fleetinggleam.android.core.Nas
import com.fleetinggleam.android.core.Pickers
import com.fleetinggleam.android.core.Share
import com.fleetinggleam.android.core.fmtBytes
import com.fleetinggleam.android.core.t
import com.fleetinggleam.android.ui.components.FgButton
import com.fleetinggleam.android.ui.components.FgIcon
import com.fleetinggleam.android.ui.components.Icons
import com.fleetinggleam.android.ui.theme.Ellipsis
import com.fleetinggleam.android.ui.theme.LocalFg
import com.fleetinggleam.android.ui.theme.Type
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * 问"项目文件夹在哪里"（桌面端 FolderPicker.tsx 的手机版）：手机相册里的某个相册、系统文件选择器里的任意文件夹，或某台 NAS 的共享。
 * 谁调 `Nas.pick()` 它就出现；回答的是一个本地路径。NAS 可以逐级进入，每一级都能看到有多少张照片。
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun FolderPicker() {
    val c = LocalFg.current
    val ctx = LocalContext.current
    val req by Nas.request.collectAsState()
    val shares by Nas.shares.collectAsState()
    val scope = rememberCoroutineScope()
    if (req == null) return
    var source by remember(req) { mutableStateOf<String?>(null) } // null | "albums" | share id
    var albums by remember(req) { mutableStateOf<List<Media.Album>?>(null) }
    var listing by remember(req) { mutableStateOf<Listing?>(null) }
    var busy by remember(req) { mutableStateOf(false) }
    var err by remember(req) { mutableStateOf<String?>(null) }
    var noPermission by remember(req) { mutableStateOf(false) }
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    fun enter(s: Share, path: String) {
        source = s.id; busy = true; err = null
        scope.launch {
            try {
                if (!s.connected) Nas.connect(s.id)
                listing = Nas.browse(s.id, path)
            } catch (e: Exception) {
                err = e.message
                if (listing == null) listing = Listing(path, "", emptyList())
            }
            busy = false
        }
    }
    // 没有权限时先显示说明页（为什么要、给了以后会怎样），点"继续"才弹系统框；有权限就直接列相册
    fun openAlbums(more: Boolean = false, ask: Boolean = false) {
        source = "albums"; noPermission = false
        if (!ask && !more && !(Media.hasPermission(ctx) || Media.partialOnly(ctx))) { noPermission = true; return }
        busy = true
        scope.launch {
            if (!Pickers.mediaPermission(ctx, more)) { noPermission = true; busy = false; return@launch }
            albums = withContext(Dispatchers.IO) { runCatching { Media.albums(ctx) }.getOrDefault(emptyList()) }
            busy = false
        }
    }
    // 从系统设置回来（用户可能刚打开了权限，或改了"部分照片"的范围）：重新读一遍相册
    androidx.lifecycle.compose.LifecycleResumeEffect(source, noPermission) {
        if (source == "albums" && !busy && (noPermission && (Media.hasPermission(ctx) || Media.partialOnly(ctx)) || Media.partialOnly(ctx))) openAlbums()
        onPauseOrDispose { }
    }
    fun pickSystemFolder() {
        scope.launch {
            val uri = Pickers.openTree() ?: return@launch
            val path = Media.treeToPath(uri)
            // 手机存储或 SD 卡上的文件夹：有读照片的权限就按路径直接读；读不到（网盘、没给权限）就复制进应用自己的镜像
            if (path != null && (Pickers.mediaPermission(ctx) && Media.readable(path))) Nas.answer(path)
            else Nas.answer(withContext(Dispatchers.IO) { Media.adoptTree(ctx, uri) })
        }
    }
    // 设置页"从这里新建项目"：直接进到那个共享
    LaunchedEffect(req) { req?.shareId?.let { id -> shares.firstOrNull { it.id == id }?.let { enter(it, "") } } }

    val share = shares.firstOrNull { it.id == source }
    ModalBottomSheet(onDismissRequest = { Nas.answer(null) }, sheetState = sheet, containerColor = c.canvas, dragHandle = null) {
        val explaining = source == "albums" && noPermission
        Column(Modifier.navigationBarsPadding().padding(horizontal = 16.dp).padding(top = 8.dp, bottom = 12.dp)) {
            // 顶栏：‹ 返回上一层 · 标题 · 取消
            Box(Modifier.fillMaxWidth().height(44.dp)) {
                if (source != null) Row(Modifier.align(Alignment.CenterStart).clip(CircleShape).clickable { source = null; listing = null; err = null; noPermission = false }.padding(start = 2.dp, end = 10.dp, top = 8.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    FgIcon(Icons.BACK, size = 18.dp, box = 12f, stroke = 1.5f)
                }
                if (!explaining) Text(share?.name ?: if (source == "albums") t("android.picker.albums") else t("picker.title"), style = Type.navTitle.copy(color = c.ink), maxLines = 1, overflow = Ellipsis, modifier = Modifier.align(Alignment.Center).padding(horizontal = 72.dp))
                Text(t("picker.cancel"), style = Type.rowTitle.copy(color = c.ink), modifier = Modifier.align(Alignment.CenterEnd).clip(CircleShape).clickable { Nas.answer(null) }.padding(horizontal = 10.dp, vertical = 8.dp))
            }
            Spacer(Modifier.size(6.dp))
            when {
                source == null -> Column(Modifier.clip(RoundedCornerShape(14.dp)).background(c.panel)) {
                    Source(Icons.ALBUM, t("android.picker.albums"), t("android.picker.albumsDesc"), first = true) { openAlbums() }
                    Source(Icons.FOLDER, t("android.picker.folder"), t("android.picker.folderDesc")) { pickSystemFolder() }
                    shares.filter { it.enabled }.forEach { s -> Source(Icons.NAS, s.name, s.host + if (s.connected) "" else " · ${t("settings.nasOffline")}") { enter(s, "") } }
                }
                explaining -> PermissionExplainer(
                    blocked = Pickers.mediaBlocked,
                    onContinue = { if (Pickers.mediaBlocked) Pickers.openAppSettings(ctx) else openAlbums(ask = true) },
                    onFolderInstead = { pickSystemFolder() },
                )
                source == "albums" -> Albums(albums, busy, onMore = { openAlbums(more = true) }) { Nas.answer(it) }
                share != null -> Browse(share, listing, busy, err, onEnter = { path -> enter(share, path) }, onChoose = { Nas.answer(it) })
            }
        }
    }
}

@Composable
private fun Source(icon: String, title: String, desc: String, first: Boolean = false, onClick: () -> Unit) {
    val c = LocalFg.current
    Column {
        if (!first) Box(Modifier.padding(start = 56.dp).fillMaxWidth().height(0.5.dp).background(c.line))
        Row(Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(28.dp).clip(RoundedCornerShape(8.dp)).background(c.ink), contentAlignment = Alignment.Center) { FgIcon(icon, size = 16.dp, stroke = 1.5f, color = c.onInk) }
            Column(Modifier.weight(1f).padding(horizontal = 12.dp)) {
                Text(title, style = Type.rowTitle.copy(color = c.ink), maxLines = 1, overflow = Ellipsis)
                Text(desc, style = Type.rowDesc.copy(color = c.ash), maxLines = 2, overflow = Ellipsis)
            }
            FgIcon(Icons.CHEVRON, size = 14.dp, stroke = 1.6f, color = c.ash)
        }
    }
}

/**
 * 授权说明页（作者 2026-10-21："授权界面好丑"）：一叠照片的插图、一句标题、三条承诺、整行的"继续"；
 * 下面一个文字按钮"改为选一个文件夹"。被永久拒绝时标题和按钮换成去系统设置的说法。
 */
@Composable
private fun PermissionExplainer(blocked: Boolean, onContinue: () -> Unit, onFolderInstead: () -> Unit) {
    val c = LocalFg.current
    Column(Modifier.fillMaxWidth().padding(top = 4.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        PhotoStack()
        Text(t(if (blocked) "android.picker.permissionBlockedTitle" else "android.picker.permissionTitle"), style = Type.title.copy(color = c.ink), textAlign = androidx.compose.ui.text.style.TextAlign.Center, modifier = Modifier.padding(top = 22.dp))
        Text(t(if (blocked) "android.picker.permissionBlocked" else "android.picker.permissionLead"), style = Type.body.copy(color = c.ash), textAlign = androidx.compose.ui.text.style.TextAlign.Center, modifier = Modifier.padding(top = 8.dp, start = 12.dp, end = 12.dp))
        if (!blocked) Column(Modifier.fillMaxWidth().padding(top = 20.dp).clip(RoundedCornerShape(14.dp)).background(c.panel)) {
            listOf(
                Triple("M8 2.2L9.6 5.6l3.7.4-2.8 2.5.8 3.6L8 10.3l-3.3 1.8.8-3.6L2.7 6l3.7-.4z", "android.picker.promise1", true),
                Triple("M3 4.5A1.5 1.5 0 014.5 3h7A1.5 1.5 0 0113 4.5v7a1.5 1.5 0 01-1.5 1.5h-7A1.5 1.5 0 013 11.5zM5.5 10l2-2.5 1.5 1.8 1-1.2 1.5 1.9", "android.picker.promise2", false),
                Triple("M2.5 8a5.5 5.5 0 1111 0 5.5 5.5 0 01-11 0zM8 5v3l2 1.5", "android.picker.promise3", false),
            ).forEach { (icon, key, first) ->
                if (!first) Box(Modifier.padding(start = 52.dp).fillMaxWidth().height(0.5.dp).background(c.line))
                Row(Modifier.padding(horizontal = 14.dp, vertical = 11.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(26.dp).clip(RoundedCornerShape(7.dp)).background(c.surface2), contentAlignment = Alignment.Center) { FgIcon(icon, size = 15.dp, stroke = 1.4f, color = c.ink) }
                    Text(t(key), style = Type.rowDesc.copy(color = c.ink, fontSize = androidx.compose.ui.unit.TextUnit(13f, androidx.compose.ui.unit.TextUnitType.Sp)), modifier = Modifier.padding(start = 12.dp))
                }
            }
        }
        FgButton(t(if (blocked) "android.picker.openSettings" else "android.picker.continue"), onContinue, primary = true, small = false, modifier = Modifier.fillMaxWidth().padding(top = 20.dp))
        Text(t("android.picker.folderInstead"), style = Type.label.copy(color = c.ash), modifier = Modifier.padding(top = 6.dp).clip(CircleShape).clickable(onClick = onFolderInstead).padding(horizontal = 14.dp, vertical = 10.dp))
    }
}

/** 一叠斜着的照片（三张有渐变的小卡），右下角一粒金色的四角星——应用图标的意象。 */
@Composable
private fun PhotoStack() {
    val c = LocalFg.current
    Box(Modifier.size(132.dp, 104.dp), contentAlignment = Alignment.Center) {
        listOf(Triple(-14f, 10.dp, 0.55f), Triple(8f, 4.dp, 0.75f), Triple(-2f, 0.dp, 1f)).forEachIndexed { i, (rot, dy, a) ->
            Box(
                Modifier.offset(y = dy).size(96.dp, 72.dp).graphicsLayer { rotationZ = rot; alpha = a }
                    .shadow(if (i == 2) 10.dp else 4.dp, RoundedCornerShape(10.dp), ambientColor = Color.Black.copy(alpha = 0.25f), spotColor = Color.Black.copy(alpha = 0.3f))
                    .clip(RoundedCornerShape(10.dp))
                    .background(Brush.linearGradient(listOf(Color(0xFFBFD6EA), Color(0xFF7FA7CB), Color(0xFFE8C98A)))),
            ) {
                // 一点地平线和太阳，像张风景
                Box(Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(26.dp).background(Brush.verticalGradient(listOf(Color.Transparent, Color(0x667A9A4C), Color(0xCC5B7E3A)))))
                Box(Modifier.align(Alignment.TopEnd).padding(top = 12.dp, end = 16.dp).size(14.dp).clip(CircleShape).background(Color(0xFFFFF1C8)))
            }
        }
        Box(Modifier.align(Alignment.BottomEnd).padding(end = 6.dp, bottom = 2.dp)) { com.fleetinggleam.android.ui.project.Star(true, 30.dp, color = c.gleam) }
    }
}

@Composable
private fun Albums(albums: List<Media.Album>?, busy: Boolean, onMore: () -> Unit, onPick: (String) -> Unit) {
    val c = LocalFg.current
    val ctx = LocalContext.current
    when {
        busy || albums == null -> Text(t("picker.loading"), style = Type.small.copy(color = c.ash), modifier = Modifier.padding(16.dp))
        albums.isEmpty() -> Text(t("android.picker.noAlbums"), style = Type.body.copy(color = c.ash), modifier = Modifier.padding(16.dp))
        else -> Column {
            if (Media.partialOnly(ctx)) Row(Modifier.fillMaxWidth().padding(bottom = 10.dp).clip(RoundedCornerShape(12.dp)).background(c.panel).padding(start = 14.dp, end = 8.dp, top = 8.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(t("android.picker.partial"), style = Type.rowDesc.copy(color = c.ash), modifier = Modifier.weight(1f).padding(end = 10.dp))
                FgButton(t("android.picker.selectMore"), onMore, small = true)
            }
            LazyColumn(Modifier.heightIn(max = 480.dp).clip(RoundedCornerShape(14.dp)).background(c.panel)) {
                itemsIndexed(albums, key = { _, a -> a.dir }) { i, a ->
                    if (i > 0) Box(Modifier.padding(start = 72.dp).fillMaxWidth().height(0.5.dp).background(c.line))
                    Row(Modifier.fillMaxWidth().clickable { onPick(a.dir) }.padding(horizontal = 12.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        AsyncImage(a.cover, null, Modifier.size(48.dp).clip(RoundedCornerShape(8.dp)).background(c.surface), contentScale = ContentScale.Crop)
                        Column(Modifier.weight(1f).padding(horizontal = 12.dp)) {
                            Text(a.name, style = Type.rowTitle.copy(color = c.ink), maxLines = 1, overflow = Ellipsis)
                            Text(a.dir.removePrefix("/storage/emulated/0/"), style = Type.monoSmall.copy(color = c.ash), maxLines = 1, overflow = Ellipsis)
                        }
                        Text(t("android.picker.albumCount", "count" to a.count), style = Type.mono.copy(color = c.ash))
                        FgIcon(Icons.CHEVRON, size = 14.dp, stroke = 1.6f, color = c.ash, modifier = Modifier.padding(start = 6.dp))
                    }
                }
            }
        }
    }
}

@Composable
private fun Browse(share: Share, listing: Listing?, busy: Boolean, err: String?, onEnter: (String) -> Unit, onChoose: (String) -> Unit) {
    val c = LocalFg.current
    val crumbs = listing?.path?.split('/')?.filter { it.isNotEmpty() } ?: emptyList()
    val atRoot = crumbs.isEmpty()
    val photos = listing?.entries?.filter { !it.dir } ?: emptyList()
    val folders = listing?.entries?.filter { it.dir } ?: emptyList()
    Column {
        Row(Modifier.horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically) {
            Crumb(share.name, atRoot) { onEnter("") }
            crumbs.forEachIndexed { i, s -> Text("/", style = Type.small.copy(color = c.ash)); Crumb(s, i == crumbs.size - 1) { onEnter(crumbs.take(i + 1).joinToString("/")) } }
        }
        if (err != null) Text(t("settings.nasFailed", "error" to err), style = Type.small.copy(color = c.ink), modifier = Modifier.padding(top = 8.dp))
        if (busy) Text(t("picker.loading"), style = Type.small.copy(color = c.ash), modifier = Modifier.padding(top = 8.dp))
        if (!busy && listing != null) {
            LazyColumn(Modifier.heightIn(max = 380.dp).padding(top = 8.dp)) {
                items(folders, key = { it.name }) { e ->
                    Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).clickable { onEnter(if (listing.path.isEmpty()) e.name else "${listing.path}/${e.name}") }.padding(horizontal = 6.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                        FgIcon(if (atRoot) Icons.SHARE else Icons.FOLDER, size = 18.dp)
                        Text(e.name, style = Type.body.copy(color = c.ink), modifier = Modifier.weight(1f).padding(horizontal = 12.dp), maxLines = 1, overflow = Ellipsis)
                        FgIcon(Icons.CHEVRON, size = 14.dp, color = c.ash)
                    }
                }
                if (folders.isEmpty()) item { Text(t(if (atRoot) "picker.noShares" else "picker.noFolders"), style = Type.small.copy(color = c.ash), modifier = Modifier.padding(8.dp)) }
            }
            Text(
                if (atRoot) t("picker.rootHint") else if (photos.isNotEmpty()) t("picker.photosHere", "count" to photos.size, "size" to fmtBytes(photos.sumOf { it.size })) else t("picker.noPhotosHere"),
                style = Type.small.copy(color = c.ash), modifier = Modifier.padding(top = 10.dp),
            )
            if (listing.local.isNotEmpty()) FgButton(t("picker.choose"), { onChoose(listing.local) }, primary = true, modifier = Modifier.padding(top = 12.dp).fillMaxWidth())
        }
    }
}

@Composable
private fun Crumb(text: String, current: Boolean, onClick: () -> Unit) {
    val c = LocalFg.current
    Text(text, style = Type.small.copy(color = if (current) c.ink else c.ash), modifier = Modifier.clip(RoundedCornerShape(6.dp)).clickable(onClick = onClick).padding(horizontal = 6.dp, vertical = 4.dp), maxLines = 1)
}
