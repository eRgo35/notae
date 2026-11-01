package com.c2yz.notae.ui.sets

import androidx.compose.foundation.BorderStroke
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
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Edited
import com.c2yz.notae.core.SetCard
import com.c2yz.notae.core.Subtitle
import com.c2yz.notae.platform.formatIsoDate
import com.c2yz.notae.ui.components.ActionSheet
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.components.NotaeCard
import com.c2yz.notae.ui.components.SheetAction
import com.c2yz.notae.ui.components.toCount
import com.c2yz.notae.ui.library.GridScrollbar
import com.c2yz.notae.ui.library.ListScrollbar
import com.c2yz.notae.ui.theme.NotaeTheme
import kotlin.math.floor

/** 0.3.0's `SetsPane`: the sets as folder cards (grid) or rows (list) in a card, with the scrollbar handle. */
@Composable
internal fun SetsPane(cards: List<SetCard>, list: Boolean, onOpen: (SetCard) -> Unit, onMore: (SetCard) -> Unit, modifier: Modifier = Modifier) {
    NotaeCard(modifier.fillMaxSize()) {
        if (list) {
            val state = rememberLazyListState()
            LazyColumn(Modifier.fillMaxSize().padding(8.dp), state = state) {
                items(cards, key = { it.id }) { card -> SetRow(card, { onOpen(card) }, { onMore(card) }) }
            }
            ListScrollbar(state, Modifier.align(Alignment.TopEnd).padding(vertical = 8.dp))
        } else {
            // 0.3.0's `SetGrid`: 20 dp inset, columns of at least 200 dp with 16 dp gaps, 24 dp between rows, and cards
            // 0.85 times as tall as wide plus 88 dp for the text.
            BoxWithConstraints(Modifier.fillMaxSize()) {
                val inner = maxWidth - 40.dp
                val columns = floor((inner + 16.dp) / 216.dp).toInt().coerceAtLeast(1)
                val cellWidth = (inner - 16.dp * (columns - 1)) / columns
                val state = rememberLazyGridState()
                LazyVerticalGrid(
                    GridCells.Fixed(columns),
                    Modifier.fillMaxSize().padding(horizontal = 20.dp),
                    state = state,
                    contentPadding = PaddingValues(top = 20.dp, bottom = 20.dp),
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                    verticalArrangement = Arrangement.spacedBy(24.dp),
                ) {
                    items(cards, key = { it.id }) { card ->
                        SetFolderCard(card, { onOpen(card) }, { onMore(card) }, Modifier.height(cellWidth * 0.85f + 88.dp))
                    }
                }
                GridScrollbar(state, Modifier.align(Alignment.TopEnd).padding(top = 20.dp))
            }
        }
    }
}

/** 0.3.0's `SetFolderCard`: blank pages leaning out of an accent-soft folder with the count badge, then name, date and edit time. */
@Composable
private fun SetFolderCard(card: SetCard, onOpen: () -> Unit, onMore: () -> Unit, modifier: Modifier = Modifier) {
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
                .background(if (pressed) c.field else Color.Transparent)
                .then(if (focused) Modifier.border(BorderStroke(2.dp, c.accent), shape) else Modifier)
                .setClick(card, source, onOpen)
                .padding(6.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            BoxWithConstraints(Modifier.weight(1f).fillMaxWidth().clip(RoundedCornerShape(18.dp)).background(c.accentSoft)) {
                FolderPage(Modifier.offset(x = maxWidth * 0.2f, y = 30.dp).size(maxWidth * 0.46f, maxHeight))
                FolderPage(Modifier.offset(x = maxWidth * 0.34f, y = 20.dp).size(maxWidth * 0.46f, maxHeight))
                Box(
                    Modifier.align(Alignment.BottomStart).padding(12.dp).height(28.dp).clip(RoundedCornerShape(14.dp)).background(c.card).padding(horizontal = 12.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(itemCount(card.count), color = c.foreground, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
                }
            }
            Column(Modifier.padding(start = 4.dp, end = 44.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(card.name, Modifier.clearAndSetSemantics { }, color = c.foreground, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(setDate(card.date), color = c.muted, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(edited(card.edited), color = c.muted, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        SetActions(card.name, onMore, Modifier.align(Alignment.BottomEnd).padding(2.dp))
    }
}

/** A blank page leaning out of a folder, drawn like the score paper. */
@Composable
private fun FolderPage(modifier: Modifier) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(10.dp)
    Box(modifier.clip(shape).background(c.paper).border(BorderStroke(1.dp, c.paperEdge), shape))
}

/** 0.3.0's `SetRow`: an 80 dp line with the folder, name, "date · N items", the edit time at the right and the ⋮. */
@Composable
private fun SetRow(card: SetCard, onOpen: () -> Unit, onMore: () -> Unit) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(18.dp)
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val focused by source.collectIsFocusedAsState()
    Row(
        Modifier
            .fillMaxWidth()
            .height(80.dp)
            .clip(shape)
            .background(if (pressed) c.field else Color.Transparent)
            .then(if (focused) Modifier.border(BorderStroke(2.dp, c.accent), shape) else Modifier)
            .setClick(card, source, onOpen)
            .padding(start = 16.dp, end = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        FolderTile(52.dp, 14.dp, 26.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(card.name, Modifier.clearAndSetSemantics { }, color = c.foreground, fontSize = 17.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(setDetail(card.date, card.count), color = c.muted, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Text(edited(card.edited), Modifier.width(160.dp), color = c.muted, fontSize = 14.sp, textAlign = TextAlign.End, maxLines = 1, overflow = TextOverflow.Ellipsis)
        SetActions(card.name, onMore)
    }
}

/** The folder glyph on an accent-soft tile, as on set rows, the set sheet and the checklist. */
@Composable
internal fun FolderTile(size: Dp, radius: Dp, glyph: Dp) {
    val c = NotaeTheme.colors
    Box(Modifier.size(size).clip(RoundedCornerShape(radius)).background(c.accentSoft), Alignment.Center) {
        Icon(painterResource(R.drawable.ic_folder), null, Modifier.size(glyph), tint = c.onAccentSoft)
    }
}

/** The card or row's own target: named after the set, read with its date and count, and opening it. */
@Composable
private fun Modifier.setClick(card: SetCard, source: MutableInteractionSource, onOpen: () -> Unit): Modifier {
    val opens = stringResource(R.string.set_opens)
    return semantics { contentDescription = card.name }
        .clickable(source, null, onClickLabel = opens, role = Role.Button, onClick = onOpen)
}

/** 0.3.0's `SetActions`: the set's ⋮, named after it, the way to Edit set and Delete set. */
@Composable
private fun SetActions(name: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    IconButton(stringResource(R.string.score_more_actions, name), painterResource(R.drawable.ic_more_vertical), onClick, modifier)
}

/**
 * The set's ⋮ sheet (0.3.0's `LibraryScreen`'s actions sheet for a set): handle, folder, name and "date · N items",
 * then Edit set and Delete set. Each action closes the sheet; focus starts on Edit set.
 */
@Composable
internal fun SetActionsSheet(card: SetCard, onDismiss: () -> Unit, onAction: (Action) -> Unit) {
    val c = NotaeTheme.colors
    val first = remember { FocusRequester() }
    ActionSheet(
        stringResource(R.string.set_actions_dialog),
        onDismiss,
        padding = PaddingValues(start = 8.dp, top = 8.dp, end = 8.dp, bottom = 16.dp),
        spacing = 0.dp,
    ) {
        Box(Modifier.fillMaxWidth().padding(bottom = 8.dp), Alignment.Center) {
            Box(Modifier.size(32.dp, 4.dp).clip(RoundedCornerShape(2.dp)).background(c.divider))
        }
        Row(
            Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            FolderTile(44.dp, 12.dp, 24.dp)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(card.name, color = c.foreground, fontSize = 18.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(setDetail(card.date, card.count), color = c.muted, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        Box(Modifier.fillMaxWidth().height(1.dp).background(c.divider))
        SheetAction(stringResource(R.string.set_edit), painterResource(R.drawable.ic_edit), { onAction(Action.EditSet(card.id)) }, Modifier.focusRequester(first))
        SheetAction(stringResource(R.string.set_delete), painterResource(R.drawable.ic_delete), { onAction(Action.DeleteSet(card.id)) })
    }
    LaunchedEffect(card.id) { first.requestFocus() }
}

/** A core ISO date in the app's locale ([formatIsoDate]). */
@Composable
internal fun localDate(iso: String): String = formatIsoDate(iso, LocalConfiguration.current.locales[0])

/** A set's date, or "No date" (texts.rs `set_date`). */
@Composable
internal fun setDate(iso: String?): String = iso?.let { localDate(it) } ?: stringResource(R.string.set_no_date)

/** "3 items": the pieces and notes a set holds (texts.rs `item_count`). */
@Composable
internal fun itemCount(count: UInt): String = pluralStringResource(R.plurals.set_items, count.toCount(), count.toLong())

/** "29 Mar 2026 · 9 items", or "No date · 0 items", as on set rows, the set sheet and the checklist. */
@Composable
internal fun setDetail(iso: String?, count: UInt): String = "${setDate(iso)} · ${itemCount(count)}"

/** "Edited today", "Edited yesterday", "Edited 3 days ago" or "Edited 29 Mar 2026" (texts.rs `edited`). */
@Composable
internal fun edited(edited: Edited): String = when (edited) {
    Edited.Today -> stringResource(R.string.set_edited_today)
    Edited.Yesterday -> stringResource(R.string.set_edited_yesterday)
    is Edited.DaysAgo -> pluralStringResource(R.plurals.set_edited_days, edited.days.toCount(), edited.days.toLong())
    is Edited.On -> stringResource(R.string.set_edited_on, localDate(edited.date))
}

/** "29 Mar 2026 · 9 pieces · 1 note": the date and non-zero counts, each only when present (texts.rs `set_subtitle`). */
@Composable
internal fun setSubtitle(subtitle: Subtitle.Set): String = listOfNotNull(
    subtitle.date?.let { localDate(it) },
    subtitle.pieces.takeIf { it > 0u }?.let { pluralStringResource(R.plurals.set_pieces, it.toCount(), it.toLong()) },
    subtitle.notes.takeIf { it > 0u }?.let { pluralStringResource(R.plurals.set_notes, it.toCount(), it.toLong()) },
).joinToString(" · ")
