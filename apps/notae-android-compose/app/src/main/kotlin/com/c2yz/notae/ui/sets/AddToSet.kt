package com.c2yz.notae.ui.sets

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.selection.toggleable
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
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.AddPicker
import com.c2yz.notae.core.AddToSetSheet
import com.c2yz.notae.core.PickerRow
import com.c2yz.notae.core.SetChoice
import com.c2yz.notae.ui.components.ActionSheet
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.components.ModalSurface
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.components.SearchField
import com.c2yz.notae.ui.components.toCount
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * Slint `AddToSetSheet`: "Add to set…" and the piece's title, a checkbox row per set (about six show, more scroll), then
 * New set. The core applies each tap at once, so there is nothing to confirm. The sheet takes focus when it opens.
 */
@Composable
internal fun AddToSetSheet(sheet: AddToSetSheet, onAction: (Action) -> Unit) {
    val c = NotaeTheme.colors
    val close = { onAction(Action.CloseAddToSet) }
    val keys = remember { FocusRequester() }
    LaunchedEffect(Unit) { keys.requestFocus() }
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val listHeight = minOf(64.dp * sheet.choices.size, maxOf(64.dp, maxHeight - 280.dp))
        ActionSheet(stringResource(R.string.add_to_set_dialog), close, Modifier.closeOnEscape(close)) {
            Column(Modifier.focusRequester(keys).focusable(), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(stringResource(R.string.score_add_to_set), color = c.foreground, fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
                Text(sheet.title, color = c.muted, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            Column {
                LazyColumn(Modifier.heightIn(max = listHeight)) {
                    items(sheet.choices, key = { it.id }) { choice ->
                        SetChoiceRow(choice) { onAction(Action.ToggleSetMembership(choice.id, sheet.noteKey)) }
                    }
                }
                NewSetRow { onAction(Action.AddToSetNewSet) }
            }
        }
    }
}

/** Slint `SetChoiceRow`: a 64 dp checkbox row with the folder, name, "date · N items" and the tick. */
@Composable
private fun SetChoiceRow(choice: SetChoice, onToggle: () -> Unit) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(16.dp)
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val focused by source.collectIsFocusedAsState()
    Row(
        Modifier
            .fillMaxWidth()
            .height(64.dp)
            .clip(shape)
            .background(if (pressed) c.field else Color.Transparent)
            .then(if (focused) Modifier.border(BorderStroke(2.dp, c.accent), shape) else Modifier)
            .toggleable(choice.checked, source, null, role = Role.Checkbox, onValueChange = { onToggle() })
            .padding(horizontal = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        FolderTile(40.dp, 12.dp, 24.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(choice.name, color = c.foreground, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(setDetail(choice.date, choice.count), color = c.muted, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        CheckBox(choice.checked)
    }
}

/** Slint `CheckBox`: a 24 dp tick box, accent and ticked, or a muted outline. */
@Composable
private fun CheckBox(checked: Boolean) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(6.dp)
    Box(
        Modifier
            .size(24.dp)
            .clip(shape)
            .then(if (checked) Modifier.background(c.accent) else Modifier.border(BorderStroke(2.dp, c.muted), shape)),
        Alignment.Center,
    ) {
        if (checked) Icon(painterResource(R.drawable.ic_check), null, Modifier.size(20.dp), tint = c.onAccent)
    }
}

/** Slint `NewSetRow`: the checklist's last row, a new set that will hold the piece. */
@Composable
private fun NewSetRow(onClick: () -> Unit) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(16.dp)
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val focused by source.collectIsFocusedAsState()
    Row(
        Modifier
            .fillMaxWidth()
            .height(56.dp)
            .clip(shape)
            .background(if (pressed) c.field else Color.Transparent)
            .then(if (focused) Modifier.border(BorderStroke(2.dp, c.accent), shape) else Modifier)
            .clickable(source, null, role = Role.Button, onClick = onClick)
            .padding(start = 24.dp, end = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(24.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(painterResource(R.drawable.ic_add), null, Modifier.size(24.dp), tint = c.accentText)
        Text(stringResource(R.string.library_new_set), color = c.accentText, fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
    }
}

/**
 * Slint `AddPiecesDialog`: "Pieces", its own search field (focused), the library's rows or the hits with an add/added
 * button each, or "No scores here", then "N added at the end of the set" and Done. A tap outside closes it, as Done.
 * The field is the picker's, not a second app search: it filters what this dialog adds and never leaves it.
 */
@Composable
internal fun AddPickerDialog(picker: AddPicker, onAction: (Action) -> Unit) {
    val c = NotaeTheme.colors
    val close = { onAction(Action.CloseAddPicker) }
    val field = remember { FocusRequester() }
    LaunchedEffect(Unit) { field.requestFocus() }
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val listHeight = maxOf(144.dp, minOf(432.dp, maxHeight - 340.dp))
        ModalSurface(
            stringResource(R.string.picker_dialog),
            onDismiss = close,
            bottomSheet = false,
            cardWidth = 720.dp,
            modifier = Modifier.closeOnEscape(close),
        ) {
            Text(stringResource(R.string.picker_title), color = c.foreground, fontSize = 24.sp, fontWeight = FontWeight.SemiBold)
            SearchField(picker.query, { onAction(Action.AddPickerSearch(it)) }, stringResource(R.string.search_placeholder), field)
            Box(Modifier.fillMaxWidth().height(listHeight)) {
                if (picker.rows.isEmpty()) {
                    Text(stringResource(R.string.library_empty_composer), Modifier.align(Alignment.Center), color = c.muted, fontSize = 15.sp)
                } else {
                    LazyColumn(Modifier.fillMaxSize()) {
                        items(picker.rows, key = { it.noteKey }) { row ->
                            PickerEntry(row) { onAction(Action.ToggleAddPickerPiece(row.noteKey)) }
                        }
                    }
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(
                    pluralStringResource(R.plurals.picker_added, picker.added.toCount(), picker.added.toLong()),
                    Modifier.weight(1f),
                    color = c.muted,
                    fontSize = 14.sp,
                )
                PillButton(stringResource(R.string.picker_done), close, primary = true)
            }
        }
    }
}

/** Slint `PickerEntry`: a 72 dp row with the document tile, title, "number · composer · already in this set" and add/added. */
@Composable
private fun PickerEntry(row: PickerRow, onToggle: () -> Unit) {
    val c = NotaeTheme.colors
    val detail = listOfNotNull(row.number, row.composer, if (row.inSet) stringResource(R.string.picker_in_set) else null).joinToString(" · ")
    Row(
        Modifier.fillMaxWidth().height(72.dp).padding(start = 12.dp, end = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(40.dp).clip(RoundedCornerShape(10.dp)).background(c.field), Alignment.Center) {
            Icon(painterResource(R.drawable.ic_document), null, Modifier.size(24.dp), tint = c.foreground)
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(row.title, color = c.foreground, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (detail.isNotEmpty()) Text(detail, color = c.muted, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        IconButton(
            stringResource(if (row.added) R.string.picker_remove else R.string.picker_add, row.title),
            painterResource(if (row.added) R.drawable.ic_check else R.drawable.ic_add),
            onToggle,
            selected = row.added,
        )
    }
}
