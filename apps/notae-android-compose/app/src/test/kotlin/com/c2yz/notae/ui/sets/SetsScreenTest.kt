package com.c2yz.notae.ui.sets

import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isSelectable
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.withKeyDown
import androidx.compose.ui.unit.Density
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.AddPicker
import com.c2yz.notae.core.AddToSetSheet
import com.c2yz.notae.core.CoreListener
import com.c2yz.notae.core.DeleteSetConfirmation
import com.c2yz.notae.core.DocumentContent
import com.c2yz.notae.core.DocumentEntry
import com.c2yz.notae.core.Effect
import com.c2yz.notae.core.LibraryView
import com.c2yz.notae.core.NotaeCore
import com.c2yz.notae.core.PickerRow
import com.c2yz.notae.core.PlatformLibrary
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.SetChoice
import com.c2yz.notae.core.SetDialog
import com.c2yz.notae.core.SetFieldError
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.core.StatusMessage
import com.c2yz.notae.platform.formatIsoDate
import com.c2yz.notae.ui.assertAllClickablesLabelled
import com.c2yz.notae.ui.assertClickTargetsAtLeast48dp
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.library.LibraryScreen
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.nio.file.Files
import java.security.MessageDigest
import java.util.Locale
import java.util.TimeZone

/** The Sets list, the set view, the set dialogs, the add-to-set sheet and the add picker. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class SetsScreenTest {
    @get:Rule
    val compose = createComposeRule()

    private val actions = mutableListOf<Action>()
    private var shown by mutableStateOf<Snapshot?>(null)
    private val zone: TimeZone = TimeZone.getDefault()

    @After
    fun restoreZone() = TimeZone.setDefault(zone)

    /** The library screen with the set overlays over it, as the shell draws them. */
    private fun show(snapshot: Snapshot, onAction: (Action) -> Unit = { actions += it }) {
        shown = snapshot
        var density = Density(1f)
        compose.setContent {
            density = LocalDensity.current
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

    private fun dialog(set: SetDialog) = Fixture.sets().copy(setDialog = set)

    @Test
    fun new_set_requires_name() {
        val dir = Files.createTempDirectory("notae-sets").toFile()
        val core = NotaeCore(dir.path, Scores(emptyList()), NoListener)
        try {
            core.dispatch(Action.NewSet)
            show(Fixture.sets().copy(setDialog = core.snapshot().setDialog)) { action ->
                actions += action
                core.dispatch(action)
                shown = Fixture.sets().copy(setDialog = core.snapshot().setDialog)
            }

            compose.onNodeWithText("Only the name is required. You add pieces after saving.").assertIsDisplayed()
            compose.onNodeWithText("Save set").performClick()

            assertEquals(listOf<Action>(Action.SaveSetDialog("", null, null)), actions)
            compose.onNodeWithText("Name is required").assertIsDisplayed()
            // Still open: the header's New set and the dialog's title both read "New set".
            compose.onNodeWithText("Save set").assertIsDisplayed()

            compose.onNodeWithContentDescription("Set name").performTextInput("Mass")
            compose.onNodeWithText("Save set").performClick()

            assertEquals(Action.SaveSetDialog("Mass", null, null), actions.last())
            compose.onNodeWithText("Save set").assertDoesNotExist()
        } finally {
            core.close()
            dir.deleteRecursively()
        }
    }

    @Test
    fun invalid_date_error_shows_as_slint() {
        show(dialog(SetDialog(null, "Mass", "2026-02-30", null, SetFieldError.INVALID_DATE)))

        compose.onNodeWithText("Use a real date as YYYY-MM-DD").assertIsDisplayed()
    }

    @Test
    fun date_picker_returns_iso_and_shows_locale_format() {
        // West of UTC: the picker's UTC-midnight millis read in this zone would be the day before.
        TimeZone.setDefault(TimeZone.getTimeZone("America/Los_Angeles"))
        val locale = Locale.getDefault()
        show(dialog(SetDialog("palm", "Niedziela Palmowa", "2026-03-29", "Msza 10:00, kantor: Anna", null)))

        compose.onNodeWithText("Edit set").assertIsDisplayed()
        compose.onNodeWithText("Only the name is required. You add pieces after saving.").assertDoesNotExist()
        // The sets behind show the same date, so the field is found by its label.
        compose.onNodeWithContentDescription("Date (optional)").assertTextEquals(formatIsoDate("2026-03-29", locale)).performClick()
        // The picker's day cells read "Sunday, March 29, 2026"; the set's own day is the one selected.
        compose.onNode(hasText("March 29, 2026", substring = true) and isSelectable()).assertIsSelected()
        compose.onNode(hasText("March 15, 2026", substring = true) and isSelectable()).performClick()
        compose.onNodeWithText("OK").performClick()

        compose.onNodeWithContentDescription("Date (optional)").assertTextEquals(formatIsoDate("2026-03-15", locale))
        compose.onNodeWithText("Save set").performClick()
        assertEquals(Action.SaveSetDialog("Niedziela Palmowa", "2026-03-15", "Msza 10:00, kantor: Anna"), actions.last())

        compose.onNodeWithContentDescription("Clear date").performClick()
        compose.onNodeWithText("Save set").performClick()
        assertEquals(Action.SaveSetDialog("Niedziela Palmowa", null, "Msza 10:00, kantor: Anna"), actions.last())
    }

    @Test
    fun delete_names_the_set() {
        show(Fixture.sets())

        compose.onNodeWithContentDescription("More actions for Pusty zestaw").performClick()
        compose.onNodeWithText("Edit set").assertIsDisplayed()
        compose.assertAllClickablesLabelled()
        compose.onNodeWithText("Delete set").performClick()
        assertEquals(listOf<Action>(Action.DeleteSet("empty")), actions)
        compose.onNodeWithText("Delete set").assertDoesNotExist()

        shown = Fixture.sets().copy(deleteSet = DeleteSetConfirmation("Pusty zestaw"))
        compose.waitForIdle()
        compose.onNodeWithText("Delete “Pusty zestaw”?").assertIsDisplayed()
        compose.onNodeWithText("The set is removed. Its pieces stay in your library.").assertIsDisplayed()
        compose.assertAllClickablesLabelled()
        compose.onNodeWithText("Delete").performClick()
        compose.onNodeWithText("Cancel").performClick()

        assertEquals(listOf(Action.DeleteSet("empty"), Action.ConfirmDeleteSet, Action.CancelDeleteSet), actions)
    }

    @Test
    fun add_to_set_sheet_checks_member_sets() {
        val ave = Fixture.rows.first { it.title == "Ave Maria" }
        show(Fixture.library().copy(addToSet = AddToSetSheet(ave.noteKey, ave.title, choices)))

        compose.onNodeWithText("Add to set…").assertIsDisplayed()
        compose.onNodeWithText("Niedziela Palmowa").assertIsOn()
        compose.onNodeWithText("Pusty zestaw").assertIsOff()
        compose.onNodeWithText(Fixture.FRIDAY_NAME).assertIsOff()
        compose.onNodeWithText("Pusty zestaw").performClick()
        compose.onNodeWithText("New set").performClick()

        assertEquals(listOf(Action.ToggleSetMembership("empty", ave.noteKey), Action.AddToSetNewSet), actions)
    }

    @Test
    fun picker_untoggle_removes_only_session_item() {
        val dir = Files.createTempDirectory("notae-picker").toFile()
        val core = NotaeCore(dir.path, Scores(listOf("Ave Maria", "Barka")), NoListener)
        try {
            core.libraryRootPicked("Scores", "tree")
            core.await { it.screen == Screen.Library }
            core.dispatch(Action.Rescan)
            val library = core.await { it.banner.message is StatusMessage.Finished && it.rows.size == 2 }
            val ave = library.rows.first { it.title == "Ave Maria" }
            core.dispatch(Action.NewSet)
            core.dispatch(Action.SaveSetDialog("Mass", null, null))
            val setId = core.snapshot().sectionKey.removePrefix("set:")
            // Ave Maria is in the set before the picker opens.
            core.dispatch(Action.AddToSet(ave.noteKey))
            core.dispatch(Action.ToggleSetMembership(setId, ave.noteKey))
            core.dispatch(Action.CloseAddToSet)
            val original = core.snapshot().rows.single().itemId
            core.dispatch(Action.OpenAddPicker)
            show(core.snapshot()) { action ->
                actions += action
                core.dispatch(action)
                shown = core.snapshot()
            }

            compose.onNodeWithText("already in this set").assertIsDisplayed()
            compose.onNodeWithContentDescription("Add Ave Maria to set").performClick()
            assertEquals(2, core.snapshot().rows.size)
            compose.onNodeWithText("1 added at the end of the set").assertIsDisplayed()
            compose.onNodeWithContentDescription("Remove Ave Maria from set").performClick()

            assertEquals(listOf(Action.ToggleAddPickerPiece(ave.noteKey), Action.ToggleAddPickerPiece(ave.noteKey)), actions)
            assertEquals(listOf(original), core.snapshot().rows.map { it.itemId })
            compose.onNodeWithText("0 added at the end of the set").assertIsDisplayed()
            compose.onNodeWithContentDescription("Add Ave Maria to set").assertIsDisplayed()
        } finally {
            core.close()
            dir.deleteRecursively()
        }
    }

    @Test
    fun picker_search_and_done_dispatch() {
        val picker = AddPicker("", listOf(PickerRow("key-1", "Alleluja", null, null, false, false)), 0u)
        show(Fixture.palmSunday().copy(addPicker = picker))

        compose.onNodeWithText("Pieces").assertIsDisplayed()
        compose.onNodeWithContentDescription("Search titles, composers, tags or lyrics").performTextInput("ave")
        compose.onNodeWithText("Done").performClick()

        assertEquals(listOf(Action.AddPickerSearch("ave"), Action.CloseAddPicker), actions)

        shown = Fixture.palmSunday().copy(addPicker = picker.copy(query = "zzz", rows = emptyList()))
        compose.waitForIdle()
        compose.onNodeWithText("No scores here").assertIsDisplayed()
    }

    @Test
    fun sets_list_opens_and_creates_sets() {
        show(Fixture.sets())

        compose.onNodeWithText("3 sets · recently edited first").assertIsDisplayed()
        compose.onNodeWithText("No date").assertIsDisplayed()
        compose.onNodeWithText(formatIsoDate("2026-03-29", Locale.getDefault())).assertIsDisplayed()
        compose.onNodeWithContentDescription("Niedziela Palmowa").performClick()
        compose.onNodeWithText("New set").performClick()

        shown = Fixture.sets().copy(layout = Fixture.sets().layout.copy(libraryView = LibraryView.LIST))
        compose.waitForIdle()
        compose.onNodeWithText("No date · 0 items").assertIsDisplayed()
        compose.onNodeWithContentDescription("More actions for Niedziela Palmowa").performClick()
        compose.onNodeWithText("Edit set").performClick()

        assertEquals(listOf(Action.OpenSet("palm"), Action.NewSet, Action.EditSet("palm")), actions)
    }

    @Test
    fun set_header_shows_subtitle_notes_and_actions() {
        show(Fixture.palmSunday())

        compose.onNodeWithText("${formatIsoDate("2026-03-29", Locale.getDefault())} · 9 pieces").assertIsDisplayed()
        compose.onNodeWithText("Msza 10:00, kantor: Anna").assertIsDisplayed()
        compose.onNodeWithText("New set").assertDoesNotExist()
        compose.onNodeWithContentDescription("Back to sets").performClick()
        compose.onNodeWithText("Add").performClick()

        assertEquals(listOf(Action.SelectSection("sets"), Action.OpenAddPicker), actions)
    }

    @Test
    fun empty_set_says_so() {
        show(Fixture.palmSunday().copy(sectionKey = "set:empty", rows = emptyList(), setEntries = emptyList(), setNotes = null))

        compose.onNodeWithText("This set is empty").assertIsDisplayed()
        compose.onNodeWithText("Pieces you add to this set appear here.").assertIsDisplayed()
    }

    @Test
    fun set_view_keys_work_without_a_click_and_after_dialogs_close() {
        show(Fixture.palmSunday())

        // Nothing clicked first: Ctrl K and Escape work as soon as the set shows.
        compose.onRoot().performKeyInput { withKeyDown(Key.CtrlLeft) { pressKey(Key.K) } }
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        assertEquals(listOf(Action.OpenSearch, Action.SelectSection("sets")), actions)

        // The picker takes focus; once it closes, the keys are back on the screen.
        shown = Fixture.palmSunday().copy(addPicker = AddPicker("", emptyList(), 0u))
        compose.waitForIdle()
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        assertEquals(Action.CloseAddPicker, actions.last())
        shown = Fixture.palmSunday()
        compose.waitForIdle()
        compose.onRoot().performKeyInput { withKeyDown(Key.CtrlLeft) { pressKey(Key.K) } }
        assertEquals(Action.OpenSearch, actions.last())

        // The same after the add-to-set sheet.
        val ave = Fixture.palmRows.first()
        shown = Fixture.palmSunday().copy(addToSet = AddToSetSheet(ave.noteKey, ave.title, choices))
        compose.waitForIdle()
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        assertEquals(Action.CloseAddToSet, actions.last())
        shown = Fixture.palmSunday()
        compose.waitForIdle()
        compose.onRoot().performKeyInput { withKeyDown(Key.CtrlLeft) { pressKey(Key.K) } }
        assertEquals(Action.OpenSearch, actions.last())
    }

    @Test
    fun sets_list_keys_return_after_the_set_dialog_and_menu_close() {
        show(Fixture.sets().copy(setDialog = SetDialog(null, "", null, null, null)))

        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        assertEquals(listOf<Action>(Action.CancelSetDialog), actions)
        shown = Fixture.sets()
        compose.waitForIdle()
        compose.onRoot().performKeyInput { withKeyDown(Key.CtrlLeft) { pressKey(Key.K) } }
        assertEquals(Action.OpenSearch, actions.last())

        compose.onNodeWithContentDescription("More actions for Pusty zestaw").performClick()
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        compose.onNodeWithText("Delete set").assertDoesNotExist()
        compose.onRoot().performKeyInput { withKeyDown(Key.CtrlLeft) { pressKey(Key.K) } }
        assertEquals(listOf(Action.CancelSetDialog, Action.OpenSearch, Action.OpenSearch), actions)
    }

    private val choices = listOf(
        SetChoice("palm", "Niedziela Palmowa", "2026-03-29", 9u, true),
        SetChoice("empty", "Pusty zestaw", null, 0u, false),
        SetChoice("friday", Fixture.FRIDAY_NAME, "2026-04-03", 0u, false),
    )

}

/** Polls the core, whose scan runs on its own threads. */
internal fun NotaeCore.await(done: (Snapshot) -> Boolean): Snapshot {
    val deadline = System.nanoTime() + 30_000_000_000L
    while (true) {
        val latest = snapshot()
        if (done(latest)) return latest
        check(System.nanoTime() < deadline) { "timed out; last snapshot: $latest" }
        Thread.sleep(20)
    }
}

/** A library folder of one-line PDFs named after [titles]; it has no text layer and no thumbnails. */
internal class Scores(private val titles: List<String>) : PlatformLibrary {
    override fun checkRoot(handle: String) = Unit

    override fun enumerate(handle: String): List<DocumentEntry> = titles.map { title ->
        val hex = MessageDigest.getInstance("SHA-256").digest(title.toByteArray()).joinToString("") { "%02x".format(it) }
        DocumentEntry("$title.pdf", "$title.pdf", 100u, DocumentContent.Sha256(hex))
    }

    override fun extractTextLayer(handle: String, path: String): String? = null

    override fun renderThumbnail(handle: String, path: String, outPng: String): Boolean = false
}

internal object NoListener : CoreListener {
    override fun onSnapshot(snapshot: Snapshot) = Unit

    override fun onEffect(effect: Effect) = Unit
}
