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
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke as DrawStroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.TextUnitType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.unit.toSize
import com.c2yz.notae.R
import com.c2yz.notae.ui.components.DialogButtons
import com.c2yz.notae.ui.components.ModalSurface
import com.c2yz.notae.ui.components.NotaeDialog
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.components.TextField
import com.c2yz.notae.ui.sets.closeOnEscape
import com.c2yz.notae.ui.theme.NotaeTheme

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
    private var erasing by mutableStateOf<Overlay?>(null)
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

    /** Drops the line or erasing in progress (a second finger turned it into a pinch). */
    fun cancel() {
        active = null
        erasing = null
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

/** What the page layer asks the viewer to do with text: add a box at a point, or delete one. */
sealed interface TextRequest {
    data class Add(val page: Int, val x: Float, val y: Float) : TextRequest

    data class Delete(val page: Int, val index: Int) : TextRequest
}

/**
 * The drawings over page [page] ([overlay], tagged `drawings-<page>` while [visible]), and, while [session] is set, the
 * input that draws on it. [routeTouch] decides each touch: a stroke, an erasing, or (not consumed) a page move left to
 * the page area. The Text tool taps to add a box, drags a box to move it, and taps one to delete it. [onStylus] is
 * called on every pen touch.
 */
@Composable
fun PageDrawings(
    page: Int,
    overlay: Overlay?,
    session: DrawSession?,
    visible: Boolean,
    stylusSeen: Boolean,
    onStylus: () -> Unit,
    onText: (TextRequest) -> Unit,
) {
    val seen by rememberUpdatedState(stylusSeen)
    val stylus by rememberUpdatedState(onStylus)
    val text by rememberUpdatedState(onText)
    val measurer = rememberTextMeasurer()
    val input = if (session == null) {
        Modifier
    } else {
        Modifier.pointerInput(page, session) {
            awaitEachGesture {
                val down = awaitFirstDown(requireUnconsumed = false)
                if (down.type == PointerType.Stylus || down.type == PointerType.Eraser) stylus()
                val pointers = currentEvent.changes.count { it.pressed }
                val route = routeTouch(down.type, seen, pointers, drawing = true)
                if (route == TouchRoute.MOVE_PAGE) return@awaitEachGesture
                val box = Rect(Offset.Zero, size.toSize())
                if (route == TouchRoute.DRAW && session.tool == DrawTool.TEXT) {
                    down.consume()
                    val at = pageFraction(down.position, box)
                    val hit = textAt(session.overlay(), page, down.position, box, measurer)
                    var last = down.position
                    var moved = false
                    while (true) {
                        val change = awaitPointerEvent().changes.firstOrNull { it.id == down.id } ?: break
                        change.consume()
                        if (!change.pressed) break
                        if ((change.position - down.position).getDistance() > viewConfiguration.touchSlop) moved = true
                        last = change.position
                    }
                    when {
                        hit != null && moved -> {
                            val start = session.overlay().pages.first { it.page == page.toUInt() }.texts[hit]
                            val delta = pageFraction(last, box) - at
                            session.moveText(page, hit, start.x + delta.x, start.y + delta.y)
                        }
                        hit != null -> text(TextRequest.Delete(page, hit))
                        !moved -> text(TextRequest.Add(page, at.x, at.y))
                    }
                    return@awaitEachGesture
                }
                val tool = session.tool
                if (route == TouchRoute.ERASE) session.tool = DrawTool.ERASER
                val start = pageFraction(down.position, box)
                session.begin(page, start.x, start.y)
                down.consume()
                while (true) {
                    val event = awaitPointerEvent()
                    if (down.type == PointerType.Touch && event.changes.count { it.pressed } > 1) {
                        // A second finger: a pinch, not a line.
                        session.cancel()
                        break
                    }
                    val change = event.changes.firstOrNull { it.id == down.id } ?: run { session.end(); null } ?: break
                    change.consume()
                    if (!change.pressed) {
                        session.end()
                        break
                    }
                    val point = pageFraction(change.position, box)
                    session.extend(point.x, point.y)
                }
                session.tool = tool
            }
        }
    }
    Box(Modifier.fillMaxSize().testTag("page-$page").then(input)) {
        val drawn = overlay?.pages?.firstOrNull { it.page == page.toUInt() }
        val active = session?.active?.takeIf { it.first == page }?.second
        if (visible && (drawn != null || active != null)) {
            Canvas(Modifier.fillMaxSize().testTag("drawings-$page")) {
                drawn?.strokes?.forEach { drawStroke(it) }
                active?.let { drawStroke(it) }
                drawn?.texts?.forEach { box ->
                    drawText(measurer, box.text, pagePoint(box.x, box.y, Rect(Offset.Zero, size)), textStyle(box, size.height / fontScale / density))
                }
            }
        }
    }
}

/** A text box's style on a page [pageHeightSp] tall: its size is a fraction of the page height. */
private fun textStyle(box: TextBox, pageHeightSp: Float) =
    TextStyle(color = Color.Black, fontSize = TextUnit(box.size * pageHeightSp, TextUnitType.Sp), fontWeight = FontWeight.Medium)

/** The index of the text box on [page] under [point], if any. */
private fun Density.textAt(overlay: Overlay, page: Int, point: Offset, box: Rect, measurer: TextMeasurer): Int? {
    val texts = overlay.pages.firstOrNull { it.page == page.toUInt() }?.texts ?: return null
    return texts.indices.lastOrNull { index ->
        val text = texts[index]
        val style = textStyle(text, box.height / fontScale / density)
        val measured = measurer.measure(text.text, style, density = this)
        Rect(pagePoint(text.x, text.y, box), measured.size.toSize()).inflate(8.dp.toPx()).contains(point)
    }
}

private fun DrawScope.drawStroke(stroke: Stroke) {
    val points = stroke.points.chunked(2).filter { it.size == 2 }
    if (points.isEmpty()) return
    val path = Path()
    points.forEachIndexed { i, (x, y) ->
        if (i == 0) path.moveTo(x * size.width, y * size.height) else path.lineTo(x * size.width, y * size.height)
    }
    if (points.size == 1) path.lineTo(points[0][0] * size.width + 0.1f, points[0][1] * size.height)
    drawPath(
        path,
        Color(stroke.color.toInt()),
        style = DrawStroke(stroke.width * size.width, cap = StrokeCap.Round, join = StrokeJoin.Round),
    )
}

/**
 * The drawing tools: Pen, Red pen, Highlighter, Eraser and Text (the chosen one filled), which scroll sideways when they
 * do not fit, then Undo, Redo, Clear page and Done, always in view. A [compact] window puts the actions on a row of
 * their own.
 */
@Composable
fun DrawToolBar(session: DrawSession, compact: Boolean, onClearPage: () -> Unit, onDone: () -> Unit, modifier: Modifier = Modifier) {
    val tools = @Composable { toolsModifier: Modifier ->
        Row(toolsModifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for ((tool, label) in listOf(
                DrawTool.PEN_BLACK to R.string.draw_pen,
                DrawTool.PEN_RED to R.string.draw_red_pen,
                DrawTool.HIGHLIGHTER to R.string.draw_highlighter,
                DrawTool.ERASER to R.string.draw_eraser,
                DrawTool.TEXT to R.string.draw_text,
            )) {
                PillButton(stringResource(label), { session.tool = tool }, primary = session.tool == tool, tonal = session.tool != tool)
            }
        }
    }
    val actions = @Composable {
        PillButton(stringResource(R.string.draw_undo), session::undo, enabled = session.canUndo)
        PillButton(stringResource(R.string.draw_redo), session::redo, enabled = session.canRedo)
        PillButton(stringResource(R.string.draw_clear_page), onClearPage)
        PillButton(stringResource(R.string.draw_done), onDone, primary = true)
    }
    val row = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp)
    if (compact) {
        Column(modifier) {
            tools(row)
            Row(row.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) { actions() }
        }
    } else {
        Row(modifier.then(row), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            tools(Modifier.weight(1f))
            actions()
        }
    }
}

/** "Add text": a field, Cancel and Add (disabled while blank). */
@Composable
fun AddTextDialog(onCancel: () -> Unit, onAdd: (String) -> Unit) {
    val c = NotaeTheme.colors
    var text by remember { mutableStateOf("") }
    val field = remember { FocusRequester() }
    LaunchedEffect(Unit) { field.requestFocus() }
    NotaeDialog(stringResource(R.string.draw_add_text), Modifier.closeOnEscape(onCancel)) {
        Text(stringResource(R.string.draw_add_text), color = c.foreground, fontSize = 24.sp, fontWeight = FontWeight.SemiBold)
        TextField(text, { text = it }, stringResource(R.string.draw_text), Modifier.focusRequester(field))
        DialogButtons(stringResource(R.string.set_cancel), onCancel, stringResource(R.string.draw_add), { onAdd(text) }, confirmEnabled = text.isNotBlank())
    }
}

/** A yes/no question before a destructive drawing change: [question], Cancel and [confirm]. */
@Composable
fun ConfirmDrawingDialog(question: String, confirm: String, onCancel: () -> Unit, onConfirm: () -> Unit) {
    val c = NotaeTheme.colors
    ModalSurface(question, onDismiss = onCancel, bottomSheet = false, cardWidth = 480.dp, modifier = Modifier.closeOnEscape(onCancel)) {
        Text(question, color = c.foreground, fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
        DialogButtons(stringResource(R.string.set_cancel), onCancel, confirm, onConfirm, destructive = true)
    }
}
