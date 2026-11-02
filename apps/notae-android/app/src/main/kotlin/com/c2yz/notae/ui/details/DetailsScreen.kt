package com.c2yz.notae.ui.details

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.Dp
import com.c2yz.notae.ui.theme.NotaeMotion
import com.c2yz.notae.ui.theme.Tokens
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Origin
import com.c2yz.notae.core.Row as ScoreRowData
import com.c2yz.notae.core.RowStatus
import com.c2yz.notae.core.Selection
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.components.WidthClass
import com.c2yz.notae.ui.components.windowWidthClass
import com.c2yz.notae.ui.components.NotaeCard
import com.c2yz.notae.ui.components.OperationBanner
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.library.PagePreview
import com.c2yz.notae.ui.library.isCtrlK
import com.c2yz.notae.ui.library.pdfErrorText
import com.c2yz.notae.ui.sets.overlayOpen
import com.c2yz.notae.ui.theme.NotaeTheme

internal const val DETAILS_BODY_TAG = "details-body"

/**
 * 0.3.0's `ScoreDetails`: full screen on a narrow window ([CompactDetails] on a phone), and the detail pane beside the library or a set on a wide one
 * ([besideList]). Alone, the operation banner and the PDF launch error stay above the card; beside a list, the list's
 * pane shows them. The header has back, "X of Y", the star, previous and next. The body scrolls and puts the page
 * beside the headline and the info card when the card is at least 720 dp wide; under that, it stacks them centred.
 * Neighbours and the position are the core's ([Selection]): each is a selection id, so every copy of a repeated piece
 * in a set is its own step. The screen takes focus, so Left and Right move to them and Escape goes back without a tap
 * first.
 */
@Composable
fun DetailsScreen(snapshot: Snapshot, onAction: (Action) -> Unit, modifier: Modifier = Modifier, besideList: Boolean = false) {
    val selection = snapshot.selection ?: return
    val c = NotaeTheme.colors
    // 0.3.0 focused its shortcut scope when the details open and when a dialog closes, so the keys work at once.
    val keys = remember { FocusRequester() }
    val overlay = overlayOpen(snapshot)
    LaunchedEffect(overlay) { if (!overlay) keys.requestFocus() }
    if (!besideList && windowWidthClass() == WidthClass.COMPACT) {
        CompactDetails(snapshot, selection, onAction, modifier.focusRequester(keys))
        return
    }
    Column(
        modifier
            .fillMaxSize()
            .background(c.background)
            .padding(16.dp)
            .onPreviewKeyEvent { event -> detailsKeyAction(event, selection)?.also(onAction) != null }
            .focusRequester(keys)
            .focusable(),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        if (!besideList) {
            OperationBanner(snapshot.banner, onAction)
            snapshot.pdfOpenError?.let { Text(pdfErrorText(it), color = c.error, fontSize = 15.sp) }
        }
        NotaeCard(Modifier.weight(1f).fillMaxWidth()) {
            Column(Modifier.fillMaxSize().padding(8.dp)) {
                Header(selection, onAction)
                Stepping(selection, Modifier.weight(1f)) { shown, scroll -> Body(shown.row, scroll, onAction) }
            }
        }
    }
}

/**
 * The details on a phone, in One UI's shape: back, "X of Y" and the star at the top; the page, the headline with Edit
 * details and Add to set as tonal buttons, and the info with each label above its value, scrolling; then a bottom bar
 * with Previous, Open score and Next, where the thumb is.
 */
@Composable
private fun CompactDetails(snapshot: Snapshot, selection: Selection, onAction: (Action) -> Unit, modifier: Modifier) {
    val c = NotaeTheme.colors
    val row = selection.row
    Column(
        modifier
            .fillMaxSize()
            .background(c.background)
            .onPreviewKeyEvent { event -> detailsKeyAction(event, selection)?.also(onAction) != null }
            .focusable(),
    ) {
        Row(Modifier.fillMaxWidth().height(64.dp).padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(stringResource(R.string.details_back), painterResource(R.drawable.ic_arrow_back), { onAction(Action.ClearSelection) })
            Position(selection, Modifier.weight(1f))
            FavoriteButton(row, onAction)
        }
        Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            OperationBanner(snapshot.banner, onAction)
            snapshot.pdfOpenError?.let { Text(pdfErrorText(it), color = c.error, fontSize = 15.sp) }
        }
        BoxWithConstraints(Modifier.weight(1f).fillMaxWidth()) {
            val preview = maxOf(120.dp, minOf(220.dp, maxWidth * 0.55f, maxHeight * 0.4f / 1.3f))
            Stepping(selection, Modifier.fillMaxSize()) { shown, scroll -> CompactBody(shown.row, scroll, preview, onAction) }
        }
        Box(Modifier.fillMaxWidth().height(1.dp).background(c.divider))
        Row(
            Modifier.fillMaxWidth().height(76.dp).padding(horizontal = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            StepButton(selection, previous = true, onAction)
            PillButton(
                stringResource(R.string.score_open),
                { onAction(Action.OpenPdf(row.relativePath, row.noteKey, row.itemId)) },
                Modifier.weight(1f),
                icon = painterResource(R.drawable.ic_external_link),
                primary = true,
            )
            StepButton(selection, previous = false, onAction)
        }
    }
}

/**
 * The details' body moving with the score: Next slides it in from the end, Previous from the start, by
 * [Tokens.motionShift]; the score leaving keeps drawing its own row as it goes. Another change to the same score (its
 * star, say) redraws it in place. The score arriving starts where the last one was scrolled to, so stepping through a
 * set keeps the same part of the details in view.
 */
@Composable
private fun Stepping(selection: Selection, modifier: Modifier, content: @Composable (Selection, ScrollState) -> Unit) {
    val carried = remember { mutableIntStateOf(0) }
    val shift = with(LocalDensity.current) { Tokens.motionShift.roundToPx() } * if (LocalLayoutDirection.current == LayoutDirection.Rtl) -1 else 1
    AnimatedContent(
        selection,
        modifier,
        transitionSpec = {
            val back = (targetState.position?.position ?: 0u) < (initialState.position?.position ?: 0u)
            val by = if (back) -shift else shift
            (slideInHorizontally(NotaeMotion.medium()) { by } + fadeIn(NotaeMotion.medium())) togetherWith
                (slideOutHorizontally(NotaeMotion.medium()) { -by } + fadeOut(NotaeMotion.short()))
        },
        contentKey = { it.row.selectionId },
        label = "details step",
    ) { shown ->
        val scroll = rememberScrollState(carried.intValue)
        LaunchedEffect(scroll) { snapshotFlow { scroll.value }.collect { carried.intValue = it } }
        content(shown, scroll)
    }
}

/** The phone's scrolling body: the page, the headline, Edit details and Add to set, and the info. */
@Composable
private fun CompactBody(row: ScoreRowData, scroll: ScrollState, preview: Dp, onAction: (Action) -> Unit) {
    val c = NotaeTheme.colors
    Column(
        Modifier.testTag(DETAILS_BODY_TAG).fillMaxSize().verticalScroll(scroll).padding(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            PagePreview(row.thumbnail, Modifier.size(preview, preview * 1.3f), radius = 16.dp)
        }
        Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            row.number?.let { Text(stringResource(R.string.details_number, it), color = c.accentText, fontSize = 14.sp, fontWeight = FontWeight.SemiBold) }
            Text(row.title, color = c.foreground, fontSize = 26.sp, fontWeight = FontWeight.SemiBold, lineHeight = 32.sp)
            row.composer?.takeIf { it.isNotEmpty() }?.let { Text(it, color = c.muted, fontSize = 17.sp) }
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            PillButton(
                stringResource(R.string.score_edit_details),
                { onAction(Action.EditMetadata(row.noteKey)) },
                Modifier.weight(1f),
                icon = painterResource(R.drawable.ic_edit),
                tonal = true,
            )
            PillButton(
                stringResource(R.string.score_add_to_set),
                { onAction(Action.AddToSet(row.noteKey)) },
                Modifier.weight(1f),
                icon = painterResource(R.drawable.ic_folder),
                tonal = true,
            )
        }
        InfoCard(row, stretch = false, stacked = true, background = c.card)
    }
}

/** "X of Y" in a set or a list, read as "Score X of Y". */
@Composable
private fun Position(selection: Selection, modifier: Modifier) {
    val position = selection.position?.let { stringResource(R.string.details_position, it.position.toLong(), it.total.toLong()) }
    val positionLabel = position?.let { stringResource(R.string.details_position_label, it) }
    Text(
        position.orEmpty(),
        modifier.then(if (positionLabel != null) Modifier.semantics { contentDescription = positionLabel } else Modifier),
        color = NotaeTheme.colors.muted,
        fontSize = 15.sp,
        textAlign = TextAlign.Center,
    )
}

@Composable
private fun FavoriteButton(row: ScoreRowData, onAction: (Action) -> Unit) {
    IconButton(
        stringResource(if (row.favorite) R.string.score_remove_favorite else R.string.score_add_favorite),
        painterResource(if (row.favorite) R.drawable.ic_star_filled else R.drawable.ic_star),
        { onAction(Action.ToggleFavorite(row.noteKey)) },
        selected = row.favorite,
    )
}

/** Previous or Next score, disabled at the ends. */
@Composable
private fun StepButton(selection: Selection, previous: Boolean, onAction: (Action) -> Unit) {
    val target = if (previous) selection.previous else selection.next
    IconButton(
        stringResource(if (previous) R.string.details_previous else R.string.details_next),
        painterResource(if (previous) R.drawable.ic_chevron_left else R.drawable.ic_chevron_right),
        { target?.let { onAction(Action.SelectScore(it)) } },
        enabled = target != null,
    )
}

/**
 * The details' keys: Left and Right to the core's neighbours, Escape back, Ctrl K to search. The library pane beside
 * the details uses them too, so the keys step whichever pane holds focus.
 */
internal fun detailsKeyAction(event: KeyEvent, selection: Selection): Action? = when {
    event.type != KeyEventType.KeyDown -> null
    event.isCtrlK() -> Action.OpenSearch
    event.key == Key.DirectionLeft -> selection.previous?.let { Action.SelectScore(it) }
    event.key == Key.DirectionRight -> selection.next?.let { Action.SelectScore(it) }
    event.key == Key.Escape -> Action.ClearSelection
    else -> null
}

@Composable
private fun Header(selection: Selection, onAction: (Action) -> Unit) {
    Row(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically) {
        IconButton(stringResource(R.string.details_back), painterResource(R.drawable.ic_arrow_back), { onAction(Action.ClearSelection) })
        Position(selection, Modifier.weight(1f))
        FavoriteButton(selection.row, onAction)
        StepButton(selection, previous = true, onAction)
        StepButton(selection, previous = false, onAction)
    }
}

/** The scrolling body; its layout follows the card's own width, as 0.3.0's `wide` does. */
@Composable
private fun Body(row: ScoreRowData, scroll: ScrollState, onAction: (Action) -> Unit) {
    BoxWithConstraints(Modifier.fillMaxSize()) {
        // The card's size: this box plus the card's 8 dp padding, and the 48 dp header above it.
        val cardWidth = maxWidth + 16.dp
        val cardHeight = maxHeight + 64.dp
        val wide = cardWidth >= 720.dp
        val preview = if (wide) {
            maxOf(160.dp, minOf(380.dp, cardWidth * 0.4f, (cardHeight - 112.dp) / 1.3f))
        } else {
            maxOf(140.dp, minOf(300.dp, cardWidth - 64.dp))
        }
        Column(Modifier.testTag(DETAILS_BODY_TAG).fillMaxSize().verticalScroll(scroll).padding(start = 16.dp, end = 16.dp, bottom = 24.dp)) {
            if (wide) {
                Row(
                    Modifier.fillMaxWidth().height(IntrinsicSize.Min),
                    horizontalArrangement = Arrangement.spacedBy(32.dp, Alignment.CenterHorizontally),
                ) {
                    PagePreview(row.thumbnail, Modifier.size(preview, preview * 1.3f), radius = 16.dp)
                    // Reading width: the details stay beside the page instead of stretching across a tablet.
                    Column(
                        Modifier.weight(1f, fill = false).widthIn(max = 560.dp).fillMaxHeight(),
                        verticalArrangement = Arrangement.spacedBy(24.dp),
                    ) {
                        Headline(row, centered = false, onAction)
                        InfoCard(row, stretch = true, Modifier.weight(1f))
                    }
                }
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(20.dp)) {
                    Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                        PagePreview(row.thumbnail, Modifier.size(preview, preview * 1.3f), radius = 16.dp)
                    }
                    Headline(row, centered = true, onAction)
                    InfoCard(row, stretch = false)
                }
            }
        }
    }
}

/** 0.3.0's `ScoreHeadline`: "No. N", the title, the composer, then Open score and Edit details. */
@Composable
private fun Headline(row: ScoreRowData, centered: Boolean, onAction: (Action) -> Unit) {
    val c = NotaeTheme.colors
    val align = if (centered) TextAlign.Center else TextAlign.Start
    val width = Modifier.fillMaxWidth()
    Column(width, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        row.number?.let {
            Text(stringResource(R.string.details_number, it), width, color = c.accentText, fontSize = 14.sp, fontWeight = FontWeight.SemiBold, textAlign = align)
        }
        Text(row.title, width, color = c.foreground, fontSize = if (centered) 26.sp else 32.sp, fontWeight = FontWeight.SemiBold, textAlign = align)
        row.composer?.takeIf { it.isNotEmpty() }?.let { Text(it, width, color = c.muted, fontSize = 17.sp, textAlign = align) }
        Row(
            width.padding(top = 18.dp),
            horizontalArrangement = Arrangement.spacedBy(10.dp, if (centered) Alignment.CenterHorizontally else Alignment.Start),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            PillButton(stringResource(R.string.score_open), { onAction(Action.OpenPdf(row.relativePath, row.noteKey, row.itemId)) }, primary = true)
            IconButton(stringResource(R.string.score_edit_details), painterResource(R.drawable.ic_edit), { onAction(Action.EditMetadata(row.noteKey)) })
        }
    }
}

/**
 * 0.3.0's `InfoCard`: Title, Composer, Tags and File, each with where it came from or its status. Beside the page it is
 * as tall as the page allows ([stretch]) and its rows share the extra height.
 */
@Composable
private fun InfoCard(
    row: ScoreRowData,
    stretch: Boolean,
    modifier: Modifier = Modifier,
    stacked: Boolean = false,
    background: Color = NotaeTheme.colors.background,
) {
    // An image-only PDF shows no status: the library does not nag about it (0.3.0's `rows.rs`).
    val status = if (row.textPending) null else statusText(row.status)
    Column(
        modifier.fillMaxWidth().clip(RoundedCornerShape(20.dp)).background(background).padding(horizontal = 20.dp, vertical = 4.dp),
    ) {
        val share = if (stretch) Modifier.weight(1f) else Modifier
        InfoRow(stringResource(R.string.details_title), row.title, originText(row.titleOrigin), share, stacked)
        Divider()
        InfoRow(stringResource(R.string.details_composer), row.composer.orEmpty(), originText(row.composerOrigin), share, stacked)
        Divider()
        InfoRow(stringResource(R.string.details_tags), row.tags.joinToString(", "), null, share, stacked)
        Divider()
        InfoRow(stringResource(R.string.details_file), row.relativePath, status, share, stacked)
    }
}

@Composable
private fun InfoRow(label: String, value: String, note: String?, modifier: Modifier, stacked: Boolean) {
    val c = NotaeTheme.colors
    if (stacked) {
        // A phone puts the label above its value, so a long file path keeps the whole width.
        Column(modifier.fillMaxWidth().heightIn(min = 60.dp).padding(vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label, color = c.muted, fontSize = 13.sp)
            Text(value.ifEmpty { "—" }, color = c.foreground, fontSize = 16.sp)
            note?.takeIf { it.isNotEmpty() }?.let { Text(it, color = c.muted, fontSize = 13.sp) }
        }
        return
    }
    Row(
        modifier.fillMaxWidth().heightIn(min = 60.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, Modifier.width(96.dp), color = c.muted, fontSize = 15.sp)
        Column(Modifier.weight(1f).padding(vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(value.ifEmpty { "—" }, color = c.foreground, fontSize = 16.sp)
            note?.takeIf { it.isNotEmpty() }?.let { Text(it, color = c.muted, fontSize = 13.sp) }
        }
    }
}

@Composable
private fun Divider() {
    Box(Modifier.fillMaxWidth().height(1.dp).background(NotaeTheme.colors.divider))
}

/** Where a value came from (texts.rs `origin`); nothing when unknown. */
@Composable
private fun originText(origin: Origin): String? = when (origin) {
    Origin.FILENAME_DERIVED -> stringResource(R.string.origin_filename)
    Origin.PC_IMPORTED -> stringResource(R.string.origin_desktop)
    Origin.MANUAL -> stringResource(R.string.origin_manual)
    Origin.UNSET -> null
}

/** A row's status (texts.rs `row_status`); text pending shows none. */
@Composable
private fun statusText(status: RowStatus): String? = when (status) {
    RowStatus.READY -> stringResource(R.string.status_ready)
    RowStatus.TEXT_PENDING -> null
    RowStatus.MISSING_FROM_LIBRARY -> stringResource(R.string.status_missing)
    RowStatus.UNREADABLE -> stringResource(R.string.status_unreadable)
    RowStatus.INVALID_PDF -> stringResource(R.string.status_invalid_pdf)
    RowStatus.DISCOVERED -> stringResource(R.string.status_discovered)
    RowStatus.INDEXING -> stringResource(R.string.status_indexing)
    RowStatus.STALE -> stringResource(R.string.status_stale)
    RowStatus.ID_CONFLICT -> stringResource(R.string.status_id_conflict)
    RowStatus.IMPORT_MISMATCH -> stringResource(R.string.status_import_mismatch)
}
