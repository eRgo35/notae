package com.c2yz.notae.ui.viewer

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.PointerType
import androidx.compose.ui.input.pointer.pointerInput

/**
 * The pinch zoom on top of the chosen scale: [factor] 1 is the chosen scale itself; [offset] is how far content larger
 * than the area is panned from its top-left corner (zero at the top-left).
 */
class PageZoom {
    var factor by mutableFloatStateOf(1f)
    var offset by mutableStateOf(Offset.Zero)

    /** The first page on screen as a percentage of its printed size, kept up to date by the page area. */
    var percent by mutableIntStateOf(100)

    fun reset() {
        factor = 1f
        offset = Offset.Zero
    }
}


/**
 * Two or more fingers pinch and pan: [onPinch] gets the fingers' centroid, the change of their spread and the centroid's
 * move. It sees each event before the pages and the drawing overlay do and, once a second finger is down, consumes
 * every finger until all are lifted, so a pinch or a two-finger tap neither turns a page nor draws, wherever it starts.
 * Pen and eraser pointers are never used or consumed, and while one is down nothing pinches.
 */
internal fun Modifier.pinchZoom(onPinch: (centroid: Offset, zoomChange: Float, pan: Offset) -> Unit): Modifier =
    // Keyed on the handler, so a pinch after the area changed (rotation, the notes panel) uses the new sizes.
    pointerInput(onPinch) {
        awaitEachGesture {
            var pinching = false
            do {
                val event = awaitPointerEvent(PointerEventPass.Initial)
                // While the pen is down, the hand resting on the glass moves nothing.
                if (event.changes.any { it.type != PointerType.Touch && it.type != PointerType.Mouse && it.pressed }) continue
                val touches = event.changes.filter { it.type == PointerType.Touch }
                if (touches.count { it.pressed } >= 2) pinching = true
                val fingers = touches.filter { it.pressed && it.previousPressed }
                if (fingers.size >= 2) {
                    val now = centroid(fingers.map { it.position })
                    val before = centroid(fingers.map { it.previousPosition })
                    val spreadNow = fingers.map { (it.position - now).getDistance() }.average().toFloat()
                    val spreadBefore = fingers.map { (it.previousPosition - before).getDistance() }.average().toFloat()
                    val change = if (spreadBefore > 0f && spreadNow > 0f) spreadNow / spreadBefore else 1f
                    onPinch(now, change, now - before)
                }
                if (pinching) touches.forEach { it.consume() }
            } while (event.changes.any { it.pressed })
        }
    }

private fun centroid(points: List<Offset>): Offset =
    points.fold(Offset.Zero) { sum, point -> sum + point } / points.size.toFloat()

/**
 * The pan, from the top-left, of content [size] wide or tall in an area [area] long, clamped to the content's edges;
 * zero when the content fits.
 */
internal fun clampPan(pan: Float, size: Float, area: Float): Float = if (size <= area) 0f else pan.coerceIn(0f, size - area)

/** Where content [size] long starts in an area [area] long: centred when it fits, else moved left by [pan]. */
internal fun contentStart(size: Float, area: Float, pan: Float): Float = if (size <= area) (area - size) / 2f else -pan

/**
 * The pinch factors for a row [baseHeight] tall at the chosen scale: from half its [fitHeight] (fit page) to four times
 * its [printedHeight] (100 %), and always the chosen scale itself.
 */
internal fun zoomBounds(baseHeight: Float, fitHeight: Float, printedHeight: Float): ClosedFloatingPointRange<Float> {
    if (baseHeight <= 0f) return 1f..1f
    return minOf(1f, fitHeight / 2f / baseHeight)..maxOf(1f, 4f * printedHeight / baseHeight)
}
