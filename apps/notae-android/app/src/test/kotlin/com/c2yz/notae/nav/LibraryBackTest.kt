package com.c2yz.notae.nav

import com.c2yz.notae.core.ImportConfirmation
import androidx.activity.BackEventCompat
import androidx.activity.ComponentActivity
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.c2yz.notae.core.PositionOf
import com.c2yz.notae.core.SectionTitle
import com.c2yz.notae.core.Selection
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Back closes the library's ⋮ sheet, which only the screen knows about, before the core is asked, whatever the stack
 * was before; and Back in reorder mode goes to the core, without previewing the screen below.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class LibraryBackTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun back_closes_the_score_sheet_first() {
        var backs = 0
        compose.setContent {
            NotaeTheme { NotaeNavDisplay(Fixture.library(), onAction = {}, onBack = { backs++ }) }
        }
        compose.onNodeWithContentDescription("More actions for Ave Maria").performClick()
        compose.onNodeWithText("Open score").assertExists()

        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()

        compose.onNodeWithText("Open score").assertDoesNotExist()
        assertEquals(0, backs)

        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
        assertEquals(1, backs)
    }

    private var snapshot by mutableStateOf(Fixture.library())
    private var backs = 0

    private fun show(first: Snapshot) {
        snapshot = first
        compose.setContent { NotaeTheme { NotaeNavDisplay(snapshot, onAction = {}, onBack = { backs++ }) } }
        compose.waitForIdle()
    }

    private fun back() {
        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
    }

    /** [from] with the details of its first row open beside it, as the core shows them. */
    private fun withDetails(from: Snapshot): Snapshot = from.copy(
        selection = Selection(row = from.rows[0], previous = null, next = from.rows.getOrNull(1)?.selectionId, position = PositionOf(1u, from.rows.size.toUInt())),
    )

    /** Opens and closes the details, so the stack changes under a library entry that stays composed. */
    private fun openAndCloseDetails(from: Snapshot) {
        show(from)
        snapshot = withDetails(from)
        compose.waitForIdle()
        snapshot = from
        compose.waitForIdle()
    }

    @Test
    fun back_closes_the_score_sheet_after_the_details_closed() {
        openAndCloseDetails(Fixture.library())
        compose.onNodeWithContentDescription("More actions for Ave Maria").performClick()
        compose.onNodeWithText("Open score").assertExists()

        back()

        compose.onNodeWithText("Open score").assertDoesNotExist()
        assertEquals("the core is not asked while the sheet is open", 0, backs)
    }

    @Test
    fun back_goes_to_a_dialog_that_opens_over_the_score_sheet() {
        // A set file arriving while the sheet is open shows its import review on top: Back closes that first.
        show(Fixture.library())
        compose.onNodeWithContentDescription("More actions for Ave Maria").performClick()
        snapshot = Fixture.library().copy(importConfirmation = ImportConfirmation(sets = 1u, replaced = emptyList(), shared = null))
        compose.waitForIdle()

        back()

        assertEquals("the core is asked to close the dialog", 1, backs)
    }

    @Test
    fun back_closes_the_score_sheet_in_favorites_after_the_details_closed() {
        val favorites = Fixture.library().copy(sectionKey = "favorites", title = SectionTitle.Favorites, rows = Fixture.rows.filter { it.favorite })
        openAndCloseDetails(favorites)
        val title = favorites.rows.first().title
        compose.onNodeWithContentDescription("More actions for $title").performClick()
        compose.onNodeWithText("Open score").assertExists()

        back()

        compose.onNodeWithText("Open score").assertDoesNotExist()
        assertEquals("Favorites stays: the core is not asked", 0, backs)
    }

    @Test
    fun back_in_reorder_mode_after_the_details_closed_goes_to_the_core_without_a_preview() {
        val reordering = Fixture.palmSunday().copy(reordering = true)
        openAndCloseDetails(reordering)
        val atRest = compose.onAllNodesWithText("Kyrie eleison").fetchSemanticsNodes().map { it.boundsInWindow }

        // A back gesture halfway: Navigation 3 would preview the list of sets coming back; reorder mode shows none.
        val dispatcher = compose.activity.onBackPressedDispatcher
        compose.runOnUiThread { dispatcher.dispatchOnBackStarted(BackEventCompat(0f, 0f, 0f, BackEventCompat.EDGE_LEFT)) }
        compose.waitForIdle()
        compose.runOnUiThread { dispatcher.dispatchOnBackProgressed(BackEventCompat(0f, 0f, 0.5f, BackEventCompat.EDGE_LEFT)) }
        compose.waitForIdle()
        val midway = compose.onAllNodesWithText("Kyrie eleison").fetchSemanticsNodes().map { it.boundsInWindow }
        compose.runOnUiThread { dispatcher.onBackPressed() }
        compose.waitForIdle()

        assertEquals("the set does not move under the gesture", atRest, midway)
        assertEquals("the core ends reorder mode", 1, backs)
        compose.onAllNodesWithText("Kyrie eleison").fetchSemanticsNodes().also { assertEquals(atRest.size, it.size) }
    }
}
