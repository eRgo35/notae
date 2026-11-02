package com.c2yz.notae.ui.components

import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.Snapshot
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.click
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipe
import androidx.compose.ui.test.swipeDown
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Dialogs and sheets animate in and out ([AnimatedOverlay]), and a sheet closes when dragged down. */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w800dp-h600dp-xhdpi")
class OverlayTest {
    @get:Rule
    val compose = createComposeRule()

    private var open by mutableStateOf<String?>("Hello")
    private var taps = 0
    private var dismissed = 0

    private fun showDialog() {
        compose.setContent {
            NotaeTheme {
                AnimatedOverlay(open) { text ->
                    NotaeDialog("Dialog") {
                        PillButton(text, { taps++ }, Modifier.fillMaxWidth().height(400.dp))
                    }
                }
            }
        }
        compose.onNodeWithText("Hello").assertIsDisplayed()
    }

    /** Closes the overlay and stops the clock a few frames into its exit. */
    private fun closeMidway() {
        compose.mainClock.autoAdvance = false
        open = null
        Snapshot.sendApplyNotifications()
        compose.mainClock.advanceTimeBy(48)
    }

    @Test
    fun a_closing_dialog_leaves_the_semantics_at_once() {
        showDialog()
        closeMidway()
        compose.onNodeWithText("Hello").assertDoesNotExist()
    }

    @Test
    fun a_tap_on_a_closing_dialog_does_nothing() {
        showDialog()
        closeMidway()
        compose.onRoot().performTouchInput { click(center) }
        compose.mainClock.advanceTimeBy(1_000)
        assertEquals(0, taps)
    }

    @Test
    fun a_closing_dialog_takes_no_keys() {
        var escapes = 0
        val focus = FocusRequester()
        compose.setContent {
            NotaeTheme {
                AnimatedOverlay(open) {
                    NotaeDialog("Dialog", Modifier.onPreviewKeyEvent { event -> (event.key == Key.Escape && event.type == KeyEventType.KeyDown).also { if (it) escapes++ } }) {
                        Box(Modifier.size(48.dp).focusRequester(focus).focusable())
                    }
                }
            }
        }
        compose.runOnIdle { focus.requestFocus() }
        closeMidway()
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        assertEquals(0, escapes)
    }

    @Test
    fun a_card_closed_by_the_back_gesture_stays_as_small_as_the_gesture_left_it() {
        val hold = BackHold()
        assertEquals(0.6f, hold.read(0.6f, closing = false))
        // The gesture commits: the system reports no progress any more while the card animates away.
        assertEquals(0.6f, hold.read(0f, closing = true))
        // Reopened, it follows the next gesture again.
        assertEquals(0f, hold.read(0f, closing = false))
    }

    @Test
    fun a_gap_after_a_collapsing_item_shrinks_with_it() {
        val positions = IntArray(4)
        with(collapsingGaps(12.dp)) { with(Density(1f)) { arrange(400, intArrayOf(100, 5, 0, 50), positions) } }
        // A full gap after the header, then gaps no taller than the banner shrinking (5) and the one gone (0).
        assertArrayEquals(intArrayOf(0, 112, 122, 122), positions)
    }

    @Test
    fun a_sheet_reopened_while_it_closes_comes_back_in_place() {
        compose.setContent {
            NotaeTheme {
                AnimatedOverlay(open) { text ->
                    ActionSheet("Sheet", { open = null }) { Text(text, Modifier.fillMaxWidth().height(240.dp)) }
                }
            }
        }
        val top = compose.onNodeWithText("Hello").getBoundsInRoot().top
        compose.onNodeWithText("Hello").performTouchInput { swipeDown() }
        compose.mainClock.autoAdvance = false
        Snapshot.sendApplyNotifications()
        compose.mainClock.advanceTimeBy(48)
        open = "Hello"
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()
        assertEquals(top, compose.onNodeWithText("Hello").getBoundsInRoot().top)
    }

    private fun showSheet() {
        compose.setContent {
            NotaeTheme {
                ActionSheet("Sheet", { dismissed++ }) {
                    Text("Body", Modifier.fillMaxWidth().height(240.dp))
                }
            }
        }
    }

    @Test
    fun a_sheet_dragged_down_closes() {
        showSheet()
        compose.onNodeWithText("Body").performTouchInput { swipeDown() }
        compose.waitForIdle()
        assertEquals(1, dismissed)
    }

    @Test
    fun a_short_drag_springs_back() {
        showSheet()
        val top = compose.onNodeWithText("Body").getBoundsInRoot().top
        compose.onNodeWithText("Body").performTouchInput { swipe(center, center + Offset(0f, 30f), 300) }
        compose.waitForIdle()
        assertEquals(0, dismissed)
        assertEquals(top, compose.onNodeWithText("Body").getBoundsInRoot().top)
    }
}
