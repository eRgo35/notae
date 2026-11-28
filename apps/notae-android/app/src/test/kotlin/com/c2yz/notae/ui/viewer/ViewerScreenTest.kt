package com.c2yz.notae.ui.viewer

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Viewer
import com.c2yz.notae.core.ViewerLayout
import com.c2yz.notae.core.ViewerPages
import com.c2yz.notae.core.ViewerScale
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.pinch
import androidx.compose.ui.test.pressKey
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import java.io.FileNotFoundException
import com.c2yz.notae.platform.PageSourceFactory
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Notae's viewer: its bars, pages, set navigation, notes, hint and errors. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class ViewerScreenTest {
    @get:Rule
    val compose = createComposeRule()

    private val actions = mutableListOf<Action>()
    private var shown by mutableStateOf<Viewer?>(null)
    private var layout by mutableStateOf(Fixture.layout)

    private fun show(viewer: Viewer, pages: PageSourceFactory = { _, _ -> FakePages(3) }) {
        shown = viewer
        compose.setContent {
            NotaeTheme { shown?.let { ViewerScreen(it, { action -> actions += action }, pages, layout) } }
        }
        compose.waitForIdle()
    }

    @Test
    fun viewer_back_button_dispatches_close() {
        show(Fixture.viewer())

        compose.onNodeWithText(Fixture.rows[0].title).assertIsDisplayed()
        compose.onNodeWithContentDescription("Back").performClick()

        assertEquals(listOf<Action>(Action.CloseViewer), actions)
    }

    @Test
    fun pages_render_from_the_source() {
        val pages = FakePages(3)
        show(Fixture.viewer()) { _, _ -> pages }

        compose.onNodeWithText("1 / 3").assertIsDisplayed()
        assertTrue("the first page is drawn", 0 in pages.rendered)
    }

    @Test
    fun unreadable_pdf_shows_the_error_with_back() {
        show(Fixture.viewer()) { _, _ -> throw FileNotFoundException("Ave Maria.pdf is gone") }

        compose.onNodeWithText("Ave Maria.pdf is gone", substring = true).assertIsDisplayed()
        compose.onNodeWithContentDescription("Back").performClick()
        assertEquals(listOf<Action>(Action.CloseViewer), actions)
    }

    @Test
    fun the_source_closes_when_the_viewer_leaves() {
        val pages = FakePages(3)
        show(Fixture.viewer()) { _, _ -> pages }
        shown = null
        compose.waitForIdle()
        assertTrue(pages.closed)
    }

    private fun inSet() = Fixture.viewer(Fixture.rows[0].copy(itemId = "item-1", scoreNote = "Capo 2", setNote = "Slowly"), setId = "palm")

    @Test
    fun set_bar_moves_between_pieces() {
        show(inSet().copy(hasPrevious = false, hasNext = true))

        compose.onNodeWithContentDescription("Previous piece").assertIsNotEnabled()
        compose.onNodeWithContentDescription("Next piece").assertIsEnabled().performClick()
        shown = inSet().copy(hasPrevious = true, hasNext = false)
        compose.waitForIdle()
        compose.onNodeWithContentDescription("Next piece").assertIsNotEnabled()
        compose.onNodeWithContentDescription("Previous piece").performClick()

        assertEquals(listOf(Action.ViewerNext, Action.ViewerPrevious), actions)
    }

    @Test
    fun the_set_and_the_place_in_it_show_under_the_title() {
        show(inSet())

        compose.onNodeWithText("Niedziela Palmowa · 1 of 2").assertIsDisplayed()
    }

    @Test
    fun turning_past_the_last_page_opens_the_next_piece() {
        layout = Fixture.layout.copy(viewerPages = ViewerPages.ONE)
        show(inSet().copy(hasNext = true)) { _, _ -> FakePages(2) }
        repeat(2) {
            compose.onRoot().performKeyInput { pressKey(Key.PageDown) }
            compose.waitForIdle()
        }

        assertEquals(listOf<Action>(Action.ViewerNext), actions)
    }

    @Test
    fun turning_back_from_the_first_page_opens_the_previous_piece_at_its_last_page() {
        layout = Fixture.layout.copy(viewerPages = ViewerPages.ONE)
        show(inSet().copy(hasPrevious = true))
        compose.onRoot().performKeyInput { pressKey(Key.PageUp) }
        compose.waitForIdle()
        assertEquals(listOf<Action>(Action.ViewerPrevious), actions)

        shown = inSet().copy(row = Fixture.rows[1].copy(itemId = "item-0"), hasNext = true)
        compose.waitForIdle()
        compose.onNodeWithText("3 / 3").assertIsDisplayed()
    }

    @Test
    fun turning_past_the_end_of_the_set_says_so() {
        layout = Fixture.layout.copy(viewerPages = ViewerPages.ONE)
        show(inSet()) { _, _ -> FakePages(1) }
        compose.onRoot().performKeyInput { pressKey(Key.PageDown) }

        compose.onNodeWithText("End of the set").assertIsDisplayed()
        assertEquals(emptyList<Action>(), actions)
    }

    @Test
    fun opening_a_piece_names_it_and_its_place() {
        compose.mainClock.autoAdvance = false
        show(inSet())
        compose.mainClock.advanceTimeBy(500)

        compose.onNodeWithText("1 of 2 · ${Fixture.rows[0].title}").assertIsDisplayed()
    }

    @Test
    fun no_set_bar_outside_a_set() {
        show(Fixture.viewer())

        compose.onNodeWithContentDescription("Next piece").assertDoesNotExist()
        compose.onNodeWithContentDescription("Previous piece").assertDoesNotExist()
    }

    @Test
    fun notes_sheet_lists_score_note_then_set_note() {
        show(inSet())

        compose.onNodeWithContentDescription("Notes").performClick()
        val score = compose.onNodeWithText("Capo 2").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
        val set = compose.onNodeWithText("Slowly").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
        assertTrue("the score note comes first", score.top < set.top)
        compose.onNodeWithText("Note for Niedziela Palmowa").assertIsDisplayed()
        compose.onNodeWithText("Slowly").performClick()
        compose.onNodeWithText("Save note").performClick()

        assertEquals(listOf<Action>(Action.SaveSetNote("item-1", "Slowly")), actions)
    }

    @Test
    fun outside_a_set_the_notes_sheet_has_only_the_score_note() {
        show(Fixture.viewer())

        compose.onNodeWithContentDescription("Notes").performClick()
        compose.onNodeWithText("Add score note").performClick()
        compose.onNodeWithText("Save note").performClick()

        assertEquals(listOf<Action>(Action.SaveScoreNote(Fixture.rows[0].noteKey, "")), actions)
    }

    @Test
    fun hint_shows_and_is_dismissed() {
        show(Fixture.viewer().copy(showHint = true))

        compose.onNodeWithText("Scores now open in Notae. Change it in Settings.").assertIsDisplayed()
        compose.onNodeWithText("OK").performClick()

        assertEquals(listOf<Action>(Action.ViewerHintSeen), actions)
    }

    @Test
    fun changed_pdf_shows_the_warning() {
        show(inSet().copy(contentChanged = true))

        compose.onNodeWithText("This score's PDF changed; drawings may not line up.").assertIsDisplayed()
    }

    @Test
    fun view_menu_marks_the_current_look() {
        show(Fixture.viewer())
        compose.onNodeWithContentDescription("Page layout and size").performClick()
        compose.onNodeWithText("Pages").assertIsSelected()
        compose.onNodeWithText("Scroll").assertIsNotSelected()
        compose.onNodeWithText("2 pages").assertIsSelected()
        compose.onNodeWithText("Fit pages").assertIsSelected()
        compose.onNodeWithText("Fit width").assertIsNotSelected()
        compose.onNodeWithText("100 %").assertIsNotSelected()
    }

    @Test
    fun picking_scroll_dispatches_the_look() {
        layout = Fixture.layout.copy(viewerScale = ViewerScale.Percent(150u))
        show(Fixture.viewer())
        compose.onNodeWithContentDescription("Page layout and size").performClick()
        compose.onNodeWithText("150 %").assertIsSelected()
        compose.onNodeWithText("Scroll").performClick()
        assertEquals(
            listOf<Action>(Action.SetViewerLook(ViewerLayout.SCROLL, ViewerPages.AUTO, ViewerScale.Percent(150u))),
            actions.filterIsInstance<Action.SetViewerLook>(),
        )
        compose.onNodeWithText("Scroll").assertDoesNotExist()
    }

    @Test
    fun switching_layout_keeps_the_page() {
        layout = Fixture.layout.copy(viewerPages = ViewerPages.ONE)
        show(Fixture.viewer()) { _, _ -> FakePages(5) }
        repeat(3) {
            compose.onRoot().performKeyInput { pressKey(Key.PageDown) }
            compose.waitForIdle()
        }
        compose.onNodeWithText("4 / 5").assertIsDisplayed()
        layout = layout.copy(viewerLayout = ViewerLayout.SCROLL)
        compose.waitForIdle()
        compose.onNodeWithText("4 / 5").assertIsDisplayed()
        // Pages pair as printed (3–4), and the page you were on is kept: back to one page, it is page 4 again.
        layout = layout.copy(viewerPages = ViewerPages.TWO, viewerLayout = ViewerLayout.PAGES)
        compose.waitForIdle()
        compose.onNodeWithText("4 / 5").assertIsDisplayed()
        layout = layout.copy(viewerPages = ViewerPages.ONE)
        compose.waitForIdle()
        compose.onNodeWithText("4 / 5").assertIsDisplayed()
    }

    @Test
    fun pinched_view_shows_live_percent() {
        show(Fixture.viewer())
        compose.onNodeWithContentDescription("Page layout and size").performClick()
        compose.onNodeWithText("Scale", substring = true).assertTextEquals("Scale")
        compose.onNodeWithText("Pages").performClick()
        compose.onRoot().performTouchInput {
            pinch(center - Offset(10f, 0f), center - Offset(80f, 0f), center + Offset(10f, 0f), center + Offset(80f, 0f))
        }
        compose.waitForIdle()
        compose.onNodeWithContentDescription("Page layout and size").performClick()
        compose.onNodeWithText("Scale (", substring = true).assertExists()
        compose.onNodeWithText("Fit pages").assertIsNotSelected()
    }
}
