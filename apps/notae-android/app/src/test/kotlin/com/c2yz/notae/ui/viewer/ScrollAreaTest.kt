package com.c2yz.notae.ui.viewer

import androidx.compose.runtime.mutableStateOf
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.click
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.pinch
import androidx.compose.ui.test.swipeUp
import com.c2yz.notae.core.ViewerScale
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Pages one under another on a landscape tablet. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class ScrollAreaTest {
    @get:Rule
    val compose = createComposeRule()

    private var page by mutableIntStateOf(0)
    private val zoom = PageZoom()

    private fun show(twoPages: Boolean = false, scale: ViewerScale = ViewerScale.FitPage, pages: Int = 5) {
        val source = FakePages(pages)
        compose.setContent {
            NotaeTheme {
                PageArea(source, ViewLook(scroll = true, twoPages = twoPages, scale = scale), page, { page = it }, {}, Modifier.fillMaxSize(), zoom) { index ->
                    Box(Modifier.fillMaxSize().testTag("p-$index"))
                }
            }
        }
        compose.waitForIdle()
    }

    private fun tapRight() {
        compose.onRoot().performTouchInput { click(Offset(width * 0.95f, centerY)) }
        compose.mainClock.advanceTimeBy(1000)
        compose.waitForIdle()
    }

    @Test
    fun rows_stack_under_each_other() {
        show(scale = ViewerScale.Percent(25u))
        val first = compose.onNodeWithTag("p-0").getBoundsInRoot()
        val second = compose.onNodeWithTag("p-1").getBoundsInRoot()
        assertTrue("$first above $second", second.top > first.bottom)
        assertEquals(first.left, second.left)
    }

    @Test
    fun edge_tap_scrolls_to_the_next_row() {
        show()
        tapRight()
        assertEquals(1, page)
        assertEquals(compose.onRoot().getBoundsInRoot().top, compose.onNodeWithTag("p-1").getBoundsInRoot().top)
    }

    @Test
    fun edge_tap_reaches_the_last_row() {
        show(twoPages = true)
        tapRight()
        assertEquals(2, page)
        tapRight()
        tapRight()
        assertEquals(4, page)
        tapRight()
        assertEquals(4, page)
    }

    @Test
    fun pinch_zooms_the_rows() {
        show()
        val before = compose.onNodeWithTag("p-0").getBoundsInRoot().let { it.right - it.left }
        compose.onRoot().performTouchInput {
            pinch(center - Offset(10f, 0f), center - Offset(80f, 0f), center + Offset(10f, 0f), center + Offset(80f, 0f))
        }
        compose.waitForIdle()
        assertTrue("zoomed: ${zoom.factor}", zoom.factor > 1f)
        assertTrue(compose.onNodeWithTag("p-0").getBoundsInRoot().let { it.right - it.left } > before)
    }

    @Test
    fun fling_reports_the_first_visible_page() {
        show()
        compose.onRoot().performTouchInput { swipeUp() }
        compose.waitForIdle()
        assertTrue("scrolled past page 1: $page", page > 0)
    }

    @Test
    fun the_counter_reaches_the_last_page_when_rows_are_shorter_than_the_screen() {
        show(scale = ViewerScale.Percent(30u))
        repeat(5) { tapRight() }
        assertEquals(4, page)
    }

    @Test
    fun pinch_keeps_the_point_under_the_fingers_across() {
        show()
        // Already wider than the screen, so the pinch can pan across.
        compose.runOnIdle { zoom.factor = 2.5f }
        compose.waitForIdle()
        val node = { compose.onNodeWithTag("p-0").fetchSemanticsNode() }
        val screen = compose.onRoot().fetchSemanticsNode().size.width
        val at = screen * 0.6f
        val before = node()
        val fraction = (at - before.positionInRoot.x) / before.size.width
        compose.onRoot().performTouchInput {
            val c = Offset(at, centerY)
            pinch(c - Offset(20f, 0f), c - Offset(26f, 0f), c + Offset(20f, 0f), c + Offset(26f, 0f))
        }
        compose.waitForIdle()
        val after = node()
        assertTrue("zoomed further: ${zoom.factor}", zoom.factor > 3f)
        assertEquals(fraction, (at - after.positionInRoot.x) / after.size.width, 0.01f)
    }

    @Test
    fun pinching_does_not_recompose_the_rows_every_frame() {
        val pages = FakePages(5)
        compose.setContent {
            NotaeTheme {
                PageArea(pages, ViewLook(scroll = true, twoPages = false, scale = ViewerScale.FitPage), page, { page = it }, {}, Modifier.fillMaxSize(), zoom) { index ->
                    Box(Modifier.fillMaxSize().testTag("p-$index"))
                }
            }
        }
        compose.waitForIdle()
        val before = pages.aspectReads
        // A pinch changes the zoom a little every frame.
        for (step in 1..10) {
            compose.runOnIdle { zoom.factor = 1f + step * 0.1f }
            compose.waitForIdle()
        }
        // Once only: past 1x the pages render sharper.
        assertEquals("rows recomposed", 1, pages.aspectReads - before)
    }

}
