package com.c2yz.notae.ui.components

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp

/**
 * A row of chips that scrolls sideways and fades out over its last 24 dp while more chips lie beyond the end, so a
 * long row (Polish labels on a phone) reads as one that scrolls rather than one cut off.
 */
@Composable
fun Modifier.scrollWithEndFade(): Modifier {
    val state = rememberScrollState()
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    return this
        .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
        .drawWithContent {
            drawContent()
            if (state.canScrollForward) {
                val fade = 24.dp.toPx()
                val (from, to) = if (rtl) 0f to fade else size.width - fade to size.width
                val colors = if (rtl) listOf(Color.Black, Color.Transparent) else listOf(Color.Transparent, Color.Black)
                drawRect(Brush.horizontalGradient(colors, startX = from, endX = to), blendMode = BlendMode.DstOut)
            }
        }
        .horizontalScroll(state)
}
