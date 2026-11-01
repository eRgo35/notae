package com.c2yz.notae.ui.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected as selectedState
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * 0.3.0's `NavRow`: 52 high (above the 48 dp target), 18 radius, 14 padding and spacing, optional 22 dp icon,
 * 16 sp label (semi-bold when selected) and an optional 14 sp count.
 */
@Composable
fun SidebarItem(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: Painter? = null,
    count: Int? = null,
    selected: Boolean = false,
) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(18.dp)
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val focused by source.collectIsFocusedAsState()
    val ink = if (selected) c.onAccentSoft else c.foreground
    Row(
        modifier
            .fillMaxWidth()
            .height(52.dp)
            .clip(shape)
            .background(if (selected) c.accentSoft else if (pressed) c.field else Color.Transparent)
            .then(if (focused) Modifier.border(BorderStroke(2.dp, c.accent), shape) else Modifier)
            .semantics { this.selectedState = selected }
            .clickable(source, null, role = Role.Button, onClick = onClick)
            .padding(horizontal = 14.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) Icon(icon, null, Modifier.size(22.dp), tint = ink)
        Text(
            label,
            Modifier.weight(1f),
            color = ink,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            style = TextStyle(fontFamily = FontFamily.Default, fontSize = 16.sp, fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal),
        )
        if (count != null) {
            Text(
                count.toString(),
                color = if (selected) c.onAccentSoft else c.muted,
                style = TextStyle(fontFamily = FontFamily.Default, fontSize = 14.sp),
            )
        }
    }
}
