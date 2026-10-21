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
    fun openAlbums(more: Boolean = false) {
        source = "albums"; busy = true; noPermission = false
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
        Column(Modifier.navigationBarsPadding().padding(horizontal = 20.dp).padding(top = 20.dp, bottom = 16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (source != null) Box(Modifier.size(36.dp).clip(RoundedCornerShape(10.dp)).clickable { source = null; listing = null; err = null }, contentAlignment = Alignment.Center) { FgIcon(Icons.BACK, size = 16.dp, box = 12f, stroke = 1.6f) }
                Text(share?.name ?: if (source == "albums") t("android.picker.albums") else t("picker.title"), style = Type.heading.copy(color = c.ink), modifier = Modifier.padding(start = if (source != null) 4.dp else 0.dp))
            }
            Spacer(Modifier.size(14.dp))
            when {
                source == null -> Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Source(Icons.ALBUM, t("android.picker.albums"), t("android.picker.albumsDesc")) { openAlbums() }
                    Source(Icons.FOLDER, t("android.picker.folder"), t("android.picker.folderDesc")) { pickSystemFolder() }
                    shares.filter { it.enabled }.forEach { s -> Source(Icons.NAS, s.name, s.host + if (s.connected) "" else " · ${t("settings.nasOffline")}") { enter(s, "") } }
                }
                source == "albums" -> Albums(albums, busy, noPermission, onRetry = { if (Pickers.mediaBlocked) Pickers.openAppSettings(ctx) else openAlbums() }, onMore = { openAlbums(more = true) }) { Nas.answer(it) }
                share != null -> Browse(share, listing, busy, err, onEnter = { path -> enter(share, path) }, onChoose = { Nas.answer(it) })
            }
            Row(Modifier.fillMaxWidth().padding(top = 16.dp), horizontalArrangement = Arrangement.End) { FgButton(t("picker.cancel"), { Nas.answer(null) }, small = true) }
        }
    }
}

@Composable
private fun Source(icon: String, title: String, desc: String, onClick: () -> Unit) {
    val c = LocalFg.current
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(c.panel).border(1.dp, c.line, RoundedCornerShape(14.dp)).clickable(onClick = onClick).padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        FgIcon(icon, size = 20.dp)
        Column(Modifier.weight(1f).padding(horizontal = 14.dp)) {
            Text(title, style = Type.bodyStrong.copy(color = c.ink), maxLines = 1, overflow = Ellipsis)
            Text(desc, style = Type.small.copy(color = c.ash), maxLines = 2, overflow = Ellipsis)
        }
        FgIcon(Icons.CHEVRON, size = 16.dp, color = c.ash)
    }
}

@Composable
private fun Albums(albums: List<Media.Album>?, busy: Boolean, noPermission: Boolean, onRetry: () -> Unit, onMore: () -> Unit, onPick: (String) -> Unit) {
    val c = LocalFg.current
    val ctx = LocalContext.current
    when {
        noPermission -> Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            val blocked = Pickers.mediaBlocked
            Text(t(if (blocked) "android.picker.permissionBlocked" else "android.picker.permissionNeeded"), style = Type.body.copy(color = c.ash))
            FgButton(t(if (blocked) "android.picker.openSettings" else "android.picker.grant"), onRetry, primary = true)
        }
        busy || albums == null -> Text(t("picker.loading"), style = Type.small.copy(color = c.ash))
        albums.isEmpty() -> Text(t("android.picker.noAlbums"), style = Type.body.copy(color = c.ash))
        else -> Column {
            if (Media.partialOnly(ctx)) Row(Modifier.fillMaxWidth().padding(bottom = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(t("android.picker.partial"), style = Type.small.copy(color = c.ash), modifier = Modifier.weight(1f).padding(end = 12.dp))
                FgButton(t("android.picker.selectMore"), onMore, small = true)
            }
            LazyColumn(Modifier.heightIn(max = 460.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                items(albums, key = { it.dir }) { a ->
                    Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).clickable { onPick(a.dir) }.padding(6.dp), verticalAlignment = Alignment.CenterVertically) {
                        AsyncImage(a.cover, null, Modifier.size(52.dp).clip(RoundedCornerShape(8.dp)).background(c.surface), contentScale = ContentScale.Crop)
                        Column(Modifier.weight(1f).padding(horizontal = 12.dp)) {
                            Text(a.name, style = Type.bodyStrong.copy(color = c.ink), maxLines = 1, overflow = Ellipsis)
                            Text(a.dir, style = Type.monoSmall.copy(color = c.ash), maxLines = 1, overflow = Ellipsis)
                        }
                        Text(t("android.picker.albumCount", "count" to a.count), style = Type.mono.copy(color = c.ash))
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
