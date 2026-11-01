package com.c2yz.notae.ui.library

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
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
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.SetEntry
import com.c2yz.notae.ui.components.ActionSheet
import com.c2yz.notae.ui.components.NotaeDialog
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.components.SheetAction
import com.c2yz.notae.ui.components.TextField
import com.c2yz.notae.ui.theme.NotaeTheme

/** What TalkBack reads for a note: "3. Note: text", numbered as a score is. */
@Composable
internal fun noteLabel(note: SetEntry.Note): String = "${note.position}. " + stringResource(R.string.note_label, note.text)

/**
 * A text note in the set's grid: a tinted card the size of a score card with its number, the note glyph and the text.
 * Tapping it does nothing (a note has no details and nothing to open); its ⋮ holds Edit note and Remove from set.
 */
@Composable
internal fun NoteCard(note: SetEntry.Note, onMore: () -> Unit, modifier: Modifier = Modifier) {
    val c = NotaeTheme.colors
    val label = noteLabel(note)
    Box(modifier.fillMaxWidth()) {
        Column(
            Modifier
                .fillMaxSize()
                .clip(RoundedCornerShape(20.dp))
                .background(c.note)
                .clearAndSetSemantics { contentDescription = label }
                .padding(start = 14.dp, top = 14.dp, end = 14.dp, bottom = 52.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
                PositionBadge(note.position)
                Icon(painterResource(R.drawable.ic_note), null, Modifier.size(24.dp), tint = c.onNote)
            }
            Text(note.text, color = c.onNote, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, overflow = TextOverflow.Ellipsis)
        }
        MoreActions(note.text, onMore, Modifier.align(Alignment.BottomEnd).padding(2.dp))
    }
}

/** A text note in the set's list: an 80 dp tinted row with its number, the note glyph, the text and the ⋮. */
@Composable
internal fun NoteRow(note: SetEntry.Note, onMore: () -> Unit, modifier: Modifier = Modifier) {
    val c = NotaeTheme.colors
    val label = noteLabel(note)
    Row(
        modifier
            .fillMaxWidth()
            .height(80.dp)
            .clip(RoundedCornerShape(18.dp))
            .background(c.note)
            .padding(start = 14.dp, end = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(
            Modifier.weight(1f).clearAndSetSemantics { contentDescription = label },
            horizontalArrangement = Arrangement.spacedBy(14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            PositionBadge(note.position)
            NoteTile(44.dp, 58.dp)
            NoteText(note.text, Modifier.weight(1f))
        }
        MoreActions(note.text, onMore)
    }
}

/** The note glyph on a tinted tile, where a score shows its page. */
@Composable
internal fun NoteTile(width: Dp, height: Dp) {
    val c = NotaeTheme.colors
    Box(Modifier.size(width, height).clip(RoundedCornerShape(6.dp)).background(c.note), Alignment.Center) {
        Icon(painterResource(R.drawable.ic_note), null, Modifier.size(24.dp), tint = c.onNote)
    }
}

/** A note's text in a row: two lines at most, in the note ink. */
@Composable
internal fun NoteText(text: String, modifier: Modifier = Modifier) {
    Text(text, modifier, color = NotaeTheme.colors.onNote, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, maxLines = 2, overflow = TextOverflow.Ellipsis)
}

/** The note's ⋮ sheet: handle, glyph and text, then Edit note and Remove from set. Focus starts on Edit note. */
@Composable
internal fun NoteActionsSheet(note: SetEntry.Note, onDismiss: () -> Unit, onEdit: () -> Unit, onRemove: () -> Unit) {
    val c = NotaeTheme.colors
    val first = remember { FocusRequester() }
    ActionSheet(
        stringResource(R.string.note_actions_dialog),
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
            NoteTile(44.dp, 58.dp)
            Text(note.text, Modifier.weight(1f), color = c.foreground, fontSize = 18.sp, fontWeight = FontWeight.SemiBold, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
        Box(Modifier.fillMaxWidth().height(1.dp).background(c.divider))
        SheetAction(stringResource(R.string.note_edit), painterResource(R.drawable.ic_edit), onEdit, Modifier.focusRequester(first))
        SheetAction(stringResource(R.string.score_remove_from_set), painterResource(R.drawable.ic_delete), onRemove)
    }
    LaunchedEffect(note.itemId) { first.requestFocus() }
}

/**
 * Edit note: the note's text in a multi-line field (focused), Cancel and Save note, which stays disabled while the text
 * is blank (the core ignores a blank note). The text is this dialog's own state, set from the note once when it opens,
 * so a snapshot arriving while the owner types never resets it.
 */
@Composable
internal fun NoteEditor(note: SetEntry.Note, onCancel: () -> Unit, onSave: (String) -> Unit) {
    val c = NotaeTheme.colors
    var text by remember(note.itemId) { mutableStateOf(note.text) }
    val field = remember { FocusRequester() }
    LaunchedEffect(note.itemId) { field.requestFocus() }
    NotaeDialog(stringResource(R.string.note_edit)) {
        Text(stringResource(R.string.note_edit), color = c.foreground, fontSize = 24.sp, fontWeight = FontWeight.SemiBold)
        TextField(text, { text = it }, stringResource(R.string.note_field), Modifier.focusRequester(field), multiLine = true)
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
            PillButton(stringResource(R.string.set_cancel), onCancel)
            PillButton(stringResource(R.string.note_save), { onSave(text) }, primary = true, enabled = text.isNotBlank())
        }
    }
}
