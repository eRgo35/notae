package com.c2yz.notae.ui.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * 0.3.0's `TextField`: one line on the field colour, 52 high, 16 radius, 16 dp side padding, 16 sp text, the muted
 * [placeholder] while empty, and a 2 dp accent border while focused. The placeholder is also its accessible name.
 * With [multiLine] it starts 52 high and grows with its text, up to [MAX_LINES] lines, then scrolls.
 */
@Composable
fun TextField(value: String, onValueChange: (String) -> Unit, placeholder: String, modifier: Modifier = Modifier, multiLine: Boolean = false) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(16.dp)
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    BasicTextField(
        value,
        onValueChange,
        modifier
            .fillMaxWidth()
            .then(if (multiLine) Modifier.heightIn(min = 52.dp) else Modifier.height(52.dp))
            .clip(shape)
            .background(c.field)
            .then(if (focused) Modifier.border(BorderStroke(2.dp, c.accent), shape) else Modifier)
            .semantics { contentDescription = placeholder },
        singleLine = !multiLine,
        maxLines = if (multiLine) MAX_LINES else 1,
        textStyle = TextStyle(color = c.foreground, fontSize = 16.sp),
        cursorBrush = SolidColor(c.accent),
        interactionSource = source,
        decorationBox = { inner ->
            Box(Modifier.padding(horizontal = 16.dp, vertical = if (multiLine) 16.dp else 0.dp), contentAlignment = Alignment.CenterStart) {
                if (value.isEmpty()) {
                    Text(placeholder, color = c.muted, fontSize = 16.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                inner()
            }
        },
    )
}

private const val MAX_LINES = 4
