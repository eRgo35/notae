package com.c2yz.notae.ui.viewer

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Viewer
import com.c2yz.notae.platform.PageSource
import com.c2yz.notae.platform.PageSourceFactory
import com.c2yz.notae.platform.PdfPageSource
import com.c2yz.notae.ui.components.AnimatedOverlay
import com.c2yz.notae.ui.components.ClaimBack
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.components.LocalBackProgress
import com.c2yz.notae.ui.components.ModalSurface
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.components.backProgress
import com.c2yz.notae.ui.details.NoteBlock
import com.c2yz.notae.ui.library.NoteEditor
import com.c2yz.notae.ui.sets.closeOnEscape
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.draw.clip
import androidx.navigationevent.NavigationEventInfo
import androidx.navigationevent.compose.NavigationBackHandler
import androidx.navigationevent.compose.rememberNavigationEventState
import com.c2yz.notae.ui.components.windowHeightClass
import com.c2yz.notae.ui.components.WidthClass
import com.c2yz.notae.ui.components.windowWidthClass
import com.c2yz.notae.ui.theme.NotaeTheme
import kotlin.coroutines.cancellation.CancellationException

/**
 * Notae's viewer: a bar with Back, the score's title and the page over its pages ([PageArea]). It is drawn over the
 * screen that opened it; Back (the button, the gesture or Escape) closes it through the core. A tap in the middle of
 * the page hides or shows the bar, and the screen stays on while the viewer shows. [openPages] opens the score's
 * pages; by default with [PdfPageSource].
 */
@Composable
fun ViewerScreen(viewer: Viewer, onAction: (Action) -> Unit, openPages: PageSourceFactory? = null, stylusSeen: Boolean = false) {
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
    val view = LocalView.current
    DisposableEffect(view) {
        view.keepScreenOn = true
        onDispose { view.keepScreenOn = false }
    }
    val configuration = LocalConfiguration.current
    val mode = pageModeFor(configuration.screenWidthDp > configuration.screenHeightDp, windowWidthClass(), windowHeightClass())
    // The first page on screen, kept across a rotation that changes how pages pair.
    var page by remember(viewer.row.noteKey) { mutableIntStateOf(0) }
    var bars by remember { mutableStateOf(true) }
    var notes by remember(viewer.row.noteKey) { mutableStateOf(false) }
    // Drawing on a set's piece: the session while drawing mode is on, saved when it ends, on Previous/Next and on leaving.
    var session by remember(viewer.row.noteKey) { mutableStateOf<DrawSession?>(null) }
    var showDrawings by remember { mutableStateOf(true) }
    var clearing by remember(viewer.row.noteKey) { mutableStateOf(false) }
    var textRequest by remember(viewer.row.noteKey) { mutableStateOf<TextRequest?>(null) }
    var stylusReported by remember { mutableStateOf(stylusSeen) }
    val setId = viewer.setId
    val itemId = viewer.row.itemId
    val save = { drawn: DrawSession? ->
        if (drawn != null && drawn.changed && setId != null && itemId != null) onAction(Action.SaveOverlay(setId, itemId, drawn.overlay()))
    }
    val finishDrawing = {
        save(session)
        session = null
    }
    DisposableEffect(viewer.row.noteKey) {
        onDispose { save(session) }
    }
    var editing by remember(viewer.row.noteKey) { mutableStateOf<ViewerNote?>(null) }
    val backState = rememberNavigationEventState(NavigationEventInfo.None)
    NavigationBackHandler(
        state = backState,
        isBackEnabled = notes || editing != null || session != null,
        onBackCompleted = {
            when {
                editing != null -> editing = null
                notes -> notes = false
                else -> finishDrawing()
            }
        },
    )
    ClaimBack(notes || editing != null || session != null)
    val compact = windowWidthClass() == WidthClass.COMPACT
    val spreads = spreadsFor(source?.pageCount ?: 0, mode)
    // Previous and Next piece: in the top bar, or on a phone in a bar of their own under the pages.
    val pieceButtons = @Composable {
        IconButton(stringResource(R.string.viewer_previous_piece), painterResource(R.drawable.ic_chevron_left), {
            finishDrawing()
            onAction(Action.ViewerPrevious)
        }, enabled = viewer.hasPrevious)
        IconButton(stringResource(R.string.viewer_next_piece), painterResource(R.drawable.ic_chevron_right), {
            finishDrawing()
            onAction(Action.ViewerNext)
        }, enabled = viewer.hasNext)
    }
    Column(
        Modifier
            .fillMaxSize()
            .background(c.background)
            .onPreviewKeyEvent { event ->
                (event.type == KeyEventType.KeyDown && event.key == Key.Escape).also { if (it) onAction(Action.CloseViewer) }
            },
    ) {
        AnimatedVisibility(bars || error != null || session != null) {
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
                if (setId != null) {
                    if (session == null && viewer.overlay != null) {
                        IconButton(
                            stringResource(if (showDrawings) R.string.drawings_hide else R.string.drawings_show),
                            painterResource(if (showDrawings) R.drawable.ic_eye_off else R.drawable.ic_eye),
                            { showDrawings = !showDrawings },
                        )
                    }
                    IconButton(
                        stringResource(R.string.viewer_draw),
                        painterResource(R.drawable.ic_edit),
                        {
                            if (session != null) {
                                finishDrawing()
                            } else {
                                showDrawings = true
                                session = DrawSession(viewer.overlay, viewer.row.noteKey)
                            }
                        },
                        selected = session != null,
                    )
                    if (!compact) pieceButtons()
                }
                IconButton(stringResource(R.string.viewer_notes), painterResource(R.drawable.ic_note), { notes = true })
            }
        }
        val drawing = session
        if (drawing != null && !compact) DrawToolBar(drawing, windowWidthClass() != WidthClass.EXPANDED, { clearing = true }, finishDrawing)
        if (viewer.contentChanged) {
            Row(
                Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(painterResource(R.drawable.ic_warning), null, Modifier.size(20.dp), tint = c.error)
                Text(stringResource(R.string.viewer_content_changed), color = c.error, fontSize = 15.sp)
            }
        }
        Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
            error?.let { Text(stringResource(R.string.viewer_cannot_open, it), Modifier.padding(24.dp), color = c.error, fontSize = 15.sp) }
            source?.let { pages ->
                PageArea(
                    pages,
                    mode,
                    spreadOf(page, spreads),
                    { spread -> page = spreads.getOrNull(spread)?.first ?: page },
                    { bars = !bars },
                    Modifier.fillMaxSize(),
                ) { index, _ ->
                    PageDrawings(
                        index,
                        session?.overlay() ?: viewer.overlay,
                        session,
                        showDrawings,
                        stylusSeen || stylusReported,
                        {
                            if (!stylusSeen && !stylusReported) {
                                stylusReported = true
                                onAction(Action.StylusSeen)
                            }
                        },
                        { textRequest = it },
                    )
                }
            }
            if (viewer.showHint) ViewerHint({ onAction(Action.ViewerHintSeen) }, Modifier.align(Alignment.BottomCenter))
        }
        if (compact && setId != null) {
            AnimatedVisibility(bars || session != null) {
                Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp), horizontalArrangement = Arrangement.SpaceBetween) { pieceButtons() }
            }
        }
        if (drawing != null && compact) DrawToolBar(drawing, windowWidthClass() != WidthClass.EXPANDED, { clearing = true }, finishDrawing)
    }
        if (clearing) {
            ConfirmDrawingDialog(stringResource(R.string.draw_clear_question), stringResource(R.string.draw_clear), { clearing = false }) {
                clearing = false
                // Every page on screen: one, or the two of a landscape spread.
                spreads.getOrNull(spreadOf(page, spreads))?.forEach { session?.clearPage(it) }
            }
        }
    when (val request = textRequest) {
        is TextRequest.Add -> AddTextDialog({ textRequest = null }) { text ->
            textRequest = null
            session?.addText(request.page, request.x, request.y, text)
        }
        is TextRequest.Delete -> ConfirmDrawingDialog(
            stringResource(R.string.draw_delete_text_question),
            stringResource(R.string.draw_delete),
            { textRequest = null },
        ) {
            textRequest = null
            session?.deleteText(request.page, request.index)
        }
        null -> Unit
    }
    CompositionLocalProvider(LocalBackProgress provides remember(backState) { backProgress(backState) }) {
        AnimatedOverlay(if (notes && editing == null) viewer else null) {
            NotesSheet(it, { notes = false }, { note -> editing = note })
        }
        AnimatedOverlay(editing) { note ->
            val close = { editing = null }
            val row = viewer.row
            Box(Modifier.closeOnEscape(close)) {
                when (note) {
                    ViewerNote.SCORE -> NoteEditor(stringResource(R.string.score_note), row.scoreNote.orEmpty(), close, allowBlank = true) { text ->
                        close()
                        onAction(Action.SaveScoreNote(row.noteKey, text))
                    }
                    ViewerNote.SET -> NoteEditor(stringResource(R.string.set_note_in, viewer.setName.orEmpty()), row.setNote.orEmpty(), close, allowBlank = true) { text ->
                        close()
                        row.itemId?.let { onAction(Action.SaveSetNote(it, text)) }
                    }
                }
            }
        }
    }
}

/** Which note the viewer's editor is open on. */
private enum class ViewerNote { SCORE, SET }

/** The score note, then (from a set) the set note labelled with the set's name; a tap opens a note's editor. */
@Composable
private fun NotesSheet(viewer: Viewer, onClose: () -> Unit, onEdit: (ViewerNote) -> Unit) {
    val c = NotaeTheme.colors
    ModalSurface(stringResource(R.string.viewer_notes), onDismiss = onClose, bottomSheet = true, cardWidth = 560.dp, modifier = Modifier.closeOnEscape(onClose)) {
        Text(stringResource(R.string.viewer_notes), color = c.foreground, fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
        Column {
            NoteBlock(stringResource(R.string.score_note), viewer.row.scoreNote, stringResource(R.string.add_score_note)) { onEdit(ViewerNote.SCORE) }
            viewer.setName?.takeIf { viewer.row.itemId != null }?.let { name ->
                NoteBlock(stringResource(R.string.set_note_in, name), viewer.row.setNote, stringResource(R.string.add_set_note)) { onEdit(ViewerNote.SET) }
            }
        }
    }
}

/** "Scores now open in Notae. Change it in Settings." with OK, once after updating. */
@Composable
private fun ViewerHint(onDismiss: () -> Unit, modifier: Modifier) {
    val c = NotaeTheme.colors
    Row(
        modifier
            .padding(16.dp)
            .widthIn(max = 560.dp)
            .clip(RoundedCornerShape(20.dp))
            .background(c.card)
            .padding(start = 20.dp, end = 8.dp, top = 8.dp, bottom = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(stringResource(R.string.viewer_hint), Modifier.weight(1f, fill = false), color = c.foreground, fontSize = 15.sp)
        PillButton(stringResource(R.string.viewer_hint_ok), onDismiss, tonal = true)
    }
}
