package com.c2yz.notae.ui.library

import androidx.compose.animation.Crossfade
import com.c2yz.notae.ui.theme.NotaeMotion
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.snap
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.Row as ScoreRowData
import com.c2yz.notae.core.SetEntry
import com.c2yz.notae.ui.components.WidthClass
import com.c2yz.notae.ui.components.windowWidthClass
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.components.NotaeCard
import com.c2yz.notae.ui.components.Scrollbar
import com.c2yz.notae.ui.theme.NotaeTheme
import kotlin.math.floor
import kotlin.math.roundToInt

internal const val GRID_TAG = "library-grid"
internal const val LIST_TAG = "library-list"
internal const val REORDER_TAG = "set-reorder-list"
internal const val FAVORITE_BADGE_TAG = "favorite-badge"

/** A grid narrower than this (a phone) packs its cards tighter, two to a row. */
internal val NARROW_GRID = 480.dp

/**
 * 0.3.0's `ScoreGrid` in its card: 20 dp inset, columns of at least 176 dp with 16 dp gaps, 20 dp between rows, and
 * cards 1.3 times as tall as wide plus 64 dp for the text. Inside a set its text notes are tinted cards in their place.
 * Keyed by the selection id, or a note's item id.
 */
@Composable
internal fun ScoreGrid(
    entries: List<SetEntry>,
    onOpen: (ScoreRowData) -> Unit,
    onMore: (ScoreRowData) -> Unit,
    onNoteMore: (SetEntry.Note) -> Unit,
    modifier: Modifier = Modifier,
    selectedId: String? = null,
) {
    NotaeCard(modifier.fillMaxSize()) {
        BoxWithConstraints(Modifier.fillMaxSize()) {
            // On a phone the cards shrink to 140 dp with tighter gaps, so two columns fit.
            val narrow = maxWidth < NARROW_GRID
            val inset = if (narrow) 12.dp else 20.dp
            val gap = if (narrow) 12.dp else 16.dp
            val inner = maxWidth - inset * 2
            val columns = floor((inner + gap) / ((if (narrow) 140.dp else 176.dp) + gap)).toInt().coerceAtLeast(1)
            val state = rememberLazyGridState()
            // Other columns lay the grid out afresh, keeping its scroll position: its cards snap to their new places,
            // even ones gliding, instead of flying over each other.
            key(columns, narrow) {
                LazyVerticalGrid(
                    GridCells.Fixed(columns),
                    Modifier.testTag(GRID_TAG).fillMaxSize().padding(horizontal = inset),
                    state = state,
                    contentPadding = PaddingValues(top = inset, bottom = 20.dp),
                    horizontalArrangement = Arrangement.spacedBy(gap),
                    verticalArrangement = Arrangement.spacedBy(20.dp),
                ) {
                    items(entries, key = { it.key() }) { entry ->
                        // Items glide to their new places, and fade in or out, when the section's scores change.
                        Box(Modifier.animateItem()) {
                            when (entry) {
                                is SetEntry.Piece -> {
                                    val row = entry.row
                                    ScoreCard(row, { onOpen(row) }, { onMore(row) }, Modifier.cellHeight(1.3f, 64.dp), selected = row.selectionId == selectedId)
                                }
                                is SetEntry.Note -> NoteCard(entry, { onNoteMore(entry) }, Modifier.cellHeight(1.3f, 64.dp))
                            }
                        }
                    }
                }
            }
            GridScrollbar(state, Modifier.align(Alignment.TopEnd).padding(top = 20.dp))
        }
    }
}

/**
 * A grid card's height: [ratio] times its cell's width plus [extra], taken as the card is measured, so it changes in the
 * same frame as its width rather than a composition later.
 */
internal fun Modifier.cellHeight(ratio: Float, extra: Dp): Modifier = layout { measurable, constraints ->
    val height = (constraints.maxWidth * ratio + extra.roundToPx()).roundToInt()
    val placeable = measurable.measure(constraints.copy(minHeight = height, maxHeight = height))
    layout(placeable.width, height) { placeable.place(0, 0) }
}

/** 0.3.0's list layout: 80 dp rows in a card with an 8 dp inset; a set's notes are tinted rows. Keyed as [ScoreGrid]. */
@Composable
internal fun ScoreList(
    entries: List<SetEntry>,
    onOpen: (ScoreRowData) -> Unit,
    onMore: (ScoreRowData) -> Unit,
    onNoteMore: (SetEntry.Note) -> Unit,
    modifier: Modifier = Modifier,
    selectedId: String? = null,
) {
    NotaeCard(modifier.fillMaxSize()) {
        val state = rememberLazyListState()
        LazyColumn(Modifier.testTag(LIST_TAG).fillMaxSize().padding(8.dp), state = state) {
            items(entries, key = { it.key() }) { entry ->
                Box(Modifier.animateItem()) {
                    when (entry) {
                        is SetEntry.Piece -> {
                            val row = entry.row
                            ScoreRow(row, { onOpen(row) }, { onMore(row) }, selected = row.selectionId == selectedId)
                        }
                        is SetEntry.Note -> NoteRow(entry, { onNoteMore(entry) })
                    }
                }
            }
        }
        ListScrollbar(state, Modifier.align(Alignment.TopEnd).padding(vertical = 8.dp))
    }
}

/** A lazy key unique among a set's items: a piece's selection id (its item id in a set) or the note's item id. */
internal fun SetEntry.key(): String = when (this) {
    is SetEntry.Piece -> row.selectionId
    is SetEntry.Note -> itemId
}

/** Recomposes only when the first visible item, the visible count or the total changes, not on every scrolled pixel. */
@Composable
internal fun GridScrollbar(state: LazyGridState, modifier: Modifier) {
    val window by remember(state) {
        derivedStateOf { Triple(state.firstVisibleItemIndex, state.layoutInfo.visibleItemsInfo.size, state.layoutInfo.totalItemsCount) }
    }
    Scrollbar(window.first, window.second, window.third, state.isScrollInProgress, { state.scrollToItem(it) }, modifier)
}

@Composable
internal fun ListScrollbar(state: LazyListState, modifier: Modifier) {
    val window by remember(state) {
        derivedStateOf { Triple(state.firstVisibleItemIndex, state.layoutInfo.visibleItemsInfo.size, state.layoutInfo.totalItemsCount) }
    }
    Scrollbar(window.first, window.second, window.third, state.isScrollInProgress, { state.scrollToItem(it) }, modifier)
}

/**
 * 0.3.0's `ScoreCard`: the page with the title and "number · composer" (or "Missing from library") under it, a
 * position badge inside a set, a star on a favorite and the ⋮ in the bottom corner. A missing piece cannot be opened.
 * The set's item opened last is highlighted like a selected one and carries the "Just opened" chip.
 */
@Composable
internal fun ScoreCard(row: ScoreRowData, onOpen: () -> Unit, onMore: () -> Unit, modifier: Modifier = Modifier, selected: Boolean = false) {
    val c = NotaeTheme.colors
    val highlighted = selected || row.justOpened
    val shape = RoundedCornerShape(20.dp)
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val focused by source.collectIsFocusedAsState()
    Box(modifier.fillMaxWidth()) {
        Column(
            Modifier
                .fillMaxSize()
                .clip(shape)
                .background(itemBackground(highlighted, pressed))
                .then(if (focused) Modifier.border(BorderStroke(2.dp, c.accent), shape) else Modifier)
                .scoreClick(row, source, onOpen, selected)
                .padding(6.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            PagePreview(row.thumbnail, Modifier.weight(1f).fillMaxWidth())
            Column(Modifier.padding(start = 4.dp, end = 44.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                ScoreTitle(row, highlighted)
                ScoreSubtitle(row, 14, selected = highlighted)
            }
        }
        Row(Modifier.padding(14.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            row.position?.let { PositionBadge(it) }
            if (row.justOpened) JustOpenedChip()
        }
        if (row.favorite) FavoriteBadge(Modifier.align(Alignment.TopEnd).padding(14.dp))
        MoreActions(row.title, onMore, Modifier.align(Alignment.BottomEnd).padding(2.dp))
    }
}

/**
 * 0.3.0's `ScoreRow`: one 80 dp line with the position, a small page, the text, a star on a favorite and the ⋮. The
 * set's item opened last is highlighted like a selected one, with the "Just opened" chip before the star.
 */
@Composable
internal fun ScoreRow(row: ScoreRowData, onOpen: () -> Unit, onMore: () -> Unit, modifier: Modifier = Modifier, selected: Boolean = false) {
    val c = NotaeTheme.colors
    val highlighted = selected || row.justOpened
    val compact = windowWidthClass() == WidthClass.COMPACT
    val shape = RoundedCornerShape(18.dp)
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val focused by source.collectIsFocusedAsState()
    Row(
        modifier
            .fillMaxWidth()
            .then(if (compact) Modifier.heightIn(min = 80.dp) else Modifier.height(80.dp))
            .clip(shape)
            .background(itemBackground(highlighted, pressed))
            .then(if (focused) Modifier.border(BorderStroke(2.dp, c.accent), shape) else Modifier)
            .scoreClick(row, source, onOpen, selected)
            .padding(start = 14.dp, end = 8.dp, top = if (compact) 8.dp else 0.dp, bottom = if (compact) 8.dp else 0.dp),
        horizontalArrangement = Arrangement.spacedBy(if (compact) 12.dp else 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        row.position?.let { PositionBadge(it) }
        PagePreview(row.thumbnail, Modifier.size(44.dp, 58.dp), radius = 6.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(1.dp)) {
            ScoreTitle(row, highlighted)
            ScoreSubtitle(row, 14, selected = highlighted)
            if (row.tags.isNotEmpty()) {
                Text(row.tags.joinToString(", "), color = if (highlighted) c.onAccentSoft else c.muted, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            // On a phone the chip goes under the text, which keeps the width for the title.
            if (row.justOpened && compact) JustOpenedChip()
        }
        if (row.justOpened && !compact) JustOpenedChip()
        if (row.favorite) FavoriteBadge()
        MoreActions(row.title, onMore)
    }
}

/**
 * The card or row's title (the item's own label reads it), followed inside a set by a small note glyph when the piece has
 * a set note; TalkBack reads that as "Has a set note" after the item's name.
 */
@Composable
private fun ScoreTitle(row: ScoreRowData, highlighted: Boolean) {
    val c = NotaeTheme.colors
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(
            row.title,
            Modifier.weight(1f, fill = false).clearAndSetSemantics { },
            color = if (highlighted) c.onAccentSoft else c.foreground,
            fontSize = 16.sp,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        if (!row.setNote.isNullOrBlank()) {
            Icon(
                painterResource(R.drawable.ic_note),
                stringResource(R.string.has_set_note),
                Modifier.size(16.dp),
                tint = if (highlighted) c.onAccentSoft else c.accentText,
            )
        }
    }
}

/** 0.3.0's item background: accent-soft while the score shows in the details beside the list or was just opened, else the press. */
@Composable
private fun itemBackground(highlighted: Boolean, pressed: Boolean): Color {
    val target = when {
        highlighted -> NotaeTheme.colors.accentSoft
        pressed -> NotaeTheme.colors.field
        else -> NotaeTheme.colors.field.copy(alpha = 0f)
    }
    return animateColorAsState(target, NotaeMotion.short(), label = "item fill").value
}

/**
 * The card or row's own target: named "N. title" as in 0.3.0 (with ", Just opened" on the set's item opened last), read
 * with its composer line, and opening the PDF. A piece missing from the library has nothing to open, so it is not
 * clickable. The score shown beside the list is marked selected.
 */
@Composable
private fun Modifier.scoreClick(row: ScoreRowData, source: MutableInteractionSource, onOpen: () -> Unit, selected: Boolean): Modifier {
    val name = (row.position?.let { "$it. " } ?: "") + row.title
    val label = if (row.justOpened) stringResource(R.string.score_label_just_opened, name) else name
    val opens = stringResource(R.string.score_opens)
    return semantics {
        contentDescription = label
        if (selected) this.selected = true
    }
        .clickable(source, null, enabled = !row.missing, onClickLabel = opens, role = Role.Button, onClick = onOpen)
}

/**
 * "number · composer" (either alone when the other is missing), or "Missing from library" in the error colour; in the
 * accent-soft text colour on a selected item, as 0.3.0's.
 */
@Composable
internal fun ScoreSubtitle(row: ScoreRowData, size: Int, missingInError: Boolean = true, selected: Boolean = false) {
    val c = NotaeTheme.colors
    val text = if (row.missing) stringResource(R.string.score_missing) else listOfNotNull(row.number, row.composer).joinToString(" · ")
    Text(
        text,
        color = when {
            row.missing && missingInError -> c.error
            selected -> c.onAccentSoft
            else -> c.muted
        },
        fontSize = size.sp,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
    )
}

/** 0.3.0's `PagePreview`: the thumbnail on paper with a 1 dp edge, or a document glyph until the scan draws it. */
@Composable
internal fun PagePreview(thumbnail: String?, modifier: Modifier = Modifier, radius: Dp = 14.dp) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(radius)
    val image by rememberThumbnail(thumbnail)
    BoxWithConstraints(
        modifier.clip(shape).border(BorderStroke(1.dp, c.paperEdge), shape),
        contentAlignment = Alignment.Center,
    ) {
        val glyph = minOf(40.dp, maxWidth / 2)
        // A page decoded after the card shows fades in over the glyph; one already in the cache shows at once.
        Crossfade(image, Modifier.fillMaxSize(), NotaeMotion.medium(), label = "page") { bitmap ->
            // The paper fades in with the page, instead of cutting in before it.
            Box(Modifier.fillMaxSize().background(if (bitmap != null) c.paper else c.pageBlank), contentAlignment = Alignment.Center) {
                if (bitmap != null) {
                    Image(bitmap, null, Modifier.fillMaxSize(), contentScale = ContentScale.Fit)
                } else {
                    Icon(painterResource(R.drawable.ic_document), null, Modifier.size(glyph), tint = c.muted)
                }
            }
        }
    }
}

/** 0.3.0's `FavoriteBadge`: a 16 dp accent star on a 24 dp card-coloured circle. */
@Composable
private fun FavoriteBadge(modifier: Modifier = Modifier) {
    val c = NotaeTheme.colors
    Box(modifier.testTag(FAVORITE_BADGE_TAG).size(24.dp).clip(CircleShape).background(c.card), Alignment.Center) {
        Icon(painterResource(R.drawable.ic_star_filled), null, Modifier.size(16.dp), tint = c.accent)
    }
}

/** 0.3.0's `PositionBadge`: the item's number in its set, on an accent-soft pill at least 28 dp wide. */
@Composable
internal fun PositionBadge(position: UInt, modifier: Modifier = Modifier) {
    val c = NotaeTheme.colors
    Box(
        modifier.height(28.dp).widthIn(min = 28.dp).clip(RoundedCornerShape(14.dp)).background(c.accentSoft).padding(horizontal = 8.dp),
        Alignment.Center,
    ) {
        Text(position.toString(), Modifier.clearAndSetSemantics { }, color = c.onAccentSoft, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
    }
}

/** "Just opened" on an accent pill as tall as the position badge; TalkBack reads it in the item's name instead. */
@Composable
private fun JustOpenedChip(modifier: Modifier = Modifier) {
    val c = NotaeTheme.colors
    Box(modifier.height(28.dp).clip(RoundedCornerShape(14.dp)).background(c.accent).padding(horizontal = 10.dp), Alignment.Center) {
        Text(
            stringResource(R.string.just_opened),
            Modifier.clearAndSetSemantics { },
            color = c.onAccent,
            fontSize = 13.sp,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
        )
    }
}

/** 0.3.0's `MoreActions`: the ⋮, named after the score, the only way to its actions sheet. */
@Composable
internal fun MoreActions(title: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    IconButton(stringResource(R.string.score_more_actions, title), painterResource(R.drawable.ic_more_vertical), onClick, modifier)
}
