package com.c2yz.notae.ui.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected as selectedState
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * Slint `Chip`: a 48 dp target holding a 40 dp pill (20 radius, 18 dp side padding) with a 15 sp medium label;
 * the accent fill when selected, the card fill otherwise.
 */
@Composable
fun Chip(label: String, selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(20.dp)
    val ink = if (selected) c.onAccent else c.foreground
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    Box(
        modifier
            .height(48.dp)
            .semantics { selectedState = selected }
            .clickable(source, null, role = Role.Button, onClick = onClick)
            .padding(vertical = 4.dp),
        contentAlignment = Alignment.Center,
    ) {
        Box(
            Modifier
                .height(40.dp)
                .clip(shape)
                .background(if (selected) c.accent else c.card)
                .then(if (focused) Modifier.border(BorderStroke(2.dp, if (selected) c.onAccent else c.accent), shape) else Modifier)
                .padding(horizontal = 18.dp),
            contentAlignment = Alignment.Center,
        ) {
            Text(label, color = ink, fontSize = 15.sp, fontWeight = FontWeight.Medium, maxLines = 1)
        }
    }
}
