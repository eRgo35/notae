package com.c2yz.notae.ui.library

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
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
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
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.components.NotaeCard
import com.c2yz.notae.ui.components.Scrollbar
import com.c2yz.notae.ui.theme.NotaeTheme
import kotlin.math.floor

internal const val GRID_TAG = "library-grid"
internal const val LIST_TAG = "library-list"
internal const val REORDER_TAG = "set-reorder-list"
internal const val FAVORITE_BADGE_TAG = "favorite-badge"

/**
 * Slint `ScoreGrid` in its card: 20 dp inset, columns of at least 176 dp with 16 dp gaps, 20 dp between rows, and
 * cards 1.3 times as tall as wide plus 64 dp for the text. Keyed by the selection id.
 */
@Composable
internal fun ScoreGrid(
    rows: List<ScoreRowData>,
    onOpen: (ScoreRowData) -> Unit,
    onMore: (ScoreRowData) -> Unit,
    modifier: Modifier = Modifier,
    selectedId: String? = null,
) {
    NotaeCard(modifier.fillMaxSize()) {
        BoxWithConstraints(Modifier.fillMaxSize()) {
            val inner = maxWidth - 40.dp
            val columns = floor((inner + 16.dp) / 192.dp).toInt().coerceAtLeast(1)
            val cellWidth = (inner - 16.dp * (columns - 1)) / columns
            val cellHeight = cellWidth * 1.3f + 64.dp
            val state = rememberLazyGridState()
            LazyVerticalGrid(
                GridCells.Fixed(columns),
                Modifier.testTag(GRID_TAG).fillMaxSize().padding(horizontal = 20.dp),
                state = state,
                contentPadding = PaddingValues(top = 20.dp, bottom = 20.dp),
                horizontalArrangement = Arrangement.spacedBy(16.dp),
                verticalArrangement = Arrangement.spacedBy(20.dp),
            ) {
                items(rows, key = { it.selectionId }) { row ->
                    ScoreCard(row, { onOpen(row) }, { onMore(row) }, Modifier.height(cellHeight), selected = row.selectionId == selectedId)
                }
            }
            GridScrollbar(state, Modifier.align(Alignment.TopEnd).padding(top = 20.dp))
        }
    }
}

/** Slint's list layout: 80 dp rows in a card with an 8 dp inset. Keyed by the selection id. */
@Composable
internal fun ScoreList(
    rows: List<ScoreRowData>,
    onOpen: (ScoreRowData) -> Unit,
    onMore: (ScoreRowData) -> Unit,
    modifier: Modifier = Modifier,
    selectedId: String? = null,
) {
    NotaeCard(modifier.fillMaxSize()) {
        val state = rememberLazyListState()
        LazyColumn(Modifier.testTag(LIST_TAG).fillMaxSize().padding(8.dp), state = state) {
            items(rows, key = { it.selectionId }) { row ->
                ScoreRow(row, { onOpen(row) }, { onMore(row) }, selected = row.selectionId == selectedId)
            }
        }
        ListScrollbar(state, Modifier.align(Alignment.TopEnd).padding(vertical = 8.dp))
    }
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
 * Slint `ScoreCard`: the page with the title and "number · composer" (or "Missing from library") under it, a
 * position badge inside a set, a star on a favorite and the ⋮ in the bottom corner. A missing piece cannot be opened.
 */
@Composable
internal fun ScoreCard(row: ScoreRowData, onOpen: () -> Unit, onMore: () -> Unit, modifier: Modifier = Modifier, selected: Boolean = false) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(20.dp)
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val focused by source.collectIsFocusedAsState()
    Box(modifier.fillMaxWidth()) {
        Column(
            Modifier
                .fillMaxSize()
                .clip(shape)
                .background(itemBackground(selected, pressed))
                .then(if (focused) Modifier.border(BorderStroke(2.dp, c.accent), shape) else Modifier)
                .scoreClick(row, source, onOpen, selected)
                .padding(6.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            PagePreview(row.thumbnail, Modifier.weight(1f).fillMaxWidth())
            Column(Modifier.padding(start = 4.dp, end = 44.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(
                    row.title,
                    Modifier.clearAndSetSemantics { },
                    color = if (selected) c.onAccentSoft else c.foreground,
                    fontSize = 16.sp,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                ScoreSubtitle(row, 14, selected = selected)
            }
        }
        row.position?.let { PositionBadge(it, Modifier.padding(14.dp)) }
        if (row.favorite) FavoriteBadge(Modifier.align(Alignment.TopEnd).padding(14.dp))
        MoreActions(row.title, onMore, Modifier.align(Alignment.BottomEnd).padding(2.dp))
    }
}

/** Slint `ScoreRow`: one 80 dp line with the position, a small page, the text, a star on a favorite and the ⋮. */
@Composable
internal fun ScoreRow(row: ScoreRowData, onOpen: () -> Unit, onMore: () -> Unit, modifier: Modifier = Modifier, selected: Boolean = false) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(18.dp)
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val focused by source.collectIsFocusedAsState()
    Row(
        modifier
            .fillMaxWidth()
            .height(80.dp)
            .clip(shape)
            .background(itemBackground(selected, pressed))
            .then(if (focused) Modifier.border(BorderStroke(2.dp, c.accent), shape) else Modifier)
            .scoreClick(row, source, onOpen, selected)
            .padding(start = 14.dp, end = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        row.position?.let { PositionBadge(it) }
        PagePreview(row.thumbnail, Modifier.size(44.dp, 58.dp), radius = 6.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(1.dp)) {
            Text(
                row.title,
                Modifier.clearAndSetSemantics { },
                color = if (selected) c.onAccentSoft else c.foreground,
                fontSize = 16.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            ScoreSubtitle(row, 14, selected = selected)
            if (row.tags.isNotEmpty()) {
                Text(row.tags.joinToString(", "), color = if (selected) c.onAccentSoft else c.muted, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        if (row.favorite) FavoriteBadge()
        MoreActions(row.title, onMore)
    }
}

/** Slint's item background: accent-soft while the score shows in the details beside the list, else the press. */
@Composable
private fun itemBackground(selected: Boolean, pressed: Boolean): Color = when {
    selected -> NotaeTheme.colors.accentSoft
    pressed -> NotaeTheme.colors.field
    else -> Color.Transparent
}

/**
 * The card or row's own target: named "N. title" as in Slint, read with its composer line, and opening the PDF.
 * A piece missing from the library has nothing to open, so it is not clickable. The score shown beside the list is
 * marked selected.
 */
@Composable
private fun Modifier.scoreClick(row: ScoreRowData, source: MutableInteractionSource, onOpen: () -> Unit, selected: Boolean): Modifier {
    val label = (row.position?.let { "$it. " } ?: "") + row.title
    val opens = stringResource(R.string.score_opens)
    return semantics {
        contentDescription = label
        if (selected) this.selected = true
    }
        .clickable(source, null, enabled = !row.missing, onClickLabel = opens, role = Role.Button, onClick = onOpen)
}

/**
 * "number · composer" (either alone when the other is missing), or "Missing from library" in the error colour; in the
 * accent-soft text colour on a selected item, as Slint's.
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

/** Slint `PagePreview`: the thumbnail on paper with a 1 dp edge, or a document glyph until the scan draws it. */
@Composable
internal fun PagePreview(thumbnail: String?, modifier: Modifier = Modifier, radius: Dp = 14.dp) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(radius)
    val image by rememberThumbnail(thumbnail)
    BoxWithConstraints(
        modifier.clip(shape).background(c.paper).border(BorderStroke(1.dp, c.paperEdge), shape),
        contentAlignment = Alignment.Center,
    ) {
        val bitmap = image
        if (bitmap != null) {
            Image(bitmap, null, Modifier.fillMaxSize(), contentScale = ContentScale.Fit)
        } else {
            val glyph = minOf(40.dp, maxWidth / 2)
            Icon(painterResource(R.drawable.ic_document), null, Modifier.size(glyph), tint = c.muted)
        }
    }
}

/** Slint `FavoriteBadge`: a 16 dp accent star on a 24 dp card-coloured circle. */
@Composable
private fun FavoriteBadge(modifier: Modifier = Modifier) {
    val c = NotaeTheme.colors
    Box(modifier.testTag(FAVORITE_BADGE_TAG).size(24.dp).clip(CircleShape).background(c.card), Alignment.Center) {
        Icon(painterResource(R.drawable.ic_star_filled), null, Modifier.size(16.dp), tint = c.accent)
    }
}

/** Slint `PositionBadge`: the item's number in its set, on an accent-soft pill at least 28 dp wide. */
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

/** Slint `MoreActions`: the ⋮, named after the score, the only way to its actions sheet. */
@Composable
private fun MoreActions(title: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    IconButton(stringResource(R.string.score_more_actions, title), painterResource(R.drawable.ic_more_vertical), onClick, modifier)
}
