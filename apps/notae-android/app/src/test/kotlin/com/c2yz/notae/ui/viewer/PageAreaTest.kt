package com.c2yz.notae.ui.viewer

import android.view.View
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.test.click
import androidx.compose.ui.test.doubleClick
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.pinch
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.swipeLeft
import androidx.compose.ui.test.swipeRight
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

    private var spread by mutableIntStateOf(0)
    private var toggles = 0
    private val zoom = PageZoom()

    private fun show(mode: PageMode = PageMode.TWO, pages: Int = 5) {
        val source = FakePages(pages)
        compose.setContent {
            NotaeTheme {
                PageArea(source, mode, spread, { spread = it }, { toggles++ }, Modifier.fillMaxSize(), zoom)
            }
        }
        compose.waitForIdle()
    }

    /** A tap, once the double-tap wait is over. */
    private fun tap(at: (androidx.compose.ui.test.TouchInjectionScope) -> Offset) {
        compose.onRoot().performTouchInput { click(at(this)) }
        compose.mainClock.advanceTimeBy(500)
        compose.waitForIdle()
    }

    @Test
    fun edge_tap_turns_by_a_spread() {
        show()
        tap { Offset(it.width * 0.95f, it.centerY) }
        assertEquals(1, spread)
        tap { Offset(it.width * 0.95f, it.centerY) }
        tap { Offset(it.width * 0.95f, it.centerY) }
        assertEquals("the last spread stays", 2, spread)
        tap { Offset(it.width * 0.05f, it.centerY) }
        assertEquals(1, spread)
        assertEquals(0, toggles)
    }

    @Test
    fun middle_tap_toggles_the_bars() {
        show()
        tap { it.center }
        assertEquals(1, toggles)
        assertEquals(0, spread)
    }

    @Test
    fun swipe_left_turns() {
        show()
        compose.onRoot().performTouchInput { swipeLeft() }
        assertEquals(1, spread)
        compose.onRoot().performTouchInput { swipeRight() }
        assertEquals(0, spread)
    }

    @Test
    fun double_tap_returns_to_fit() {
        show()
        compose.onRoot().performTouchInput {
            pinch(center - Offset(20f, 0f), center - Offset(200f, 0f), center + Offset(20f, 0f), center + Offset(200f, 0f))
        }
        compose.waitForIdle()
        assertTrue("pinched in: ${zoom.scale}", zoom.scale > 1f)
        assertTrue("at most 4×", zoom.scale <= 4f)
        compose.onRoot().performTouchInput { swipeLeft() }
        assertEquals("a zoomed page pans instead of turning", 0, spread)
        compose.onRoot().performTouchInput { doubleClick(center) }
        compose.waitForIdle()
        assertEquals(1f, zoom.scale)
        assertEquals(Offset.Zero, zoom.offset)
    }

    @Test
    fun page_down_and_arrows_turn() {
        show(PageMode.ONE)
        compose.onRoot().performKeyInput { pressKey(Key.PageDown) }
        assertEquals(1, spread)
        compose.onRoot().performKeyInput { pressKey(Key.DirectionRight) }
        assertEquals(2, spread)
        compose.onRoot().performKeyInput { pressKey(Key.DirectionLeft) }
        compose.onRoot().performKeyInput { pressKey(Key.PageUp) }
        assertEquals(0, spread)
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
