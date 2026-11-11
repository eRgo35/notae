package com.c2yz.notae.ui.sets

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Paint
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
import com.c2yz.notae.ui.library.rememberThumbnail
import com.c2yz.notae.ui.theme.NotaeTheme
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Rule
import org.junit.rules.TemporaryFolder
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.io.File
import kotlin.random.Random
import android.graphics.Color as AndroidColor

/** The sets screens' Roborazzi captures (app/src/test/screenshots), named after their docs/compose-parity references. */
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
    // Kyrie eleison has a set note, so the set shows its marker.
    private val palm = Fixture.palmWithSetNote()
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
            Fixture.setCards.map { SetChoice(it.id, it.name, it.date, it.count, checked = it.id == "palm", previews = it.previews) },
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

    // No reference: 0.3.0 had no reorder mode. The grid choice stays saved underneath.
    private val reorder = palm.copy(reordering = true)

    @Test fun reorder_landscape_light() = shot("set-reorder", reorder, landscape = true, dark = false)

    @Test fun reorder_landscape_dark() = shot("set-reorder", reorder, landscape = true, dark = true)

    @Test fun reorder_portrait_light() = shot("set-reorder", reorder, landscape = false, dark = false)

    @Test fun reorder_portrait_dark() = shot("set-reorder", reorder, landscape = false, dark = true)

    // No reference: 0.3.0 had no text notes. The psalm is item 3 and the homily item 6.
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

    // No reference: 0.3.0 had no "Just opened". Kyrie eleison (item 2) was opened last.
    private val justOpened = Fixture.justOpened()
    private val justOpenedList = justOpened.withView(LibraryView.LIST)

    @Test fun just_opened_grid_light() = shot("set-just-opened-grid", justOpened, landscape = true, dark = false)

    @Test fun just_opened_grid_dark() = shot("set-just-opened-grid", justOpened, landscape = true, dark = true)

    @Test fun just_opened_list_light() = shot("set-just-opened-list", justOpenedList, landscape = true, dark = false)

    @Test fun just_opened_list_dark() = shot("set-just-opened-list", justOpenedList, landscape = true, dark = true)

    // No reference: 0.3.0 drew a set as a folder. Each stack shape, with first pages drawn like printed scores.
    @get:Rule
    val pagesDir = TemporaryFolder()

    /** A first page like a printed score: a title, a composer, and systems of five-line staves with notes. */
    private fun scorePage(name: String, seed: Int): String {
        val random = Random(seed)
        val width = 420
        val bitmap = Bitmap.createBitmap(width, 594, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        canvas.drawColor(AndroidColor.WHITE)
        val ink = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = AndroidColor.rgb(40, 40, 40) }
        val title = 140 + random.nextInt(120)
        canvas.drawRect(((width - title) / 2).toFloat(), 40f, ((width + title) / 2).toFloat(), 56f, ink)
        canvas.drawRect(290f, 74f, 380f, 82f, ink)
        ink.strokeWidth = 1.5f
        var top = 112f
        while (top < 540f) {
            for (line in 0 until 5) canvas.drawLine(36f, top + line * 7, 384f, top + line * 7, ink)
            var x = 56f
            while (x < 370f) {
                val y = top + random.nextInt(9) * 3.5f
                canvas.drawOval(x - 4f, y - 3f, x + 4f, y + 3f, ink)
                canvas.drawLine(x + 3.5f, y, x + 3.5f, y - 22f, ink)
                x += 18f + random.nextInt(4) * 6f
            }
            top += 62f
        }
        val file = File(pagesDir.root, name)
        file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        // The sandbox's first native decode must run on the test thread: one on the IO thread aborts the process.
        BitmapFactory.decodeFile(file.path)
        return file.path
    }

    private val pages by lazy { listOf(scorePage("kyrie.png", 1), scorePage("gloria.png", 2), scorePage("sanctus.png", 3)) }

    /** Like [shot] at a named window, once the pages are decoded, so the captures never catch a fade. */
    private fun stackShot(state: String, window: String, dark: Boolean, snapshot: (List<String>) -> Snapshot, before: () -> Unit = {}) {
        RuntimeEnvironment.setQualifiers(WINDOWS.getValue(window))
        shown = snapshot(pages)
        var decoded = false
        compose.setContent {
            decoded = pages.map { rememberThumbnail(it).value }.all { it != null }
            NotaeTheme(dark = dark) {
                if (decoded) {
                    shown?.let {
                        Box {
                            LibraryScreen(it, {})
                            SetOverlays(it, {})
                        }
                    }
                }
            }
        }
        compose.waitUntil(5_000) { decoded }
        before()
        compose.waitForIdle()
        compose.onRoot().captureRoboImage("src/test/screenshots/$state-$window-${if (dark) "dark" else "light"}.png")
    }

    private val stackGrid = { pages: List<String> -> Fixture.stackSets(pages) }
    private val stackList = { pages: List<String> -> Fixture.stackSets(pages).withView(LibraryView.LIST) }
    private val stackChecklist = { pages: List<String> ->
        Fixture.library().copy(
            addToSet = AddToSetSheet(
                ave.noteKey,
                ave.title,
                Fixture.stackCards(pages).map { SetChoice(it.id, it.name, it.date, it.count, it.id == "palm", it.previews) },
            ),
        )
    }

    @Test fun stack_grid_phone_light() = stackShot("set-stack-grid", "phone", dark = false, stackGrid)

    @Test fun stack_grid_phone_dark() = stackShot("set-stack-grid", "phone", dark = true, stackGrid)

    @Test fun stack_grid_rail_light() = stackShot("set-stack-grid", "rail", dark = false, stackGrid)

    @Test fun stack_grid_rail_dark() = stackShot("set-stack-grid", "rail", dark = true, stackGrid)

    @Test fun stack_grid_landscape_light() = stackShot("set-stack-grid", "landscape", dark = false, stackGrid)

    @Test fun stack_grid_landscape_dark() = stackShot("set-stack-grid", "landscape", dark = true, stackGrid)

    @Test fun stack_grid_portrait_light() = stackShot("set-stack-grid", "portrait", dark = false, stackGrid)

    @Test fun stack_grid_portrait_dark() = stackShot("set-stack-grid", "portrait", dark = true, stackGrid)

    @Test fun stack_list_light() = stackShot("set-stack-list", "landscape", dark = false, stackList)

    @Test fun stack_list_dark() = stackShot("set-stack-list", "landscape", dark = true, stackList)

    @Test fun stack_list_phone_light() = stackShot("set-stack-list", "phone", dark = false, stackList)

    @Test fun stack_list_phone_dark() = stackShot("set-stack-list", "phone", dark = true, stackList)

    @Test fun stack_checklist_light() = stackShot("set-stack-checklist", "landscape", dark = false, stackChecklist)

    @Test fun stack_checklist_dark() = stackShot("set-stack-checklist", "landscape", dark = true, stackChecklist)

    @Test fun stack_set_menu_light() = stackShot("set-stack-menu", "landscape", dark = false, stackGrid, before = openSetMenu)

    @Test fun stack_set_menu_dark() = stackShot("set-stack-menu", "landscape", dark = true, stackGrid, before = openSetMenu)

    private companion object {
        /** The phone, a phone on its side with the rail, and the tablet both ways. */
        val WINDOWS = mapOf(
            "phone" to "+w384dp-h832dp-xhdpi",
            "rail" to "+w700dp-h500dp-xhdpi",
            "landscape" to "+w1280dp-h800dp-xhdpi",
            "portrait" to "+w800dp-h1280dp-xhdpi",
        )
    }
}
