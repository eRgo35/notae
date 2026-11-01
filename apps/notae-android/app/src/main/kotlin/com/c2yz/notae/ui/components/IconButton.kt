package com.c2yz.notae.ui.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.ripple
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.c2yz.notae.ui.theme.NotaeTheme

/** 0.3.0's `IconButton`: a 48 dp circle (already the minimum target) with a 24 dp icon. [label] is its accessible name. */
@Composable
fun IconButton(
    label: String,
    icon: Painter,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    selected: Boolean = false,
    enabled: Boolean = true,
) {
    val c = NotaeTheme.colors
    val ink = if (selected) c.onAccentSoft else c.foreground
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    Box(
        modifier
            .size(48.dp)
            .clip(CircleShape)
            .background(if (selected && enabled) c.accentSoft else Color.Transparent)
            .then(if (focused && enabled) Modifier.border(BorderStroke(2.dp, c.accent), CircleShape) else Modifier)
            .semantics { contentDescription = label }
            .clickable(source, ripple(color = ink), enabled = enabled, role = Role.Button, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, null, Modifier.size(24.dp).alpha(if (enabled) 1f else 0.38f), tint = ink)
    }
}
