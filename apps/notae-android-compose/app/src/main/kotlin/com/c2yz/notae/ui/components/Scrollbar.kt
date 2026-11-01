package com.c2yz.notae.ui.components

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import com.c2yz.notae.R
import com.c2yz.notae.ui.theme.NotaeTheme
import com.c2yz.notae.ui.theme.Tokens
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlin.math.roundToInt

/**
 * Slint `Scrollbar` for a lazy grid or list: a thin thumb on the right edge of a 48 dp track that shows while the
 * content scrolls or the thumb is dragged and fades 1.2 s later. Dragging it scrolls to the matching item
 * ([onScrollTo]). Positions are in items: [first] visible of [total], [visible] on screen.
 */
@Composable
fun Scrollbar(
    first: Int,
    visible: Int,
    total: Int,
    scrolling: Boolean,
    onScrollTo: suspend (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (total <= visible || visible <= 0) return
    val c = NotaeTheme.colors
    val scope = rememberCoroutineScope()
    var dragging by remember { mutableStateOf(false) }
    var active by remember { mutableStateOf(false) }
    LaunchedEffect(scrolling, dragging, first) {
        if (scrolling || dragging) {
            active = true
        } else if (active) {
            delay(1200)
            active = false
        }
    }
    val shown = active || dragging
    val label = stringResource(R.string.scroll_position)
    val range = total - visible
    val fraction = (first.toFloat() / range).coerceIn(0f, 1f)
    val currentFraction by rememberUpdatedState(fraction)
    BoxWithConstraints(modifier.width(48.dp).fillMaxHeight()) {
        val thumb = (maxHeight * (visible.toFloat() / total)).coerceAtLeast(48.dp).coerceAtMost(maxHeight)
        val travel = maxHeight - thumb
        val travelPx = with(LocalDensity.current) { travel.toPx() }
        var dragY by remember { mutableFloatStateOf(0f) }
        Box(
            Modifier
                .offset { IntOffset(0, (travelPx * fraction).roundToInt()) }
                .width(48.dp)
                .height(thumb)
                .semantics {
                    contentDescription = label
                    progressBarRangeInfo = ProgressBarRangeInfo(fraction, 0f..1f)
                }
                // Hidden, the thumb takes no touches, so the ⋮ buttons beneath it stay tappable (Slint `enabled: root.shown`).
                .then(
                    if (shown && travelPx > 0f) {
                        Modifier.pointerInput(travelPx, range) {
                            detectVerticalDragGestures(
                                onDragStart = {
                                    dragging = true
                                    dragY = travelPx * currentFraction
                                },
                                onDragEnd = { dragging = false },
                                onDragCancel = { dragging = false },
                            ) { change, delta ->
                                change.consume()
                                dragY = (dragY + delta).coerceIn(0f, travelPx)
                                val target = (dragY / travelPx * range).roundToInt()
                                scope.launch { onScrollTo(target) }
                            }
                        }
                    } else {
                        Modifier
                    },
                ),
        ) {
            val width by animateDpAsState(if (dragging) 8.dp else 4.dp, tween(Tokens.motionShortMillis), label = "thumb width")
            val alpha by animateFloatAsState(if (shown) 1f else 0f, tween(Tokens.motionShortMillis), label = "thumb fade")
            Box(
                Modifier
                    .align(Alignment.TopEnd)
                    .padding(end = 4.dp, top = 6.dp, bottom = 6.dp)
                    .width(width)
                    .fillMaxHeight()
                    .alpha(alpha)
                    .clip(CircleShape)
                    .background(if (dragging) c.accent else c.muted),
            )
        }
    }
}
