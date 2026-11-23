package com.c2yz.notae.ui.viewer

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.input.pointer.PointerType
import com.c2yz.notae.core.Overlay
import com.c2yz.notae.core.PageOverlay
import com.c2yz.notae.core.Stroke
import com.c2yz.notae.core.Tool
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The drawing session: tools, the eraser, text, undo and redo, and where touches go. */
class DrawSessionTest {
    private fun session(initial: Overlay? = null) = DrawSession(initial, "aa")

    private fun DrawSession.line(page: Int, vararg points: Pair<Float, Float>) {
        begin(page, points[0].first, points[0].second)
        points.drop(1).forEach { (x, y) -> extend(x, y) }
        end()
    }

    private fun strokes(session: DrawSession, page: Int = 0) =
        session.overlay().pages.firstOrNull { it.page == page.toUInt() }?.strokes.orEmpty()

    @Test
    fun pen_stroke_is_recorded_in_fractions() {
        val session = session()
        assertFalse(session.changed)
        session.line(0, 0.1f to 0.2f, 0.3f to 0.4f)
        val stroke = strokes(session).single()
        assertEquals(Tool.PEN, stroke.tool)
        assertEquals(0xFF000000u, stroke.color)
        assertEquals(0.003f, stroke.width)
        assertEquals(listOf(0.1f, 0.2f, 0.3f, 0.4f), stroke.points)
        assertEquals("aa", session.overlay().contentKey)
        assertTrue(session.changed)

        session.tool = DrawTool.PEN_RED
        session.line(0, 0.5f to 0.5f, 0.6f to 0.6f)
        assertEquals(0xFFD32F2Fu, strokes(session)[1].color)
        session.tool = DrawTool.HIGHLIGHTER
        session.line(1, 0.5f to 0.5f, 0.6f to 0.6f)
        val highlight = strokes(session, 1).single()
        assertEquals(Tool.HIGHLIGHTER, highlight.tool)
        assertEquals(0x66FFEB3Bu, highlight.color)
        assertEquals(0.02f, highlight.width)
    }

    @Test
    fun eraser_removes_whole_strokes_it_touches() {
        val session = session()
        session.line(0, 0.1f to 0.1f, 0.9f to 0.1f)
        session.line(0, 0.1f to 0.5f, 0.9f to 0.5f)
        session.tool = DrawTool.ERASER
        session.line(0, 0.5f to 0.105f)
        assertEquals("only the touched stroke goes, whole", 1, strokes(session).size)
        assertEquals(0.5f, strokes(session).single().points[1])
        session.line(0, 0.5f to 0.3f)
        assertEquals("a miss erases nothing", 1, strokes(session).size)
    }

    @Test
    fun undo_and_redo_walk_the_session() {
        val session = session()
        assertFalse(session.canUndo)
        session.line(0, 0.1f to 0.1f, 0.2f to 0.2f)
        session.line(0, 0.3f to 0.3f, 0.4f to 0.4f)
        session.undo()
        assertEquals(1, strokes(session).size)
        assertTrue(session.canRedo)
        session.redo()
        assertEquals(2, strokes(session).size)
        session.undo()
        session.line(0, 0.5f to 0.5f, 0.6f to 0.6f)
        assertFalse("a new stroke drops the redo", session.canRedo)
        session.undo()
        session.undo()
        assertTrue(strokes(session).isEmpty())
        assertFalse(session.canUndo)
        assertFalse("back where it started", session.changed)
    }

    @Test
    fun clear_page_is_undoable() {
        val initial = Overlay(
            "aa",
            listOf(PageOverlay(0u, listOf(Stroke(Tool.PEN, 0xFF000000u, 0.003f, listOf(0.1f, 0.1f, 0.2f, 0.2f))), emptyList())),
        )
        val session = session(initial)
        session.line(1, 0.1f to 0.1f, 0.2f to 0.2f)
        session.clearPage(0)
        assertTrue(strokes(session, 0).isEmpty())
        assertEquals("other pages stay", 1, strokes(session, 1).size)
        session.undo()
        assertEquals(1, strokes(session, 0).size)
    }

    @Test
    fun text_box_can_be_moved_and_deleted() {
        val session = session()
        session.addText(0, 0.2f, 0.3f, "rit.")
        val text = session.overlay().pages.single().texts.single()
        assertEquals("rit.", text.text)
        assertEquals(0.025f, text.size)
        session.moveText(0, 0, 0.4f, 0.5f)
        assertEquals(0.4f, session.overlay().pages.single().texts.single().x)
        session.deleteText(0, 0)
        assertTrue(session.overlay().pages.all { it.texts.isEmpty() })
        session.undo()
        session.undo()
        assertEquals(0.2f, session.overlay().pages.single().texts.single().x)
    }

    @Test
    fun finger_never_draws_once_a_stylus_was_seen() {
        assertEquals(TouchRoute.MOVE_PAGE, routeTouch(PointerType.Touch, stylusSeen = true, pointers = 1, drawing = true))
        assertEquals(TouchRoute.DRAW, routeTouch(PointerType.Stylus, stylusSeen = true, pointers = 1, drawing = true))
        assertEquals(TouchRoute.ERASE, routeTouch(PointerType.Eraser, stylusSeen = true, pointers = 1, drawing = true))
        assertEquals("outside drawing mode nothing draws", TouchRoute.MOVE_PAGE, routeTouch(PointerType.Stylus, stylusSeen = true, pointers = 1, drawing = false))
    }

    @Test
    fun finger_draws_before_a_stylus_was_seen() {
        assertEquals(TouchRoute.DRAW, routeTouch(PointerType.Touch, stylusSeen = false, pointers = 1, drawing = true))
        assertEquals("two fingers move the page", TouchRoute.MOVE_PAGE, routeTouch(PointerType.Touch, stylusSeen = false, pointers = 2, drawing = true))
    }

    @Test
    fun overlay_points_map_to_the_page_box() {
        val portrait = Rect(100f, 50f, 500f, 616f)
        val spread = Rect(640f, 0f, 1180f, 764f)
        assertEquals(Offset(300f, 191.5f), pagePoint(0.5f, 0.25f, portrait))
        assertEquals(Offset(910f, 191f), pagePoint(0.5f, 0.25f, spread))
        assertEquals(Offset(0.5f, 0.25f), pageFraction(Offset(300f, 191.5f), portrait))
        assertEquals(Offset(0.5f, 0.25f), pageFraction(Offset(910f, 191f), spread))
    }
}
