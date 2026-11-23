package com.c2yz.notae.ui.viewer

import android.view.View
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.click
import androidx.compose.ui.test.doubleClick
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.pinch
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.swipeLeft
import androidx.compose.ui.test.swipeRight
import com.c2yz.notae.core.ViewerScale
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Turning, zooming and the bars on a landscape tablet, two pages at a time. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class PageAreaTest {
    @get:Rule
    val compose = createComposeRule()

    private var page by mutableIntStateOf(0)
    private var toggles = 0
    private val zoom = PageZoom()
    private val twoPages = ViewLook(scroll = false, twoPages = true, scale = ViewerScale.FitPage)
    private val onePage = ViewLook(scroll = false, twoPages = false, scale = ViewerScale.FitPage)

    private fun show(look: ViewLook = twoPages, source: FakePages = FakePages(5)) {
        compose.setContent {
            NotaeTheme {
                PageArea(source, look, page, { page = it }, { toggles++ }, Modifier.fillMaxSize(), zoom) { index, _ ->
                    Box(Modifier.fillMaxSize().testTag("p-$index"))
                }
            }
        }
        compose.waitForIdle()
    }

    /** A tap, once the double-tap wait is over. */
    private fun tap(at: (androidx.compose.ui.test.TouchInjectionScope) -> Offset) {
        compose.onRoot().performTouchInput { click(at(this)) }
        compose.mainClock.advanceTimeBy(1000)
        compose.waitForIdle()
    }

    private fun pinchOut(from: (androidx.compose.ui.test.TouchInjectionScope) -> Offset) {
        compose.onRoot().performTouchInput {
            val at = from(this)
            pinch(at - Offset(10f, 0f), at - Offset(80f, 0f), at + Offset(10f, 0f), at + Offset(80f, 0f))
        }
        compose.mainClock.advanceTimeBy(1000)
        compose.waitForIdle()
    }

    @Test
    fun edge_tap_turns_by_a_spread() {
        show()
        tap { Offset(it.width * 0.95f, it.centerY) }
        assertEquals(2, page)
        tap { Offset(it.width * 0.95f, it.centerY) }
        tap { Offset(it.width * 0.95f, it.centerY) }
        assertEquals("the last spread stays", 4, page)
        tap { Offset(it.width * 0.05f, it.centerY) }
        assertEquals(2, page)
        assertEquals(0, toggles)
    }

    @Test
    fun turn_is_animated() {
        show()
        compose.mainClock.autoAdvance = false
        compose.onRoot().performTouchInput { click(Offset(width * 0.95f, centerY)) }
        // Past the double-tap wait, not yet settled.
        compose.mainClock.advanceTimeBy(400)
        compose.onNodeWithTag("p-0").assertExists()
        compose.onNodeWithTag("p-2").assertExists()
        compose.mainClock.advanceTimeBy(2000)
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()
        assertEquals(2, page)
    }

    @Test
    fun middle_tap_toggles_the_bars() {
        show()
        tap { it.center }
        assertEquals(1, toggles)
        assertEquals(0, page)
    }

    @Test
    fun swipe_left_turns() {
        show()
        compose.onRoot().performTouchInput { swipeLeft() }
        compose.waitForIdle()
        assertEquals(2, page)
        compose.onRoot().performTouchInput { swipeRight() }
        compose.waitForIdle()
        assertEquals(0, page)
    }

    @Test
    fun double_tap_returns_to_fit() {
        show()
        pinchOut { it.center }
        assertTrue("pinched in: ${zoom.factor}", zoom.factor > 1f)
        assertTrue("at most 4×", zoom.factor <= 4f)
        compose.onRoot().performTouchInput { swipeLeft() }
        compose.waitForIdle()
        assertEquals("a zoomed page pans instead of turning", 0, page)
        compose.onRoot().performTouchInput { doubleClick(center) }
        compose.waitForIdle()
        assertEquals(1f, zoom.factor)
        assertEquals(Offset.Zero, zoom.offset)
    }

    @Test
    fun pinch_at_the_edge_zooms_without_turning() {
        show()
        pinchOut { Offset(it.width * 0.1f, it.centerY) }
        assertTrue("pinched at the edge: ${zoom.factor}", zoom.factor > 1f)
        assertEquals(0, page)
        pinchOut { Offset(it.width * 0.9f, it.height * 0.1f) }
        assertEquals(0, page)
    }

    @Test
    fun overflowing_spread_pans_on_swipe_and_turns_on_edge_tap() {
        // Landscape pages at fit height are wider than the area.
        show(ViewLook(scroll = false, twoPages = false, scale = ViewerScale.FitHeight), FakePages(3, aspect = 0.5f))
        compose.onRoot().performTouchInput { swipeLeft() }
        compose.waitForIdle()
        assertEquals("a swipe pans", 0, page)
        assertTrue("panned: ${zoom.offset}", zoom.offset.x > 0f)
        tap { Offset(it.width * 0.95f, it.centerY) }
        assertEquals(1, page)
        assertEquals("a new page starts at its left", 0f, zoom.offset.x)
    }

    @Test
    fun page_down_and_arrows_turn() {
        show(onePage)
        compose.onRoot().performKeyInput { pressKey(Key.PageDown) }
        compose.waitForIdle()
        assertEquals(1, page)
        compose.onRoot().performKeyInput { pressKey(Key.DirectionRight) }
        compose.waitForIdle()
        assertEquals(2, page)
        compose.onRoot().performKeyInput { pressKey(Key.DirectionLeft) }
        compose.onRoot().performKeyInput { pressKey(Key.PageUp) }
        compose.waitForIdle()
        assertEquals(0, page)
    }
    @Test
    fun a_pinch_after_the_area_narrows_keeps_the_point_under_the_fingers() {
        var wide by mutableStateOf(true)
        compose.setContent {
            NotaeTheme {
                Box(Modifier.fillMaxSize()) {
                    PageArea(FakePages(5), onePage, page, { page = it }, {}, Modifier.fillMaxHeight().width(if (wide) 1280.dp else 900.dp), zoom) { index, _ ->
                        Box(Modifier.fillMaxSize().testTag("p-$index"))
                    }
                }
            }
        }
        compose.waitForIdle()
        // Something before the narrowing, so a pinch handler exists from the wide area.
        pinchOut { it.center }
        zoom.reset()
        wide = false
        compose.waitForIdle()
        pinchOut { Offset(450f * it.density, it.centerY) }
        // One A4 page fits 900 dp (1800 px) by 1600 px: 1132 px wide from x 334. Pinched to 4x around x 900 px, the
        // page point under the fingers (566) stays there, so the page is panned 4 * 566 - 900 = 1364 px. Measured
        // against the old 2560 px area, the pan would come out 0.
        assertEquals(4f, zoom.factor)
        assertEquals(1364f, zoom.offset.x, 5f)
    }
}

/** The screen stays on while the viewer shows, and Escape closes it. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class ViewerWindowTest {
    @get:Rule
    val compose = createComposeRule()

    @Test
    fun screen_stays_on_while_the_viewer_shows() {
        var showing by mutableStateOf(true)
        var view: View? = null
        compose.setContent {
            view = LocalView.current
            NotaeTheme { if (showing) ViewerScreen(Fixture.viewer(), {}, { _, _ -> FakePages(3) }) }
        }
        compose.waitForIdle()
        assertTrue(view?.keepScreenOn == true)
        showing = false
        compose.waitForIdle()
        assertFalse(view?.keepScreenOn == true)
    }

}
