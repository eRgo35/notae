package com.c2yz.notae.ui.viewer

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.input.pointer.PointerType
import com.c2yz.notae.core.Overlay
import com.c2yz.notae.core.PageOverlay
import com.c2yz.notae.core.Stroke
import com.c2yz.notae.core.TextBox
import com.c2yz.notae.core.Tool
import kotlin.math.hypot

/** The drawing tools. */
enum class DrawTool { PEN_BLACK, PEN_RED, HIGHLIGHTER, ERASER, TEXT }

/** Where a touch in the viewer goes. */
enum class TouchRoute { DRAW, ERASE, MOVE_PAGE }

private const val PEN_WIDTH = 0.003f
private const val HIGHLIGHTER_WIDTH = 0.02f
private const val ERASER_RADIUS = 0.01f
private const val TEXT_SIZE = 0.025f
private const val BLACK = 0xFF000000u
private const val RED = 0xFFD32F2Fu
private const val YELLOW = 0x66FFEB3Bu

/**
 * Where a touch goes: outside drawing mode it moves the page; in it the pen draws and its eraser end erases. A finger
 * moves the page once a stylus has been seen (so a resting palm never draws); before that one finger draws and two
 * move the page.
 */
fun routeTouch(type: PointerType, stylusSeen: Boolean, pointers: Int, drawing: Boolean): TouchRoute = when {
    !drawing -> TouchRoute.MOVE_PAGE
    type == PointerType.Eraser -> TouchRoute.ERASE
    type == PointerType.Stylus -> TouchRoute.DRAW
    stylusSeen || pointers > 1 -> TouchRoute.MOVE_PAGE
    else -> TouchRoute.DRAW
}

/** The point at page fractions ([x], [y]) inside [box]. */
fun pagePoint(x: Float, y: Float, box: Rect): Offset = Offset(box.left + x * box.width, box.top + y * box.height)

/** [point] inside [box] as page fractions. */
fun pageFraction(point: Offset, box: Rect): Offset =
    Offset((point.x - box.left) / box.width.coerceAtLeast(1f), (point.y - box.top) / box.height.coerceAtLeast(1f))

/**
 * One drawing session on a set item's overlay: the chosen tool, the stroke being drawn, and undo/redo over every
 * change. Each change makes a new immutable [Overlay]; [overlay] is the current one, which the viewer saves when the
 * session [changed].
 */
class DrawSession(initial: Overlay?, private val contentKey: String) {
    var tool by mutableStateOf(DrawTool.PEN_BLACK)

    private val history = mutableListOf(initial?.copy(contentKey = contentKey) ?: Overlay(contentKey, emptyList()))
    private var index by mutableIntStateOf(0)

    /** The line being drawn, and its page; not yet in [history]. */
    var active by mutableStateOf<Pair<Int, Stroke>?>(null)
        private set
    private var erasing: Overlay? = null
    private var erasePage = 0

    val canUndo: Boolean get() = index > 0
    val canRedo: Boolean get() = index < history.size - 1

    /** Whether the drawings differ from what the session started with. */
    val changed: Boolean get() = index > 0

    /** The current drawings, the line being drawn not included. */
    fun overlay(): Overlay = erasing ?: history[index]

    fun begin(page: Int, x: Float, y: Float) {
        when (tool) {
            DrawTool.ERASER -> {
                erasing = history[index]
                erasePage = page
                erase(x, y)
            }
            DrawTool.TEXT -> Unit
            else -> active = page to Stroke(strokeTool(), strokeColor(), strokeWidth(), listOf(x, y))
        }
    }

    fun extend(x: Float, y: Float) {
        if (erasing != null) return erase(x, y)
        active = active?.let { (page, stroke) -> page to stroke.copy(points = stroke.points + listOf(x, y)) }
    }

    fun end() {
        erasing?.let { erased ->
            erasing = null
            if (erased != history[index]) commit(erased)
        }
        active?.let { (page, stroke) ->
            active = null
            commit(edit(history[index], page) { it.copy(strokes = it.strokes + stroke) })
        }
    }

    fun addText(page: Int, x: Float, y: Float, text: String) {
        if (text.isBlank()) return
        commit(edit(history[index], page) { it.copy(texts = it.texts + TextBox(x, y, text, TEXT_SIZE)) })
    }

    fun moveText(page: Int, textIndex: Int, x: Float, y: Float) =
        commit(edit(history[index], page) { it.copy(texts = it.texts.mapIndexed { i, box -> if (i == textIndex) box.copy(x = x, y = y) else box }) })

    fun deleteText(page: Int, textIndex: Int) =
        commit(edit(history[index], page) { it.copy(texts = it.texts.filterIndexed { i, _ -> i != textIndex }) })

    fun clearPage(page: Int) = commit(edit(history[index], page) { it.copy(strokes = emptyList(), texts = emptyList()) })

    fun undo() {
        if (canUndo) index--
    }

    fun redo() {
        if (canRedo) index++
    }

    private fun commit(next: Overlay) {
        if (next == history[index]) return
        while (history.size > index + 1) history.removeAt(history.size - 1)
        history += next
        index = history.size - 1
    }

    private fun erase(x: Float, y: Float) {
        val current = erasing ?: return
        erasing = edit(current, erasePage) { page -> page.copy(strokes = page.strokes.filterNot { touches(it, x, y) }) }
    }

    private fun strokeTool() = if (tool == DrawTool.HIGHLIGHTER) Tool.HIGHLIGHTER else Tool.PEN

    private fun strokeColor() = when (tool) {
        DrawTool.PEN_RED -> RED
        DrawTool.HIGHLIGHTER -> YELLOW
        else -> BLACK
    }

    private fun strokeWidth() = if (tool == DrawTool.HIGHLIGHTER) HIGHLIGHTER_WIDTH else PEN_WIDTH
}

/** [overlay] with page [page] changed by [change]; a page left empty is dropped. */
private fun edit(overlay: Overlay, page: Int, change: (PageOverlay) -> PageOverlay): Overlay {
    val number = page.toUInt()
    val old = overlay.pages.firstOrNull { it.page == number } ?: PageOverlay(number, emptyList(), emptyList())
    val new = change(old)
    val others = overlay.pages.filter { it.page != number }
    val pages = if (new.strokes.isEmpty() && new.texts.isEmpty()) others else (others + new).sortedBy { it.page }
    return overlay.copy(pages = pages)
}

/** Whether the eraser at ([x], [y]) is within its radius of any segment of [stroke]. */
private fun touches(stroke: Stroke, x: Float, y: Float): Boolean {
    val points = stroke.points.chunked(2).filter { it.size == 2 }.map { Offset(it[0], it[1]) }
    if (points.size == 1) return distance(points[0], points[0], x, y) <= ERASER_RADIUS
    return points.zipWithNext().any { (a, b) -> distance(a, b, x, y) <= ERASER_RADIUS }
}

/** The distance from ([x], [y]) to the segment [a]–[b]. */
private fun distance(a: Offset, b: Offset, x: Float, y: Float): Float {
    val dx = b.x - a.x
    val dy = b.y - a.y
    val length = dx * dx + dy * dy
    val t = if (length == 0f) 0f else (((x - a.x) * dx + (y - a.y) * dy) / length).coerceIn(0f, 1f)
    return hypot(x - (a.x + t * dx), y - (a.y + t * dy))
}
