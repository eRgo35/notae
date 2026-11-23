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
import kotlin.math.roundToInt

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

internal const val MIN_ZOOM = 0.5f
internal const val MAX_ZOOM = 4f

/**
 * Two or more fingers pinch and pan: [onPinch] gets the fingers' centroid, the change of their spread and the centroid's
 * move. It sees each event before the pages and the drawing overlay do and consumes the fingers it uses, so a pinch
 * neither turns a page nor draws, wherever it starts. Pen and eraser pointers are never used or consumed.
 */
internal fun Modifier.pinchZoom(onPinch: (centroid: Offset, zoomChange: Float, pan: Offset) -> Unit): Modifier =
    pointerInput(Unit) {
        awaitEachGesture {
            do {
                val event = awaitPointerEvent(PointerEventPass.Initial)
                val fingers = event.changes.filter { it.type == PointerType.Touch && it.pressed && it.previousPressed }
                if (fingers.size >= 2) {
                    val now = centroid(fingers.map { it.position })
                    val before = centroid(fingers.map { it.previousPosition })
                    val spreadNow = fingers.map { (it.position - now).getDistance() }.average().toFloat()
                    val spreadBefore = fingers.map { (it.previousPosition - before).getDistance() }.average().toFloat()
                    val change = if (spreadBefore > 0f && spreadNow > 0f) spreadNow / spreadBefore else 1f
                    onPinch(now, change, now - before)
                    fingers.forEach { it.consume() }
                }
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

/** A row [shownHeight] tall as a percentage of page [first]'s printed height at [dpi]. */
internal fun printedPercent(source: com.c2yz.notae.platform.PageSource, first: Int, shownHeight: Float, dpi: Float): Int {
    val printed = source.pageWidthPoints(first) / 72f * dpi * source.pageAspect(first)
    return if (printed > 0f) (shownHeight / printed * 100f).roundToInt() else 100
}
