package com.c2yz.notae.ui.viewer

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.geometry.Offset
import android.view.InputDevice
import android.view.MotionEvent
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.pinch
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
            NotaeTheme { shown?.let { ViewerScreen(it, { action -> actions += action }, { _, _ -> FakePages(3) }, Fixture.layout.copy(stylusSeen = stylusSeen)) } }
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
    fun pinch_over_drawings_zooms_in_drawing_mode() {
        show(inSet(drawn))
        compose.onNodeWithContentDescription("Draw").performClick()
        val before = compose.onNodeWithTag("page-0").fetchSemanticsNode().size.width
        compose.onNodeWithTag("page-0").performTouchInput {
            pinch(center - Offset(10f, 0f), center - Offset(150f, 0f), center + Offset(10f, 0f), center + Offset(150f, 0f))
        }
        compose.waitForIdle()
        val after = compose.onNodeWithTag("page-0").fetchSemanticsNode().size.width
        assertTrue("the page grew: $before → $after", after > before)
        compose.onNodeWithText("Done").performClick()
        assertTrue("a pinch draws nothing: $actions", actions.none { it is Action.SaveOverlay })
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

    @Test
    fun same_score_twice_saves_onto_the_item_drawn_on() {
        show(inSet())
        // Next to a second copy of the same score in the set.
        shown = Fixture.viewer(Fixture.rows[0].copy(itemId = "item-2"), setId = "palm")
        compose.waitForIdle()
        compose.onNodeWithContentDescription("Draw").performClick()
        drawLine()
        shown = null
        compose.waitForIdle()

        assertEquals(listOf("item-2"), actions.filterIsInstance<Action.SaveOverlay>().map { it.itemId })
    }

    @Test
    fun stopping_the_app_saves_the_drawing() {
        val owner = object : LifecycleOwner {
            val registry = LifecycleRegistry(this).apply { currentState = Lifecycle.State.RESUMED }
            override val lifecycle: Lifecycle get() = registry
        }
        shown = inSet()
        compose.setContent {
            CompositionLocalProvider(LocalLifecycleOwner provides owner) {
                NotaeTheme { shown?.let { ViewerScreen(it, { action -> actions += action }, { _, _ -> FakePages(3) }) } }
            }
        }
        compose.waitForIdle()
        compose.onNodeWithContentDescription("Draw").performClick()
        drawLine()
        compose.runOnUiThread { owner.registry.currentState = Lifecycle.State.CREATED }
        compose.waitForIdle()

        assertEquals("saved before Android may end the app", 1, actions.filterIsInstance<Action.SaveOverlay>().size)
    }

    /** Sends [events] (built over page 1) to the window as Android's input would. */
    private fun inject(vararg events: (Offset) -> MotionEvent) {
        val bounds = compose.onNodeWithTag("page-0").fetchSemanticsNode().boundsInWindow
        // One event per frame, as Android delivers them: the gesture code resumes between events.
        events.forEach { event ->
            compose.runOnUiThread { view.dispatchTouchEvent(event(bounds.topLeft)) }
            compose.waitForIdle()
        }
    }

    private lateinit var view: android.view.View

    private fun showCapturingView(viewer: Viewer, stylusSeen: Boolean) {
        shown = viewer
        compose.setContent {
            view = LocalView.current
            NotaeTheme { shown?.let { ViewerScreen(it, { action -> actions += action }, { _, _ -> FakePages(3) }, Fixture.layout.copy(stylusSeen = stylusSeen)) } }
        }
        compose.waitForIdle()
    }

    @Test
    fun stylus_stroke_is_saved_on_done() {
        showCapturingView(inSet(), stylusSeen = false)
        compose.onNodeWithContentDescription("Draw").performClick()
        val pen = Pointer(0, MotionEvent.TOOL_TYPE_STYLUS)
        inject(
            { o -> motion(MotionEvent.ACTION_DOWN, o, pen to Offset(100f, 300f)) },
            { o -> motion(MotionEvent.ACTION_MOVE, o, pen to Offset(200f, 320f)) },
            { o -> motion(MotionEvent.ACTION_MOVE, o, pen to Offset(300f, 340f)) },
            { o -> motion(MotionEvent.ACTION_UP, o, pen to Offset(300f, 340f)) },
        )
        compose.onNodeWithText("Done").performClick()

        assertTrue("the first pen touch is reported", Action.StylusSeen in actions)
        assertEquals(1, actions.filterIsInstance<Action.SaveOverlay>().single().overlay.pages.single().strokes.size)
    }

    @Test
    fun pen_draws_while_a_palm_rests() {
        showCapturingView(inSet(), stylusSeen = true)
        compose.onNodeWithContentDescription("Draw").performClick()
        val palm = Pointer(0, MotionEvent.TOOL_TYPE_FINGER)
        val pen = Pointer(1, MotionEvent.TOOL_TYPE_STYLUS)
        val rest = Offset(500f, 800f)
        inject(
            { o -> motion(MotionEvent.ACTION_DOWN, o, palm to rest) },
            { o -> motion(MotionEvent.ACTION_POINTER_DOWN or (1 shl MotionEvent.ACTION_POINTER_INDEX_SHIFT), o, palm to rest, pen to Offset(100f, 300f)) },
            { o -> motion(MotionEvent.ACTION_MOVE, o, palm to rest, pen to Offset(200f, 320f)) },
            { o -> motion(MotionEvent.ACTION_MOVE, o, palm to rest, pen to Offset(300f, 340f)) },
            { o -> motion(MotionEvent.ACTION_POINTER_UP or (1 shl MotionEvent.ACTION_POINTER_INDEX_SHIFT), o, palm to rest, pen to Offset(300f, 340f)) },
            { o -> motion(MotionEvent.ACTION_UP, o, palm to rest) },
        )
        compose.onNodeWithText("Done").performClick()

        val stroke = actions.filterIsInstance<Action.SaveOverlay>().single().overlay.pages.single().strokes.single()
        assertTrue("the pen line, not the palm: ${stroke.points}", stroke.points.chunked(2).all { (_, y) -> y < 0.5f })
    }
}

/** A pointer for [motion]: its id and Android tool type. */
private data class Pointer(val id: Int, val toolType: Int)

private var eventTime = 1_000L

/** A touchscreen MotionEvent; positions are relative to [origin] (the page's top-left in the window). */
private fun motion(action: Int, origin: Offset, vararg pointers: Pair<Pointer, Offset>): MotionEvent {
    eventTime += 16
    val properties = pointers.map { (p, _) -> MotionEvent.PointerProperties().apply { id = p.id; toolType = p.toolType } }.toTypedArray()
    val coords = pointers.map { (_, at) -> MotionEvent.PointerCoords().apply { x = origin.x + at.x; y = origin.y + at.y; pressure = 1f; size = 1f } }.toTypedArray()
    // One source for the whole stream, as a pen-enabled touchscreen reports it.
    val source = InputDevice.SOURCE_STYLUS or InputDevice.SOURCE_TOUCHSCREEN
    return MotionEvent.obtain(1_000L, eventTime, action, pointers.size, properties, coords, 0, 0, 1f, 1f, 0, 0, source, 0)
}
