package com.c2yz.notae.ui.details

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.MetadataDraft
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.components.ModalSurface
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.components.TextField
import com.c2yz.notae.ui.components.failureText
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * Slint's "Score details" sheet (app.slint): a card at most 600 × 620 over a scrim that ignores taps, with the title
 * and close, the intro, Title, Composer or artist, Score number and Tags, the core's error in red, then Cancel and
 * Save details at the bottom; it scrolls when the window is shorter. The fields are this composable's own state, filled
 * again whenever the core sends a different draft (the record has loaded); Save sends them, Close and Cancel drop them.
 */
@Composable
fun MetadataEditor(draft: MetadataDraft, onAction: (Action) -> Unit, modifier: Modifier = Modifier) {
    val c = NotaeTheme.colors
    val tagsText = draft.tags.joinToString(", ")
    var title by remember(draft.noteKey, draft.title) { mutableStateOf(draft.title) }
    var composer by remember(draft.noteKey, draft.composer) { mutableStateOf(draft.composer) }
    var number by remember(draft.noteKey, draft.number) { mutableStateOf(draft.number) }
    var tags by remember(draft.noteKey, tagsText) { mutableStateOf(tagsText) }
    val cancel = { onAction(Action.CancelMetadata) }
    val close = remember { FocusRequester() }
    LaunchedEffect(Unit) { close.requestFocus() }
    BoxWithConstraints(modifier.fillMaxSize()) {
        val expanded = maxWidth >= 840.dp
        ModalSurface(
            stringResource(R.string.metadata_dialog),
            onDismiss = null,
            bottomSheet = false,
            cardWidth = 600.dp,
            modifier = Modifier.onPreviewKeyEvent { event ->
                (event.type == KeyEventType.KeyDown && event.key == Key.Escape).also { if (it) cancel() }
            },
            padding = PaddingValues(0.dp),
            spacing = 0.dp,
            cardHeight = 620.dp,
        ) {
            BoxWithConstraints(Modifier.fillMaxSize()) {
                Column(
                    Modifier
                        .fillMaxWidth()
                        .verticalScroll(rememberScrollState())
                        .heightIn(min = maxHeight)
                        .padding(if (expanded) 28.dp else 20.dp),
                    verticalArrangement = Arrangement.SpaceBetween,
                ) {
                    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                        Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text(
                                stringResource(R.string.score_details),
                                Modifier.weight(1f),
                                color = c.foreground,
                                fontSize = 24.sp,
                                fontWeight = FontWeight.SemiBold,
                            )
                            IconButton(stringResource(R.string.dismiss), painterResource(R.drawable.ic_close), cancel, Modifier.focusRequester(close))
                        }
                        Text(stringResource(R.string.metadata_intro), color = c.muted, fontSize = 14.sp)
                        Field(stringResource(R.string.details_title), title, { title = it }, stringResource(R.string.metadata_title_hint))
                        Field(stringResource(R.string.metadata_composer), composer, { composer = it }, stringResource(R.string.metadata_composer_hint))
                        Field(stringResource(R.string.metadata_number), number, { number = it }, stringResource(R.string.metadata_number_hint))
                        Field(stringResource(R.string.details_tags), tags, { tags = it }, stringResource(R.string.metadata_tags_hint))
                        draft.error?.let { Text(failureText(it), color = c.error, fontSize = 14.sp) }
                    }
                    Row(
                        Modifier.fillMaxWidth().padding(top = 16.dp),
                        horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.End),
                    ) {
                        PillButton(stringResource(R.string.metadata_cancel), cancel)
                        PillButton(
                            stringResource(R.string.metadata_save),
                            { onAction(Action.SaveMetadata(draft.noteKey, number, title, composer, tags)) },
                            primary = true,
                        )
                    }
                }
            }
        }
    }
}

/** A field's label, 6 dp above it: 14 sp semi-bold, muted. */
@Composable
private fun Field(label: String, value: String, onValueChange: (String) -> Unit, placeholder: String) {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(label, color = NotaeTheme.colors.muted, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
        TextField(value, onValueChange, placeholder)
    }
}
