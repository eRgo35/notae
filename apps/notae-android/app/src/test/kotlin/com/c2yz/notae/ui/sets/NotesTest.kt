package com.c2yz.notae.ui.sets

import android.graphics.Bitmap
import android.graphics.Canvas
import android.view.View
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.withKeyDown
import androidx.compose.ui.unit.Density
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.AddPicker
import com.c2yz.notae.core.LibraryView
import com.c2yz.notae.core.NotaeCore
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.SetEntry
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.core.StatusMessage
import com.c2yz.notae.core.testCore
import com.c2yz.notae.platform.formatIsoDate
import com.c2yz.notae.ui.assertAllClickablesLabelled
import com.c2yz.notae.ui.assertClickTargetsAtLeast48dp
import com.c2yz.notae.ui.details.DetailsScreen
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.library.LibraryScreen
import com.c2yz.notae.ui.library.noteName
import com.c2yz.notae.ui.theme.NotaeTheme
import com.c2yz.notae.ui.theme.Tokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.nio.file.Files
import java.util.Locale

/** Text notes in a set: the add picker's Text note tab, the tinted cards and rows, the note's ⋮ and its editor. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class NotesTest {
    @get:Rule
    val compose = createComposeRule()

    private val actions = mutableListOf<Action>()
    private var shown by mutableStateOf<Snapshot?>(null)
    private var view: View? = null

    /** The library screen with the set overlays over it, as the shell draws them. */
    private fun show(snapshot: Snapshot, onAction: (Action) -> Unit = { actions += it }) {
        shown = snapshot
        var density = Density(1f)
        compose.setContent {
            density = LocalDensity.current
            view = LocalView.current
            NotaeTheme {
                shown?.let {
                    Box {
                        LibraryScreen(it, onAction)
                        SetOverlays(it, onAction)
                    }
                }
            }
        }
        compose.assertAllClickablesLabelled()
        compose.assertClickTargetsAtLeast48dp(density)
    }

    /** A core over Ave Maria and Barka, with the set "Mass" open and holding Ave Maria. */
    private fun withSet(titles: List<String> = listOf("Ave Maria", "Barka"), test: (NotaeCore, Snapshot) -> Unit) {
        val dir = Files.createTempDirectory("notae-notes").toFile()
        val core = testCore(dir.path, Scores(titles), NoListener)
        try {
            core.libraryRootPicked("Scores", "tree")
            core.await { it.screen == Screen.Library }
            core.dispatch(Action.Rescan)
            val library = core.await { it.banner.message is StatusMessage.Finished && it.rows.size == titles.size }
            core.dispatch(Action.NewSet)
            core.dispatch(Action.SaveSetDialog("Mass", null, null))
            core.dispatch(Action.OpenAddPicker)
            core.dispatch(Action.ToggleAddPickerPiece(library.rows.first { it.title == "Ave Maria" }.noteKey))
            core.dispatch(Action.CloseAddPicker)
            test(core, library)
        } finally {
            core.close()
            dir.deleteRecursively()
        }
    }

    /**
     * Most of the node's pixels are [color]: the card is tinted, not just its text. Draws the window's view itself, as
     * `captureToImage` times out under Robolectric here.
     */
    private fun SemanticsNodeInteraction.assertTinted(color: Color) {
        val bounds = fetchSemanticsNode().boundsInRoot
        val bitmap = compose.runOnIdle {
            val root = checkNotNull(view).rootView
            Bitmap.createBitmap(root.width, root.height, Bitmap.Config.ARGB_8888).also { root.draw(Canvas(it)) }
        }
        val argb = color.toArgb()
        var tinted = 0
        var all = 0
        for (x in bounds.left.toInt() until bounds.right.toInt()) {
            for (y in bounds.top.toInt() until bounds.bottom.toInt()) {
                all++
                if (bitmap.getPixel(x, y) == argb) tinted++
            }
        }
        val share = tinted.toFloat() / all
        assertTrue("only ${(share * 100).toInt()}% of the note is tinted", share > 0.5f)
    }

    @Test
    fun add_note_appends_tinted_card() = withSet { core, _ ->
        core.dispatch(Action.OpenAddPicker)
        show(core.snapshot()) { action ->
            actions += action
            core.dispatch(action)
            shown = core.snapshot()
        }

        compose.onNodeWithText("Text note").performClick()
        compose.onNodeWithText("Added at the end of the set.").assertIsDisplayed()
        compose.assertAllClickablesLabelled()
        compose.onNodeWithContentDescription("Note").performTextInput("Psalm 23")
        compose.onNodeWithText("Add note").performClick()

        assertEquals(listOf<Action>(Action.AddNote("Psalm 23")), actions)
        assertEquals(null, core.snapshot().addPicker)
        compose.onNodeWithText("Add note").assertDoesNotExist()
        compose.onNodeWithText("1 piece · 1 note").assertIsDisplayed()
        // Numbered after Ave Maria, read as a note, and tinted.
        compose.onNodeWithContentDescription("2. Note: Psalm 23").assertIsDisplayed().assertTinted(Tokens.light.note)

        // The list draws it as a tinted row.
        core.dispatch(Action.SetLibraryView(LibraryView.LIST))
        shown = core.snapshot()
        compose.waitForIdle()
        compose.onNodeWithContentDescription("2. Note: Psalm 23").assertIsDisplayed().assertTinted(Tokens.light.note)
    }

    @Test
    fun empty_note_cannot_be_added() {
        show(Fixture.palmSunday().copy(addPicker = AddPicker("", emptyList(), 0u)))

        compose.onNodeWithText("Text note").performClick()
        compose.onNodeWithText("Add note").assertIsNotEnabled()
        compose.onNodeWithContentDescription("Note").performTextInput("  \n ")
        compose.onNodeWithText("Add note").assertIsNotEnabled().performClick()
        compose.onNodeWithContentDescription("Note").performTextReplacement("Homilia")
        compose.onNodeWithText("Add note").assertIsEnabled()
        compose.onNodeWithText("Cancel").performClick()

        assertEquals(listOf<Action>(Action.CloseAddPicker), actions)
    }

    @Test
    fun details_previous_next_skip_notes() = withSet { core, library ->
        core.dispatch(Action.AddNote("Psalm"))
        core.dispatch(Action.OpenAddPicker)
        core.dispatch(Action.ToggleAddPickerPiece(library.rows.first { it.title == "Barka" }.noteKey))
        core.dispatch(Action.CloseAddPicker)
        val ave = core.snapshot().rows.first().selectionId
        core.dispatch(Action.SelectScore(ave))
        shown = core.snapshot()
        compose.setContent {
            NotaeTheme {
                shown?.let {
                    DetailsScreen(it, { action ->
                        actions += action
                        core.dispatch(action)
                        shown = core.snapshot()
                    })
                }
            }
        }

        compose.onNodeWithText("1 of 2").assertIsDisplayed()
        compose.onNodeWithContentDescription("Next score").performClick()
        // Over the note, straight to Barka, item 3.
        compose.onNodeWithText("2 of 2").assertIsDisplayed()
        assertEquals("Barka", core.snapshot().selection?.row?.title)
        compose.onNodeWithContentDescription("Next score").assertIsNotEnabled()
        compose.onNodeWithContentDescription("Previous score").performClick()
        compose.onNodeWithText("1 of 2").assertIsDisplayed()
        assertEquals(ave, core.snapshot().selection?.row?.selectionId)
    }

    @Test
    fun notes_are_numbered_in_place_and_tapping_one_does_nothing() {
        show(Fixture.palmWithNotes())

        compose.onNodeWithText("${formatIsoDate("2026-03-29", Locale.getDefault())} · 9 pieces · 2 notes").assertIsDisplayed()
        val psalm = compose.onNodeWithContentDescription("3. Note: ${Fixture.PSALM}")
        psalm.assertIsDisplayed()
        // A note has nothing to open: it is not a click target, and its ⋮ is its only action.
        assertEquals(0, compose.onAllNodes(hasContentDescription("Note:", substring = true) and hasClickAction()).fetchSemanticsNodes().size)
        compose.onNodeWithContentDescription("4. Magnificat").assertIsDisplayed()
        assertEquals(emptyList<Action>(), actions)
    }

    @Test
    fun note_controls_are_named_by_the_first_line_capped_at_60() {
        assertEquals("Homilia", noteName("Homilia"))
        assertEquals("Psalm 23…", noteName("Psalm 23\nverses 1-6"))
        assertEquals(Fixture.PSALM_NAME, noteName(Fixture.PSALM))
        assertEquals("a".repeat(60), noteName("a".repeat(60)))
    }

    @Test
    fun note_menu_edits_and_removes() {
        // The list, where every note's ⋮ is on screen.
        val noted = Fixture.palmWithNotes().let { it.copy(layout = it.layout.copy(libraryView = LibraryView.LIST)) }
        show(noted)

        compose.onNodeWithContentDescription("More actions for ${Fixture.PSALM_NAME}").performClick()
        compose.assertAllClickablesLabelled()
        compose.onNodeWithText("Edit note").performClick()
        // The editor holds the note's text and keeps what is typed when a snapshot arrives mid-edit.
        compose.onNodeWithContentDescription("Note").assertTextEquals(Fixture.PSALM)
        compose.assertAllClickablesLabelled()
        compose.onNodeWithContentDescription("Note").performTextReplacement("Psalm 24")
        shown = noted.copy(revision = 99u)
        compose.waitForIdle()
        compose.onNodeWithContentDescription("Note").performTextInput(", verses 1-6")
        compose.onNodeWithContentDescription("Note").assertTextEquals("Psalm 24, verses 1-6")
        compose.onNodeWithContentDescription("Note").performTextReplacement(" ")
        compose.onNodeWithText("Save note").assertIsNotEnabled()
        compose.onNodeWithContentDescription("Note").performTextReplacement("Psalm 24")
        compose.onNodeWithText("Save note").performClick()
        compose.onNodeWithText("Save note").assertDoesNotExist()

        // The keys are back on the screen once the editor closes.
        compose.onRoot().performKeyInput { withKeyDown(Key.CtrlLeft) { pressKey(Key.K) } }

        compose.onNodeWithContentDescription("More actions for ${Fixture.HOMILY}").performClick()
        compose.onNodeWithText("Remove from set").performClick()
        compose.onNodeWithText("Remove from set").assertDoesNotExist()

        // Escape closes the editor without saving.
        compose.onNodeWithContentDescription("More actions for ${Fixture.HOMILY}").performClick()
        compose.onNodeWithText("Edit note").performClick()
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        compose.onNodeWithText("Save note").assertDoesNotExist()

        assertEquals(
            listOf(Action.EditNote("note-1", "Psalm 24"), Action.OpenSearch, Action.RemoveSetItem("note-2")),
            actions,
        )
    }
}
