package com.c2yz.notae.ui.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.ripple
import androidx.compose.material3.Text
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
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * 0.3.0's `PillButton`: 52 high (so already above the 48 dp target), 26 radius, 28/14 padding, 8 spacing,
 * 16 sp semi-bold label, optional 20 dp icon. Primary is the accent fill, tonal the field fill, otherwise transparent.
 */
@Composable
fun PillButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: Painter? = null,
    primary: Boolean = false,
    tonal: Boolean = false,
    enabled: Boolean = true,
) {
    val c = NotaeTheme.colors
    val ink = if (!enabled) c.muted else if (primary) c.onAccent else c.foreground
    val background = if (!enabled) c.field else if (primary) c.accent else if (tonal) c.field else Color.Transparent
    val shape = RoundedCornerShape(26.dp)
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    Row(
        modifier
            .defaultMinSize(minWidth = 52.dp, minHeight = 52.dp)
            .clip(shape)
            .background(background)
            .then(if (focused && enabled) Modifier.border(BorderStroke(2.dp, if (primary) c.onAccent else c.accent), shape) else Modifier)
            .clickable(source, ripple(color = ink), enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(horizontal = 28.dp, vertical = 14.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            Icon(icon, null, Modifier.size(20.dp).alpha(if (enabled) 1f else 0.6f), tint = ink)
        }
        Text(
            text,
            Modifier.alpha(if (enabled) 1f else 0.6f),
            color = ink,
            style = TextStyle(fontFamily = FontFamily.Default, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, textAlign = TextAlign.Center),
        )
    }
}
