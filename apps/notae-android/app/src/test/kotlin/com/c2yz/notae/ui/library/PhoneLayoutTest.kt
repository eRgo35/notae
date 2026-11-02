package com.c2yz.notae.ui.library

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.and
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.Density
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.LibraryView
import com.c2yz.notae.core.PositionOf
import com.c2yz.notae.core.Selection
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.ui.assertAllClickablesLabelled
import com.c2yz.notae.ui.assertClickTargetsAtLeast48dp
import com.c2yz.notae.ui.details.DetailsScreen
import com.c2yz.notae.ui.theme.NotaeTheme
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The library and the details on a phone (a 384 × 832 dp window): the bottom bar, the toolbar's ⋮, the composer sheet,
 * the floating button, Done while reordering and the details' bottom bar; labels and 48 dp targets as on a tablet.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w384dp-h832dp-xhdpi")
class PhoneLayoutTest {
    @get:Rule
    val compose = createComposeRule()

    private val actions = mutableListOf<Action>()
    private var shown by mutableStateOf<Snapshot?>(null)
    private var dark by mutableStateOf(false)

    private fun show(snapshot: Snapshot, details: Boolean = false) {
        shown = snapshot
        var density = Density(1f)
        compose.setContent {
            density = LocalDensity.current
            NotaeTheme(dark = dark) {
                shown?.let {
                    if (details) DetailsScreen(it, { action -> actions += action }) else LibraryScreen(it, { action -> actions += action })
                }
            }
        }
        compose.assertAllClickablesLabelled()
        compose.assertClickTargetsAtLeast48dp(density)
    }

    private val radio = SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.RadioButton)

    @Test
    fun bottom_bar_selects_sections() {
        show(Fixture.library())

        compose.onNodeWithText("Favorites").assertIsDisplayed().performClick()

        assertEquals(listOf<Action>(Action.SelectSection("favorites")), actions)
    }

    @Test
    fun bottom_bar_keeps_all_scores_selected_for_a_composer() {
        show(Fixture.library().copy(sectionKey = "composer:bach"))

        compose.onNode(hasText("All scores") and SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Tab)).assertIsSelected()
    }

    @Test
    fun overflow_menu_holds_scan_view_and_settings() {
        show(Fixture.library().withView(LibraryView.GRID))
        compose.onNodeWithTag(GRID_TAG).assertIsDisplayed()

        compose.onNodeWithContentDescription("More options").performClick()
        compose.onNodeWithText("Scan library").performClick()
        compose.onNodeWithContentDescription("More options").performClick()
        compose.onNodeWithText("Switch to list view").performClick()
        compose.onNodeWithContentDescription("More options").performClick()
        compose.onNodeWithText("Settings").performClick()

        assertEquals(
            listOf(Action.Rescan, Action.SetLibraryView(LibraryView.LIST), Action.OpenSettings),
            actions,
        )
    }

    @Test
    fun composer_sheet_filters_the_library() {
        show(Fixture.library())

        compose.onNodeWithContentDescription("Choose composer, All composers").performClick()
        compose.onNode(hasText("All composers") and radio).assertIsSelected()
        compose.onNode(hasText("Palestrina") and radio).performClick()

        assertEquals(listOf<Action>(Action.SelectSection("composer:palestrina")), actions)
        compose.onNodeWithText("Composers").assertDoesNotExist()
    }

    @Test
    fun floating_button_makes_a_set_or_adds_to_one() {
        show(Fixture.sets())
        compose.onNodeWithText("New set").performClick()

        shown = Fixture.palmSunday()
        compose.waitForIdle()
        compose.onNodeWithText("Add").performClick()
        compose.onNodeWithContentDescription("Reorder").performClick()
        compose.onNodeWithContentDescription("Back to sets").performClick()

        assertEquals(
            listOf(Action.NewSet, Action.OpenAddPicker, Action.ReorderStarted, Action.SelectSection("sets")),
            actions,
        )
    }

    @Test
    fun reordering_ends_with_done_at_the_bottom() {
        show(Fixture.palmSunday().copy(reordering = true))

        compose.onNodeWithTag(REORDER_TAG).assertIsDisplayed()
        compose.onNodeWithText("Favorites").assertDoesNotExist()
        compose.onNodeWithText("Add").assertDoesNotExist()
        compose.onNodeWithText("Done").performClick()

        assertEquals(listOf<Action>(Action.ReorderFinished), actions)
    }

    @Test
    fun details_step_and_open_from_the_bottom_bar() {
        show(details(), details = true)

        compose.onNodeWithText("Open score").assertIsDisplayed().performClick()
        compose.onNodeWithContentDescription("Next score").performClick()
        compose.onNodeWithContentDescription("Previous score").performClick()
        compose.onNodeWithText("Edit details").performClick()
        compose.onNodeWithText("Add to set…").performClick()

        val row = details().selection!!.row
        assertTrue(actions[0] is Action.OpenPdf)
        assertEquals(
            listOf(Action.SelectScore("key-3"), Action.SelectScore("key-1"), Action.EditMetadata(row.noteKey), Action.AddToSet(row.noteKey)),
            actions.drop(1),
        )
    }

    private fun details(): Snapshot {
        val library = Fixture.library()
        val row = library.rows[2]
        return library.copy(selection = Selection(row, previous = "key-1", next = "key-3", position = PositionOf(3u, 20u)))
    }

    // Screenshots (app/src/test/screenshots) of the phone layouts.

    private fun shot(name: String, snapshot: Snapshot, dark: Boolean, details: Boolean = false) {
        this.dark = dark
        shown = snapshot
        compose.setContent {
            NotaeTheme(dark = this.dark) {
                shown?.let {
                    if (details) DetailsScreen(it, { action -> actions += action }) else LibraryScreen(it, { action -> actions += action })
                }
            }
        }
        compose.waitForIdle()
        compose.onRoot().captureRoboImage("src/test/screenshots/phone-$name-${if (dark) "dark" else "light"}.png")
    }

    private fun Snapshot.withView(view: LibraryView) = copy(layout = layout.copy(libraryView = view))

    @Test fun shot_library_grid_light() = shot("library-grid", Fixture.library().withView(LibraryView.GRID), dark = false)

    @Test fun shot_library_grid_dark() = shot("library-grid", Fixture.library().withView(LibraryView.GRID), dark = true)

    @Test fun shot_library_list_light() = shot("library-list", Fixture.library().withView(LibraryView.LIST), dark = false)

    @Test fun shot_sets_light() = shot("sets", Fixture.sets(), dark = false)

    @Test fun shot_set_list_light() = shot("set-list", Fixture.justOpened().withView(LibraryView.LIST), dark = false)

    @Test fun shot_set_reorder_light() = shot("set-reorder", Fixture.palmSunday().copy(reordering = true), dark = false)

    @Test fun shot_details_light() = shot("details", details(), dark = false, details = true)

    @Test fun shot_details_dark() = shot("details", details(), dark = true, details = true)
}
