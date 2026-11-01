package com.c2yz.notae.ui.sets

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.DeleteSetConfirmation
import com.c2yz.notae.core.SetDialog
import com.c2yz.notae.core.SetFieldError
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.platform.isoToPickerMillis
import com.c2yz.notae.platform.pickerMillisToIso
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.components.ModalSurface
import com.c2yz.notae.ui.components.NotaeDialog
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.components.TextField
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * The sets' dialogs and sheets, drawn over whichever screen opened them while the snapshot holds them (app.slint):
 * the set dialog, the delete confirmation, the add-to-set sheet and the add picker. Each takes focus when it opens;
 * Escape closes it, as Back does through the core.
 */
@Composable
fun SetOverlays(snapshot: Snapshot, onAction: (Action) -> Unit) {
    snapshot.setDialog?.let { SetDialogCard(it, onAction) }
    snapshot.deleteSet?.let { DeleteSetDialog(it, onAction) }
    snapshot.addToSet?.let { AddToSetSheet(it, onAction) }
    snapshot.addPicker?.let { AddPickerDialog(it, onAction) }
}

/** Escape, as Slint's dialogs read it. */
internal fun KeyEvent.isEscape(): Boolean = type == KeyEventType.KeyDown && key == Key.Escape

/** Closes an overlay on Escape. */
internal fun Modifier.closeOnEscape(close: () -> Unit): Modifier = onPreviewKeyEvent { event -> event.isEscape().also { if (it) close() } }

/**
 * Slint `SetDialog`: "New set" (with its intro) or "Edit set"; Name, Date and Notes; the core's field error in red;
 * Cancel and Save set. The fields are this dialog's own state, filled again when the core sends different values.
 * The date is the locale's, picked with the Material date picker, and goes to the core as ISO. The name takes focus.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SetDialogCard(dialog: SetDialog, onAction: (Action) -> Unit) {
    val c = NotaeTheme.colors
    var name by remember(dialog.setId, dialog.name) { mutableStateOf(dialog.name) }
    var date by remember(dialog.setId, dialog.date) { mutableStateOf(dialog.date) }
    var notes by remember(dialog.setId, dialog.notes) { mutableStateOf(dialog.notes.orEmpty()) }
    var picking by remember { mutableStateOf(false) }
    val nameField = remember { FocusRequester() }
    LaunchedEffect(Unit) { nameField.requestFocus() }
    val cancel = { onAction(Action.CancelSetDialog) }
    NotaeDialog(stringResource(R.string.set_dialog), Modifier.closeOnEscape(cancel)) {
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(
                stringResource(if (dialog.setId == null) R.string.library_new_set else R.string.set_edit),
                color = c.foreground,
                fontSize = 24.sp,
                fontWeight = FontWeight.SemiBold,
            )
            if (dialog.setId == null) Text(stringResource(R.string.set_dialog_intro), color = c.muted, fontSize = 14.sp)
        }
        Field(stringResource(R.string.set_name)) {
            TextField(name, { name = it }, stringResource(R.string.set_name_hint), Modifier.focusRequester(nameField))
        }
        Field(stringResource(R.string.set_date)) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                DateField(date, { picking = true }, Modifier.weight(1f))
                IconButton(stringResource(R.string.set_clear_date), painterResource(R.drawable.ic_close), { date = null }, enabled = date != null)
            }
        }
        Field(stringResource(R.string.set_notes_label)) {
            TextField(notes, { notes = it }, stringResource(R.string.set_notes_hint), multiLine = true)
        }
        dialog.error?.let { Text(fieldError(it), color = c.error, fontSize = 14.sp) }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
            PillButton(stringResource(R.string.set_cancel), cancel)
            PillButton(stringResource(R.string.set_save), { onAction(Action.SaveSetDialog(name, date, notes.ifBlank { null })) }, primary = true)
        }
    }
    if (picking) {
        val state = rememberDatePickerState(initialSelectedDateMillis = date?.let(::isoToPickerMillis))
        DatePickerDialog(
            onDismissRequest = { picking = false },
            confirmButton = {
                TextButton({
                    state.selectedDateMillis?.let { date = pickerMillisToIso(it) }
                    picking = false
                }) { Text(stringResource(android.R.string.ok)) }
            },
            dismissButton = { TextButton({ picking = false }) { Text(stringResource(android.R.string.cancel)) } },
        ) {
            DatePicker(state)
        }
    }
}

/** A dialog field's label, 8 dp above it: 14 sp semi-bold, muted (Slint `DialogLabel`). */
@Composable
private fun Field(label: String, content: @Composable () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(label, color = NotaeTheme.colors.muted, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
        content()
    }
}

/** The date in the locale's format, or the muted "YYYY-MM-DD" hint, styled as a text field; a tap opens the picker. */
@Composable
private fun DateField(iso: String?, onPick: () -> Unit, modifier: Modifier = Modifier) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(16.dp)
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    val label = stringResource(R.string.set_date)
    Box(
        modifier
            .fillMaxWidth()
            .height(52.dp)
            .clip(shape)
            .background(c.field)
            .then(if (focused) Modifier.border(BorderStroke(2.dp, c.accent), shape) else Modifier)
            .semantics { contentDescription = label }
            .clickable(source, null, role = Role.Button, onClick = onPick)
            .padding(horizontal = 16.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        if (iso != null) {
            Text(localDate(iso), color = c.foreground, fontSize = 16.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        } else {
            Text(stringResource(R.string.set_date_hint), color = c.muted, fontSize = 16.sp, maxLines = 1)
        }
    }
}

/** The set dialog's message for rejected fields (texts.rs `set_field_error`). */
@Composable
private fun fieldError(error: SetFieldError): String = when (error) {
    SetFieldError.EMPTY_NAME -> stringResource(R.string.set_error_name)
    SetFieldError.INVALID_DATE -> stringResource(R.string.set_error_date)
}

/** Slint `DeleteSetDialog`: "Delete “name”?", what deleting keeps, then Cancel (focused) and Delete. */
@Composable
private fun DeleteSetDialog(confirmation: DeleteSetConfirmation, onAction: (Action) -> Unit) {
    val c = NotaeTheme.colors
    val cancelButton = remember { FocusRequester() }
    LaunchedEffect(Unit) { cancelButton.requestFocus() }
    val cancel = { onAction(Action.CancelDeleteSet) }
    ModalSurface(
        stringResource(R.string.set_delete_dialog),
        onDismiss = null,
        bottomSheet = false,
        cardWidth = 480.dp,
        modifier = Modifier.closeOnEscape(cancel),
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(stringResource(R.string.set_delete_title, confirmation.name), color = c.foreground, fontSize = 24.sp, fontWeight = FontWeight.SemiBold)
            Text(stringResource(R.string.set_delete_body), color = c.muted, fontSize = 14.sp)
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
            PillButton(stringResource(R.string.set_cancel), cancel, Modifier.focusRequester(cancelButton))
            PillButton(
                stringResource(R.string.set_delete_confirm),
                { onAction(Action.ConfirmDeleteSet) },
                icon = painterResource(R.drawable.ic_delete),
                primary = true,
            )
        }
    }
}
