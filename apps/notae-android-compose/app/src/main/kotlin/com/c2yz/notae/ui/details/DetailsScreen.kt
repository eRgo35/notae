package com.c2yz.notae.ui.details

import androidx.compose.foundation.background
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
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
import com.c2yz.notae.ui.components.NotaeCard
import com.c2yz.notae.ui.components.OperationBanner
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.library.PagePreview
import com.c2yz.notae.ui.library.pdfErrorText
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * Slint `ScoreDetails` in the library's preview layer: it replaces the whole library chrome. The operation banner and
 * the PDF launch error stay above the card. The header has back, "X of Y", the star, previous and next. The body
 * scrolls and puts the page beside the headline and the info card when the card is at least 720 dp wide; under that,
 * it stacks them centred. Neighbours and the position are the core's ([Selection]): each is a selection id, so every
 * copy of a repeated piece in a set is its own step. Left and Right move to them, Escape goes back.
 */
@Composable
fun DetailsScreen(snapshot: Snapshot, onAction: (Action) -> Unit, modifier: Modifier = Modifier) {
    val selection = snapshot.selection ?: return
    val c = NotaeTheme.colors
    Column(
        modifier
            .fillMaxSize()
            .background(c.background)
            .padding(16.dp)
            .onPreviewKeyEvent { event -> event.type == KeyEventType.KeyDown && keyAction(event.key, selection)?.also(onAction) != null },
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        OperationBanner(snapshot.banner, onAction)
        snapshot.pdfOpenError?.let { Text(pdfErrorText(it), color = c.error, fontSize = 15.sp) }
        NotaeCard(Modifier.weight(1f).fillMaxWidth()) {
            Column(Modifier.fillMaxSize().padding(8.dp)) {
                Header(selection, onAction)
                Body(selection.row, onAction)
            }
        }
    }
}

private fun keyAction(key: Key, selection: Selection): Action? = when (key) {
    Key.DirectionLeft -> selection.previous?.let { Action.SelectScore(it) }
    Key.DirectionRight -> selection.next?.let { Action.SelectScore(it) }
    Key.Escape -> Action.ClearSelection
    else -> null
}

@Composable
private fun Header(selection: Selection, onAction: (Action) -> Unit) {
    val c = NotaeTheme.colors
    val row = selection.row
    Row(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically) {
        IconButton(stringResource(R.string.details_back), painterResource(R.drawable.ic_arrow_back), { onAction(Action.ClearSelection) })
        val position = selection.position?.let { stringResource(R.string.details_position, it.position.toLong(), it.total.toLong()) }
        val positionLabel = position?.let { stringResource(R.string.details_position_label, it) }
        Text(
            position.orEmpty(),
            Modifier.weight(1f).then(if (positionLabel != null) Modifier.semantics { contentDescription = positionLabel } else Modifier),
            color = c.muted,
            fontSize = 15.sp,
            textAlign = TextAlign.Center,
        )
        IconButton(
            stringResource(if (row.favorite) R.string.score_remove_favorite else R.string.score_add_favorite),
            painterResource(if (row.favorite) R.drawable.ic_star_filled else R.drawable.ic_star),
            { onAction(Action.ToggleFavorite(row.noteKey)) },
            selected = row.favorite,
        )
        IconButton(
            stringResource(R.string.details_previous),
            painterResource(R.drawable.ic_chevron_left),
            { selection.previous?.let { onAction(Action.SelectScore(it)) } },
            enabled = selection.previous != null,
        )
        IconButton(
            stringResource(R.string.details_next),
            painterResource(R.drawable.ic_chevron_right),
            { selection.next?.let { onAction(Action.SelectScore(it)) } },
            enabled = selection.next != null,
        )
    }
}

/** The scrolling body; its layout follows the card's own width, as Slint's `wide` does. */
@Composable
private fun ColumnScope.Body(row: ScoreRowData, onAction: (Action) -> Unit) {
    BoxWithConstraints(Modifier.weight(1f).fillMaxWidth()) {
        // The card's size: this box plus the card's 8 dp padding, and the 48 dp header above it.
        val cardWidth = maxWidth + 16.dp
        val cardHeight = maxHeight + 64.dp
        val wide = cardWidth >= 720.dp
        val preview = if (wide) {
            maxOf(160.dp, minOf(380.dp, cardWidth * 0.4f, (cardHeight - 112.dp) / 1.3f))
        } else {
            maxOf(140.dp, minOf(300.dp, cardWidth - 64.dp))
        }
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(start = 16.dp, end = 16.dp, bottom = 24.dp)) {
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

/** Slint `ScoreHeadline`: "No. N", the title, the composer, then Open score and Edit details. */
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
 * Slint `InfoCard`: Title, Composer, Tags and File, each with where it came from or its status. Beside the page it is
 * as tall as the page allows ([stretch]) and its rows share the extra height.
 */
@Composable
private fun InfoCard(row: ScoreRowData, stretch: Boolean, modifier: Modifier = Modifier) {
    val c = NotaeTheme.colors
    // An image-only PDF shows no status: the library does not nag about it (Slint `rows.rs`).
    val status = if (row.textPending) null else statusText(row.status)
    Column(
        modifier.fillMaxWidth().clip(RoundedCornerShape(20.dp)).background(c.background).padding(horizontal = 20.dp, vertical = 4.dp),
    ) {
        val share = if (stretch) Modifier.weight(1f) else Modifier
        InfoRow(stringResource(R.string.details_title), row.title, originText(row.titleOrigin), share)
        Divider()
        InfoRow(stringResource(R.string.details_composer), row.composer.orEmpty(), originText(row.composerOrigin), share)
        Divider()
        InfoRow(stringResource(R.string.details_tags), row.tags.joinToString(", "), null, share)
        Divider()
        InfoRow(stringResource(R.string.details_file), row.relativePath, status, share)
    }
}

@Composable
private fun InfoRow(label: String, value: String, note: String?, modifier: Modifier) {
    val c = NotaeTheme.colors
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
