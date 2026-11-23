package com.c2yz.notae.ui.viewer

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipe
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Overlay
import com.c2yz.notae.core.PageOverlay
import com.c2yz.notae.core.Stroke
import com.c2yz.notae.core.Tool
import com.c2yz.notae.core.Viewer
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Drawing on a set's piece in the viewer: the tools, where touches go, and saving. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w800dp-h1280dp-xhdpi")
class DrawingTest {
    @get:Rule
    val compose = createComposeRule()

    private val actions = mutableListOf<Action>()
    private var shown by mutableStateOf<Viewer?>(null)

    private fun show(viewer: Viewer, stylusSeen: Boolean = false) {
        shown = viewer
        compose.setContent {
            NotaeTheme { shown?.let { ViewerScreen(it, { action -> actions += action }, { _, _ -> FakePages(3) }, stylusSeen) } }
        }
        compose.waitForIdle()
    }

    private fun inSet(overlay: Overlay? = null) =
        Fixture.viewer(Fixture.rows[0].copy(itemId = "item-1"), setId = "palm").copy(overlay = overlay)

    private val drawn = Overlay(
        Fixture.rows[0].noteKey,
        listOf(PageOverlay(0u, listOf(Stroke(Tool.PEN, 0xFF000000u, 0.003f, listOf(0.1f, 0.1f, 0.9f, 0.9f))), emptyList())),
    )

    /** A line across the middle of page 1. */
    private fun drawLine() {
        compose.onNodeWithTag("page-0").performTouchInput { swipe(Offset(width * 0.3f, height * 0.5f), Offset(width * 0.6f, height * 0.55f)) }
        compose.waitForIdle()
    }

    @Test
    fun draw_toggle_only_in_a_set() {
        show(Fixture.viewer())
        compose.onNodeWithContentDescription("Draw").assertDoesNotExist()
        shown = inSet()
        compose.waitForIdle()
        compose.onNodeWithContentDescription("Draw").assertIsDisplayed()
    }

    @Test
    fun finger_stroke_is_saved_on_done_before_a_stylus_was_seen() {
        show(inSet())
        compose.onNodeWithContentDescription("Draw").performClick()
        drawLine()
        compose.onNodeWithText("Done").performClick()

        val save = actions.filterIsInstance<Action.SaveOverlay>().single()
        assertEquals("palm", save.setId)
        assertEquals("item-1", save.itemId)
        val stroke = save.overlay.pages.single().strokes.single()
        assertEquals(Tool.PEN, stroke.tool)
        assertTrue("x starts near 0.3 of the page: ${stroke.points}", stroke.points[0] in 0.25f..0.35f)
    }

    @Test
    fun finger_moves_the_page_after_a_stylus() {
        show(inSet(), stylusSeen = true)
        compose.onNodeWithContentDescription("Draw").performClick()
        drawLine()
        compose.onNodeWithText("Done").performClick()

        assertTrue("a finger never draws: $actions", actions.none { it is Action.SaveOverlay })
    }

    @Test
    fun undo_then_done_saves_nothing() {
        show(inSet())
        compose.onNodeWithContentDescription("Draw").performClick()
        drawLine()
        compose.onNodeWithText("Undo").performClick()
        compose.onNodeWithText("Done").performClick()

        assertTrue(actions.none { it is Action.SaveOverlay })
    }

    @Test
    fun clear_page_asks_first() {
        show(inSet(drawn))
        compose.onNodeWithContentDescription("Draw").performClick()
        compose.onNodeWithText("Clear page").performClick()
        compose.onNodeWithText("Clear this page's drawings?").assertIsDisplayed()
        compose.onNodeWithText("Clear").performClick()
        compose.onNodeWithText("Done").performClick()

        val save = actions.filterIsInstance<Action.SaveOverlay>().single()
        assertTrue("the page is cleared", save.overlay.pages.isEmpty())
    }

    @Test
    fun hidden_drawings_are_not_drawn() {
        show(inSet(drawn))
        compose.onNodeWithTag("drawings-0").assertExists()
        compose.onNodeWithContentDescription("Hide drawings").performClick()
        compose.onNodeWithTag("drawings-0").assertDoesNotExist()
        compose.onNodeWithContentDescription("Show drawings").performClick()
        compose.onNodeWithTag("drawings-0").assertExists()
    }

    @Test
    fun leaving_the_viewer_saves_a_changed_drawing() {
        show(inSet())
        compose.onNodeWithContentDescription("Draw").performClick()
        drawLine()
        shown = null
        compose.waitForIdle()

        assertEquals(1, actions.filterIsInstance<Action.SaveOverlay>().size)
    }
}
