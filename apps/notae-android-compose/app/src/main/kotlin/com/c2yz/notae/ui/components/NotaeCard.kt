package com.c2yz.notae.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import com.c2yz.notae.ui.theme.NotaeTheme

/** Slint `Card`: `Theme.card` with a 26 radius. */
@Composable
fun NotaeCard(modifier: Modifier = Modifier, content: @Composable BoxScope.() -> Unit) {
    Box(
        modifier.clip(RoundedCornerShape(26.dp)).background(NotaeTheme.colors.card),
        content = content,
    )
}
