package com.c2yz.notae.ui.sets

import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToIndex
import androidx.compose.ui.test.performTextInput
import com.c2yz.notae.core.AddPicker
import com.c2yz.notae.core.AddToSetSheet
import com.c2yz.notae.core.DeleteSetConfirmation
import com.c2yz.notae.core.LibraryView
import com.c2yz.notae.core.PickerRow
import com.c2yz.notae.core.SetChoice
import com.c2yz.notae.core.SetDialog
import com.c2yz.notae.core.SetFieldError
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.library.GRID_TAG
import com.c2yz.notae.ui.library.LIST_TAG
import com.c2yz.notae.ui.library.LibraryScreen
import com.c2yz.notae.ui.theme.NotaeTheme
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** The sets screens' Roborazzi captures (app/src/test/screenshots), named after their docs/compose-parity/slint/ references. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class SetsScreenshotTest {
    @get:Rule
    val compose = createComposeRule()

    private var shown by mutableStateOf<Snapshot?>(null)

    private fun shot(state: String, snapshot: Snapshot, landscape: Boolean, dark: Boolean, before: () -> Unit = {}) {
        RuntimeEnvironment.setQualifiers(if (landscape) "+w1280dp-h800dp-xhdpi" else "+w800dp-h1280dp-xhdpi")
        shown = snapshot
        compose.setContent {
            NotaeTheme(dark = dark) {
                shown?.let {
                    Box {
                        LibraryScreen(it, {})
                        SetOverlays(it, {})
                    }
                }
            }
        }
        before()
        compose.waitForIdle()
        val orientation = if (landscape) "landscape" else "portrait"
        compose.onRoot().captureRoboImage("src/test/screenshots/$state-$orientation-${if (dark) "dark" else "light"}.png")
    }

    private fun Snapshot.withView(view: LibraryView) = copy(layout = layout.copy(libraryView = view))

    private val sets = Fixture.sets()
    private val setsList = sets.withView(LibraryView.LIST)
    private val palm = Fixture.palmSunday()
    private val palmList = palm.withView(LibraryView.LIST)
    private val empty = palm.copy(
        sectionKey = "set:empty",
        title = com.c2yz.notae.core.SectionTitle.Set("Pusty zestaw"),
        subtitle = com.c2yz.notae.core.Subtitle.Set(null, 0u, 0u),
        setNotes = null,
        rows = emptyList(),
        setEntries = emptyList(),
    )
    private val newSet = sets.copy(setDialog = SetDialog(null, "", null, null, null))
    private val editSet = sets.copy(setDialog = SetDialog("palm", "Niedziela Palmowa", "2026-03-29", "Msza 10:00, kantor: Anna", null))
    private val nameError = sets.copy(setDialog = SetDialog(null, "", null, null, SetFieldError.EMPTY_NAME))
    private val deleteConfirm = sets.copy(deleteSet = DeleteSetConfirmation("Pusty zestaw"))
    private val ave = Fixture.rows.first { it.title == "Ave Maria" }
    private val addToSet = Fixture.library().copy(
        addToSet = AddToSetSheet(
            ave.noteKey,
            ave.title,
            Fixture.setCards.map { SetChoice(it.id, it.name, it.date, it.count, checked = it.id == "palm") },
        ),
    )
    private val inPalm = Fixture.palmRows.mapNotNull { row -> row.takeUnless { it.missing }?.title }.toSet()
    private fun pickerRow(row: com.c2yz.notae.core.Row) = PickerRow(row.noteKey, row.title, row.number, row.composer, row.title in inPalm, false)
    private val pickerEmpty = palm.copy(addPicker = AddPicker("", Fixture.rows.map(::pickerRow), 0u))
    private val pickerResults = palm.copy(
        addPicker = AddPicker("ave", Fixture.rows.filter { it.title.startsWith("Ave") }.map(::pickerRow), 0u),
    )
    private val openSetMenu = { compose.onNodeWithContentDescription("More actions for Niedziela Palmowa").performClick(); Unit }
    private val openItemMenu = { compose.onNodeWithContentDescription("More actions for Kyrie eleison").performClick(); Unit }
    private val scrollGrid = { compose.onNodeWithTag(GRID_TAG).performScrollToIndex(8); Unit }
    private val scrollList = { compose.onNodeWithTag(LIST_TAG).performScrollToIndex(8); Unit }

    @Test fun sets_grid_landscape_light() = shot("sets-grid", sets, landscape = true, dark = false)

    @Test fun sets_grid_landscape_dark() = shot("sets-grid", sets, landscape = true, dark = true)

    @Test fun sets_grid_portrait_light() = shot("sets-grid", sets, landscape = false, dark = false)

    @Test fun sets_grid_portrait_dark() = shot("sets-grid", sets, landscape = false, dark = true)

    @Test fun sets_list_light() = shot("sets-list", setsList, landscape = true, dark = false)

    @Test fun sets_list_dark() = shot("sets-list", setsList, landscape = true, dark = true)

    @Test fun inside_grid_landscape_light() = shot("set-inside-grid", palm, landscape = true, dark = false)

    @Test fun inside_grid_landscape_dark() = shot("set-inside-grid", palm, landscape = true, dark = true)

    @Test fun inside_grid_portrait_light() = shot("set-inside-grid", palm, landscape = false, dark = false)

    @Test fun inside_grid_portrait_dark() = shot("set-inside-grid", palm, landscape = false, dark = true)

    @Test fun inside_list_landscape_light() = shot("set-inside-list", palmList, landscape = true, dark = false)

    @Test fun inside_list_landscape_dark() = shot("set-inside-list", palmList, landscape = true, dark = true)

    @Test fun inside_list_portrait_light() = shot("set-inside-list", palmList, landscape = false, dark = false)

    @Test fun inside_list_portrait_dark() = shot("set-inside-list", palmList, landscape = false, dark = true)

    @Test fun inside_grid_scrolled_light() = shot("set-inside-grid-scrolled", palm, landscape = true, dark = false, before = scrollGrid)

    @Test fun inside_grid_scrolled_dark() = shot("set-inside-grid-scrolled", palm, landscape = true, dark = true, before = scrollGrid)

    @Test fun inside_list_scrolled_light() = shot("set-inside-list-scrolled", palmList, landscape = true, dark = false, before = scrollList)

    @Test fun inside_list_scrolled_dark() = shot("set-inside-list-scrolled", palmList, landscape = true, dark = true, before = scrollList)

    @Test fun inside_empty_light() = shot("set-inside-empty", empty, landscape = true, dark = false)

    @Test fun inside_empty_dark() = shot("set-inside-empty", empty, landscape = true, dark = true)

    @Test fun dialog_new_light() = shot("set-dialog-new", newSet, landscape = true, dark = false)

    @Test fun dialog_new_dark() = shot("set-dialog-new", newSet, landscape = true, dark = true)

    @Test fun dialog_edit_light() = shot("set-dialog-edit", editSet, landscape = true, dark = false)

    @Test fun dialog_edit_dark() = shot("set-dialog-edit", editSet, landscape = true, dark = true)

    @Test fun dialog_error_light() = shot("set-dialog-error", nameError, landscape = true, dark = false)

    @Test fun dialog_error_dark() = shot("set-dialog-error", nameError, landscape = true, dark = true)

    @Test fun delete_confirm_light() = shot("set-delete-confirm", deleteConfirm, landscape = true, dark = false)

    @Test fun delete_confirm_dark() = shot("set-delete-confirm", deleteConfirm, landscape = true, dark = true)

    @Test fun sheet_add_to_set_light() = shot("sheet-add-to-set", addToSet, landscape = true, dark = false)

    @Test fun sheet_add_to_set_dark() = shot("sheet-add-to-set", addToSet, landscape = true, dark = true)

    @Test fun sheet_set_menu_light() = shot("sheet-set-menu", sets, landscape = true, dark = false, before = openSetMenu)

    @Test fun sheet_set_menu_dark() = shot("sheet-set-menu", sets, landscape = true, dark = true, before = openSetMenu)

    @Test fun sheet_set_item_light() = shot("sheet-set-item", palmList, landscape = true, dark = false, before = openItemMenu)

    @Test fun sheet_set_item_dark() = shot("sheet-set-item", palmList, landscape = true, dark = true, before = openItemMenu)

    @Test fun add_picker_empty_light() = shot("add-picker-empty", pickerEmpty, landscape = true, dark = false)

    @Test fun add_picker_empty_dark() = shot("add-picker-empty", pickerEmpty, landscape = true, dark = true)

    @Test fun add_picker_results_light() = shot("add-picker-results", pickerResults, landscape = true, dark = false)

    @Test fun add_picker_results_dark() = shot("add-picker-results", pickerResults, landscape = true, dark = true)

    // No Slint reference: 0.3.0 had no reorder mode. The grid choice stays saved underneath.
    private val reorder = palm.copy(reordering = true)

    @Test fun reorder_landscape_light() = shot("set-reorder", reorder, landscape = true, dark = false)

    @Test fun reorder_landscape_dark() = shot("set-reorder", reorder, landscape = true, dark = true)

    @Test fun reorder_portrait_light() = shot("set-reorder", reorder, landscape = false, dark = false)

    @Test fun reorder_portrait_dark() = shot("set-reorder", reorder, landscape = false, dark = true)

    // No Slint reference: 0.3.0 had no text notes. The psalm is item 3 and the homily item 6.
    private val noted = Fixture.palmWithNotes()
    private val notedList = noted.withView(LibraryView.LIST)
    private val notedReorder = noted.copy(reordering = true)
    private val noteTab = noted.copy(addPicker = AddPicker("", Fixture.rows.map(::pickerRow), 0u))
    private val typeNote = {
        compose.onNodeWithText("Text note").performClick()
        compose.onNodeWithContentDescription("Note").performTextInput("Psalm responsoryjny")
        Unit
    }
    private val editNote = {
        compose.onNodeWithContentDescription("More actions for ${Fixture.PSALM_NAME}").performClick()
        compose.onNodeWithText("Edit note").performClick()
        Unit
    }

    @Test fun notes_grid_landscape_light() = shot("set-notes-grid", noted, landscape = true, dark = false)

    @Test fun notes_grid_landscape_dark() = shot("set-notes-grid", noted, landscape = true, dark = true)

    @Test fun notes_grid_portrait_light() = shot("set-notes-grid", noted, landscape = false, dark = false)

    @Test fun notes_grid_portrait_dark() = shot("set-notes-grid", noted, landscape = false, dark = true)

    @Test fun notes_list_landscape_light() = shot("set-notes-list", notedList, landscape = true, dark = false)

    @Test fun notes_list_landscape_dark() = shot("set-notes-list", notedList, landscape = true, dark = true)

    @Test fun notes_list_portrait_light() = shot("set-notes-list", notedList, landscape = false, dark = false)

    @Test fun notes_list_portrait_dark() = shot("set-notes-list", notedList, landscape = false, dark = true)

    @Test fun notes_reorder_landscape_light() = shot("set-notes-reorder", notedReorder, landscape = true, dark = false)

    @Test fun notes_reorder_landscape_dark() = shot("set-notes-reorder", notedReorder, landscape = true, dark = true)

    @Test fun notes_reorder_portrait_light() = shot("set-notes-reorder", notedReorder, landscape = false, dark = false)

    @Test fun notes_reorder_portrait_dark() = shot("set-notes-reorder", notedReorder, landscape = false, dark = true)

    @Test fun add_picker_note_landscape_light() = shot("add-picker-note", noteTab, landscape = true, dark = false, before = typeNote)

    @Test fun add_picker_note_landscape_dark() = shot("add-picker-note", noteTab, landscape = true, dark = true, before = typeNote)

    @Test fun add_picker_note_portrait_light() = shot("add-picker-note", noteTab, landscape = false, dark = false, before = typeNote)

    @Test fun add_picker_note_portrait_dark() = shot("add-picker-note", noteTab, landscape = false, dark = true, before = typeNote)

    @Test fun note_editor_landscape_light() = shot("note-editor", notedList, landscape = true, dark = false, before = editNote)

    @Test fun note_editor_landscape_dark() = shot("note-editor", notedList, landscape = true, dark = true, before = editNote)

    @Test fun note_editor_portrait_light() = shot("note-editor", notedList, landscape = false, dark = false, before = editNote)

    @Test fun note_editor_portrait_dark() = shot("note-editor", notedList, landscape = false, dark = true, before = editNote)
}
