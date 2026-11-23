package com.c2yz.notae.ui.viewer

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import com.c2yz.notae.platform.PageSource
import com.c2yz.notae.platform.PageSourceFactory
import com.c2yz.notae.platform.PdfPageSource
import kotlin.coroutines.cancellation.CancellationException
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Viewer
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * Notae's viewer: a bar with Back and the score's title over its pages. It is drawn over the screen that opened it;
 * Back (the button, the gesture or Escape) closes it through the core.
 */
@Composable
fun ViewerScreen(viewer: Viewer, onAction: (Action) -> Unit, openPages: PageSourceFactory? = null) {
    val c = NotaeTheme.colors
    val resolver = LocalContext.current.contentResolver
    val open = openPages ?: { treeUri, path -> PdfPageSource.open(resolver, treeUri, path) }
    var source by remember(viewer.row.noteKey) { mutableStateOf<PageSource?>(null) }
    var error by remember(viewer.row.noteKey) { mutableStateOf<String?>(null) }
    LaunchedEffect(viewer.row.noteKey) {
        try {
            source = open(viewer.uri, viewer.row.relativePath)
        } catch (failure: CancellationException) {
            throw failure
        } catch (failure: Exception) {
            error = failure.message ?: failure.toString()
        }
    }
    DisposableEffect(source) {
        val opened = source
        onDispose { opened?.close() }
    }
    val page by remember(viewer.row.noteKey) { mutableIntStateOf(0) }
    Column(Modifier.fillMaxSize().background(c.background)) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(stringResource(R.string.viewer_back), painterResource(R.drawable.ic_arrow_back), { onAction(Action.CloseViewer) })
            Text(
                viewer.row.title,
                Modifier.weight(1f),
                color = c.foreground,
                fontSize = 18.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            source?.let { Text(stringResource(R.string.viewer_page_of, page + 1, it.pageCount), color = c.muted, fontSize = 15.sp) }
        }
        Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
            error?.let { Text(stringResource(R.string.viewer_cannot_open, it), Modifier.padding(24.dp), color = c.error, fontSize = 15.sp) }
            source?.let { SinglePage(it, page) }
        }
    }
}

/** Page [index] fit to the space, until the page area arrives. */
@Composable
private fun SinglePage(source: PageSource, index: Int) {
    BoxWithConstraints(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        val widthPx = constraints.maxWidth.coerceAtLeast(1)
        val bitmap by produceState<ImageBitmap?>(null, source, index, widthPx) { value = source.render(index, widthPx) }
        bitmap?.let { Image(it, null, Modifier.fillMaxSize(), contentScale = ContentScale.Fit) }
    }
}
