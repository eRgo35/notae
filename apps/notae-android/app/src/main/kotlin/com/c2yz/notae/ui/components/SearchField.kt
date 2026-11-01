package com.c2yz.notae.ui.components

import android.content.res.Configuration
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
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
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * 0.3.0's `SearchField`: a 52 dp pill on the field colour with the search glyph, the muted [placeholder] while empty
 * (also its accessible name), a 2 dp accent border while focused, the clear (×) button while there is text, and a
 * "Ctrl K" hint while empty when a hardware keyboard is attached.
 *
 * [query] is the core's text. The field keeps its own text and cursor and sends every edit to [onQueryChange]. The
 * core echoes each edit in a later snapshot, so an echo of an edit already made (even an older one, when typing
 * outruns the snapshots) changes nothing; any other query, such as a voice result, replaces the text.
 */
@Composable
fun SearchField(
    query: String,
    onQueryChange: (String) -> Unit,
    placeholder: String,
    focusRequester: FocusRequester,
    modifier: Modifier = Modifier,
) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(26.dp)
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    var value by remember { mutableStateOf(TextFieldValue(query, TextRange(query.length))) }
    // The edits sent and not yet echoed, oldest first.
    val unechoed = remember { ArrayDeque<String>() }
    LaunchedEffect(query) {
        val echoed = unechoed.indexOf(query)
        if (echoed >= 0) {
            repeat(echoed + 1) { unechoed.removeFirst() }
        } else if (query != value.text) {
            unechoed.clear()
            value = TextFieldValue(query, TextRange(query.length))
        }
    }
    val edit = { next: TextFieldValue ->
        val changed = next.text != value.text
        value = next
        if (changed) {
            unechoed.addLast(next.text)
            onQueryChange(next.text)
        }
    }
    val configuration = LocalConfiguration.current
    val hardwareKeyboard = configuration.keyboard == Configuration.KEYBOARD_QWERTY &&
        configuration.hardKeyboardHidden == Configuration.HARDKEYBOARDHIDDEN_NO
    val empty = value.text.isEmpty() && value.composition == null
    Box(
        modifier
            .fillMaxWidth()
            .height(52.dp)
            .clip(shape)
            .background(c.field)
            .then(if (focused) Modifier.border(BorderStroke(2.dp, c.accent), shape) else Modifier),
        contentAlignment = Alignment.CenterStart,
    ) {
        Icon(painterResource(R.drawable.ic_search), null, Modifier.padding(start = 16.dp).size(24.dp), tint = c.muted)
        BasicTextField(
            value,
            edit,
            Modifier
                .fillMaxSize()
                .padding(start = 52.dp, end = if (!empty) 56.dp else if (hardwareKeyboard) 88.dp else 16.dp)
                .focusRequester(focusRequester)
                .semantics { contentDescription = placeholder },
            singleLine = true,
            textStyle = TextStyle(color = c.foreground, fontSize = 16.sp),
            cursorBrush = SolidColor(c.accent),
            interactionSource = source,
            decorationBox = { inner ->
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.CenterStart) {
                    if (empty) {
                        Text(placeholder, color = c.muted, fontSize = 16.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                    inner()
                }
            },
        )
        if (!empty) {
            IconButton(
                stringResource(R.string.search_clear),
                painterResource(R.drawable.ic_close),
                {
                    edit(TextFieldValue(""))
                    focusRequester.requestFocus()
                },
                Modifier.align(Alignment.CenterEnd).padding(end = 4.dp),
            )
        } else if (hardwareKeyboard) {
            Box(
                Modifier
                    .align(Alignment.CenterEnd)
                    .padding(end = 12.dp)
                    .size(64.dp, 28.dp)
                    .clip(RoundedCornerShape(6.dp))
                    .background(c.card)
                    .border(BorderStroke(1.dp, c.divider), RoundedCornerShape(6.dp)),
                contentAlignment = Alignment.Center,
            ) {
                Text(CTRL_K, color = c.muted, fontSize = 12.sp)
            }
        }
    }
}

private const val CTRL_K = "Ctrl K"
