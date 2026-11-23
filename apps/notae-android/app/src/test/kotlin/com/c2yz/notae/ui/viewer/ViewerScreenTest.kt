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

    private fun show(viewer: Viewer, pages: PageSourceFactory = { _, _ -> FakePages(3) }) {
        shown = viewer
        compose.setContent {
            NotaeTheme { shown?.let { ViewerScreen(it, { action -> actions += action }, pages) } }
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
}
