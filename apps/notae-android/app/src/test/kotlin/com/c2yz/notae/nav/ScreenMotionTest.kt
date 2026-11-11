package com.c2yz.notae.nav

import androidx.activity.ComponentActivity
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import com.c2yz.notae.core.PositionOf
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.Selection
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.ui.details.DETAILS_BODY_TAG
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.library.GRID_TAG
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
 * Only content moves when the screen changes: halfway through a change (the clock paused), the rail or the side panel,
 * the bottom bar and the top bars sit where they sit at rest, while the content is off its place. Tablet landscape (two
 * panes), tablet portrait and phone; the core stands in as the snapshot.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class ScreenMotionTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private var snapshot by mutableStateOf(Fixture.library())

    private fun show(first: Snapshot) {
        snapshot = first
        compose.setContent { NotaeTheme { NotaeNavDisplay(snapshot, onAction = {}, onBack = {}) } }
        compose.waitForIdle()
    }

    /** Sends [next] and stops the clock early in the 300 ms change, while everything that moves is well off its place. */
    private fun halfway(next: Snapshot) {
        compose.mainClock.autoAdvance = false
        snapshot = next
        // In steps, each one composing and laying out: the change starts on the frame after the one that composes it.
        repeat(HALFWAY_STEPS) {
            compose.mainClock.advanceTimeBy(STEP_MILLIS)
            compose.waitForIdle()
        }
    }

    private fun finish() {
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()
    }

    /**
     * Where each [matcher] node shows. Moving content is clipped to its own place, so content leaving toward the start
     * shows as its end edge moving, and content toward the end as its start edge.
     */
    private fun bounds(matcher: SemanticsMatcher): List<Rect> =
        compose.onAllNodes(matcher, useUnmergedTree = true).fetchSemanticsNodes().map { it.boundsInRoot }

    /** Every [matcher] node in the middle of the change sits where one sits at rest, before or after it. */
    private fun assertStill(matcher: SemanticsMatcher, rest: List<Rect>, middle: List<Rect>) {
        assertTrue("$matcher shows in the middle of the change", middle.isNotEmpty())
        middle.forEach { assertTrue("$matcher at $it, at rest only at $rest", it in rest) }
    }

    private val rows = Fixture.rows
    private val ave = rows.indexOfFirst { it.title == "Ave Maria" }

    private fun selecting(index: Int, from: Snapshot = Fixture.library()): Snapshot = from.copy(
        selection = Selection(
            row = rows[index],
            previous = rows.getOrNull(index - 1)?.selectionId,
            next = rows.getOrNull(index + 1)?.selectionId,
            position = PositionOf((index + 1).toUInt(), rows.size.toUInt()),
        ),
    )

    private val search = hasContentDescription("Search")
    private val favorites = hasText("Favorites")
    private val sets = hasText("Sets")
    private val grid = hasTestTag(GRID_TAG)
    private val palmCard = hasContentDescription("More actions for Niedziela Palmowa")
    private val detailsBack = hasContentDescription("Back to library")
    private val settingsTitle = hasText("Settings")
    private val libraryFolder = hasText("Library folder")

    @Test
    @Config(qualifiers = "w1280dp-h800dp-xhdpi")
    fun landscape_section_switch_moves_only_the_content() {
        show(Fixture.library())
        val searchAtRest = bounds(search)
        val panelAtRest = bounds(favorites)
        val gridAtRest = bounds(grid).single()

        halfway(Fixture.sets())
        val middle = Triple(bounds(search), bounds(favorites), bounds(grid).single())
        val cardMiddle = bounds(palmCard).single()
        compose.onRoot().captureRoboImage("src/test/screenshots/motion-section-landscape-light.png")
        finish()
        val cardAtRest = bounds(palmCard).single()

        assertStill(search, searchAtRest + bounds(search), middle.first)
        assertStill(favorites, panelAtRest + bounds(favorites), middle.second)
        // All scores leaves toward the start and Sets, later in the rail, comes in from the end.
        assertTrue("the grid leaves to the start: $gridAtRest -> ${middle.third}", middle.third.right < gridAtRest.right - 100f)
        assertTrue("the sets come from the end: $cardAtRest <- $cardMiddle", cardMiddle.left > cardAtRest.left + 100f)
    }

    @Test
    @Config(qualifiers = "w1280dp-h800dp-xhdpi")
    fun landscape_details_pane_slides_in_and_out_beside_a_still_list() {
        show(Fixture.library())

        halfway(selecting(ave))
        val gridMiddle = bounds(grid).single()
        val backMiddle = bounds(detailsBack).single()
        finish()
        val gridOpen = bounds(grid).single()
        val backOpen = bounds(detailsBack).single()
        assertEquals("the list does not move while the pane opens", gridOpen, gridMiddle)
        assertTrue("the pane comes in from the end: $backOpen <- $backMiddle", backMiddle.left > backOpen.left + 100f)

        halfway(Fixture.library())
        val gridClosing = bounds(grid).single()
        val backClosing = bounds(detailsBack).single()
        finish()
        assertEquals("the list does not move while the pane closes", gridOpen, gridClosing)
        assertTrue("the pane leaves to the end: $backOpen -> $backClosing", backClosing.left > backOpen.left + 100f)
        assertTrue("the pane is gone", bounds(detailsBack).isEmpty())
    }

    @Test
    @Config(qualifiers = "w800dp-h1280dp-xhdpi")
    fun portrait_settings_slide_in_and_back_beside_a_still_rail_and_top_bar() {
        show(Fixture.library())
        val railAtRest = bounds(favorites)

        halfway(Fixture.library().copy(screen = Screen.Settings))
        val railMiddle = bounds(favorites)
        val titleMiddle = bounds(settingsTitle)
        val folderMiddle = bounds(libraryFolder).single()
        compose.onRoot().captureRoboImage("src/test/screenshots/motion-settings-portrait-light.png")
        finish()
        val titleAtRest = bounds(settingsTitle)
        val folderAtRest = bounds(libraryFolder).single()

        // The rail stays put under the Settings content that slides over it; Settings' own top bar never moves.
        assertStill(favorites, railAtRest, railMiddle)
        assertStill(settingsTitle, titleAtRest, titleMiddle)
        assertTrue("Settings' content comes in from the end: $folderAtRest <- $folderMiddle", folderMiddle.left > folderAtRest.left + 100f)

        // Back reverses it: the content leaves to the end, the top bar stays.
        halfway(Fixture.library())
        val titleBack = bounds(settingsTitle)
        val folderBack = bounds(libraryFolder).single()
        val railBack = bounds(favorites)
        finish()
        assertStill(settingsTitle, titleAtRest, titleBack)
        assertStill(favorites, railAtRest, railBack)
        assertTrue("Settings' content leaves to the end: $folderAtRest -> $folderBack", folderBack.left > folderAtRest.left + 100f)
        assertTrue("Settings are gone", bounds(libraryFolder).isEmpty())
    }

    @Test
    @Config(qualifiers = "w800dp-h1280dp-xhdpi")
    fun portrait_rail_stays_while_a_set_opens() {
        show(Fixture.sets())
        val railAtRest = bounds(favorites) + bounds(sets)

        halfway(Fixture.palmSunday())
        val railMiddle = bounds(favorites) + bounds(sets)
        val piecesMiddle = bounds(grid).single()
        finish()
        val piecesAtRest = bounds(grid).single()

        assertTrue("the rail shows in the middle of the change", railMiddle.isNotEmpty())
        railMiddle.forEach { assertTrue("rail at $it, at rest at $railAtRest", it in railAtRest) }
        assertTrue("the set's pieces come in from the end: $piecesAtRest <- $piecesMiddle", piecesMiddle.left > piecesAtRest.left + 100f)
    }

    @Test
    @Config(qualifiers = "w384dp-h832dp-xhdpi")
    fun phone_bars_stay_while_sections_and_details_change() {
        show(Fixture.library())
        val barAtRest = bounds(favorites)
        val gridAtRest = bounds(grid).single()

        halfway(Fixture.sets())
        val barMiddle = bounds(favorites)
        val gridMiddle = bounds(grid).single()
        finish()
        assertStill(favorites, barAtRest, barMiddle)
        assertTrue("the grid leaves to the start: $gridAtRest -> $gridMiddle", gridMiddle.right < gridAtRest.right - 100f)

        snapshot = Fixture.library()
        finish()
        halfway(selecting(ave))
        val backMiddle = bounds(detailsBack)
        val openMiddle = bounds(hasText("Open score"))
        val bodyMiddle = bounds(hasTestTag(DETAILS_BODY_TAG)).single()
        compose.onRoot().captureRoboImage("src/test/screenshots/motion-details-phone-light.png")
        finish()
        // The details' own top and bottom bars appear in place; only the body slides in.
        assertStill(detailsBack, bounds(detailsBack), backMiddle)
        assertStill(hasText("Open score"), bounds(hasText("Open score")), openMiddle)
        val bodyAtRest = bounds(hasTestTag(DETAILS_BODY_TAG)).single()
        assertTrue("the body comes in from the end: $bodyAtRest <- $bodyMiddle", bodyMiddle.left > bodyAtRest.left + 100f)
    }

    private companion object {
        /** About 100 ms into the 300 ms change, before the emphasized curve has settled the content. */
        const val HALFWAY_STEPS = 3
        const val STEP_MILLIS = 40L
    }
}
