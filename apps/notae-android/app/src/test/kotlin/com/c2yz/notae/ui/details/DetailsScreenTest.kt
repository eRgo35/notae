package com.c2yz.notae.ui.details

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.withKeyDown
import androidx.compose.ui.test.swipeUp
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.semantics.SemanticsProperties
import com.c2yz.notae.nav.NotaeNavDisplay
import androidx.compose.ui.unit.Density
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Origin
import com.c2yz.notae.core.PositionOf
import com.c2yz.notae.core.Row
import com.c2yz.notae.core.SectionTitle
import com.c2yz.notae.core.Selection
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.core.Subtitle
import com.c2yz.notae.ui.assertAllClickablesLabelled
import com.c2yz.notae.ui.assertClickTargetsAtLeast48dp
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.theme.NotaeTheme
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** The details screen: neighbours and "X of Y" as the core sends them, its actions, labels and 48 dp targets. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class DetailsScreenTest {
    @get:Rule
    val compose = createComposeRule()

    private val actions = mutableListOf<Action>()
    private var shown by mutableStateOf<Snapshot?>(null)

    private fun show(snapshot: Snapshot) {
        shown = snapshot
        var density = Density(1f)
        compose.setContent {
            density = LocalDensity.current
            NotaeTheme { shown?.let { DetailsScreen(it, { action -> actions += action }) } }
        }
        compose.assertAllClickablesLabelled()
        compose.assertClickTargetsAtLeast48dp(density)
    }

    /** The set A, B, A: each copy is its own row, selected by its item id. */
    private val setRows: List<Row> = listOf(
        piece(Fixture.rows.first { it.title == "Ave Maria" }, "item-a1", 1u),
        piece(Fixture.rows.first { it.title == "Kyrie eleison" }, "item-b", 2u),
        piece(Fixture.rows.first { it.title == "Ave Maria" }, "item-a2", 3u),
    )

    private fun piece(row: Row, itemId: String, position: UInt) = row.copy(selectionId = itemId, itemId = itemId, position = position)

    private fun inSet(index: Int): Snapshot = Fixture.library().copy(
        sectionKey = "set:palm-sunday",
        title = SectionTitle.Set("Niedziela Palmowa"),
        subtitle = Subtitle.Set(date = "2026-03-29", pieces = 3u, notes = 0u),
        rows = setRows,
        selection = Selection(
            row = setRows[index],
            previous = setRows.getOrNull(index - 1)?.selectionId,
            next = setRows.getOrNull(index + 1)?.selectionId,
            position = PositionOf((index + 1).toUInt(), setRows.size.toUInt()),
        ),
    )

    @Test
    fun second_copy_keeps_position() {
        show(inSet(2))

        compose.onNodeWithText("3 of 3").assertIsDisplayed()
        compose.onNodeWithContentDescription("Previous score").performClick()
        compose.onNodeWithText("Open score").performClick()

        val aveMaria = setRows[2]
        assertEquals(
            listOf(
                Action.SelectScore("item-b"),
                Action.OpenPdf(aveMaria.relativePath, aveMaria.noteKey, "item-a2"),
            ),
            actions,
        )

        // The first copy of the same score is its own step, at its own place.
        shown = inSet(0)
        compose.waitForIdle()
        compose.onNodeWithText("1 of 3").assertIsDisplayed()
        compose.onNodeWithContentDescription("Next score").performClick()
        assertEquals(Action.SelectScore("item-b"), actions.last())
    }

    @Test
    fun next_disabled_on_last() {
        show(inSet(2))

        compose.onNodeWithContentDescription("Next score").assertIsNotEnabled()
        compose.onNodeWithContentDescription("Previous score").assertIsEnabled()
        compose.onNodeWithContentDescription("Next score").performClick()
        assertEquals(emptyList<Action>(), actions)

        shown = inSet(0)
        compose.waitForIdle()
        compose.onNodeWithContentDescription("Previous score").assertIsNotEnabled()
        compose.onNodeWithContentDescription("Next score").assertIsEnabled()
    }

    @Test
    fun a_search_result_has_no_neighbours_and_no_position() {
        val row = Fixture.rows.first { it.title == "Agnus Dei" }
        show(Fixture.library().copy(selection = Selection(row, previous = null, next = null, position = null)))

        compose.onNodeWithContentDescription("Previous score").assertIsNotEnabled()
        compose.onNodeWithContentDescription("Next score").assertIsNotEnabled()
        compose.onNodeWithText(" of ", substring = true).assertDoesNotExist()
    }

    @Test
    fun header_and_headline_dispatch_their_actions() {
        val snapshot = aveMariaDetails()
        val row = snapshot.selection!!.row
        show(snapshot)

        compose.onNodeWithContentDescription("Remove from favorites").performClick()
        compose.onNodeWithContentDescription("Edit details").performClick()
        compose.onNodeWithContentDescription("Back to library").performClick()

        assertEquals(
            listOf(Action.ToggleFavorite(row.noteKey), Action.EditMetadata(row.noteKey), Action.ClearSelection),
            actions,
        )

        shown = snapshot.copy(selection = snapshot.selection!!.copy(row = row.copy(favorite = false)))
        compose.waitForIdle()
        compose.onNodeWithContentDescription("Add to favorites").assertIsDisplayed()
    }

    @Test
    fun info_card_shows_origins_status_and_dashes() {
        show(aveMariaDetails())

        compose.onNodeWithText("No. 12").assertIsDisplayed()
        compose.onNodeWithText("Ave Maria.pdf").assertIsDisplayed()
        compose.onNodeWithText("Ready").assertIsDisplayed()
        compose.onAllNodesWithText("Edited").assertCountEquals(2)
        compose.onNodeWithText("—").assertIsDisplayed()
        compose.onNodeWithContentDescription("Score 3 of 20").assertExists()
    }

    @Test
    fun arrow_keys_step_without_a_click() {
        show(inSet(1))

        compose.onRoot().performKeyInput { pressKey(Key.DirectionRight) }
        compose.onRoot().performKeyInput { pressKey(Key.DirectionLeft) }

        assertEquals(listOf<Action>(Action.SelectScore("item-a2"), Action.SelectScore("item-a1")), actions)
    }

    @Test
    fun left_on_the_first_does_nothing() {
        show(inSet(0))

        compose.onRoot().performKeyInput { pressKey(Key.DirectionLeft) }

        assertEquals(emptyList<Action>(), actions)
        // The screen does hold the keys: Right still steps.
        compose.onRoot().performKeyInput { pressKey(Key.DirectionRight) }
        assertEquals(listOf<Action>(Action.SelectScore("item-b")), actions)
    }

    @Test
    fun escape_goes_back_without_a_click() {
        show(inSet(1))

        compose.onRoot().performKeyInput { pressKey(Key.Escape) }

        assertEquals(listOf<Action>(Action.ClearSelection), actions)
    }

    @Test
    fun ctrl_k_on_details_opens_search() {
        show(inSet(1))

        // Nothing clicked first, as in 0.3.0's root shortcut scope.
        compose.onRoot().performKeyInput { withKeyDown(Key.CtrlLeft) { pressKey(Key.K) } }

        assertEquals(listOf<Action>(Action.OpenSearch), actions)
    }

    @Test
    @Config(qualifiers = "w700dp-h500dp-xhdpi")
    fun a_step_keeps_the_screen_and_its_scroll() {
        // A short, narrow window stacks the details, so the body scrolls.
        var snapshot by mutableStateOf(inSet(0))
        compose.setContent { NotaeTheme { NotaeNavDisplay(snapshot, onAction = {}, onBack = {}) } }
        compose.onNodeWithTag(DETAILS_BODY_TAG).performTouchInput { swipeUp() }
        compose.waitForIdle()
        val scrolled = scrollOffset()
        assertTrue("the body scrolled", scrolled > 0f)

        snapshot = inSet(1)
        compose.waitForIdle()

        compose.onNodeWithText("2 of 3").assertIsDisplayed()
        // Kyrie has no composer line, so its body is shorter and the kept offset is clamped to the new end; a new
        // entry would start at 0.
        val range = compose.onNodeWithTag(DETAILS_BODY_TAG).fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
        assertEquals(minOf(scrolled, range.maxValue()), range.value(), 0.5f)
    }

    private fun scrollOffset(): Float =
        compose.onNodeWithTag(DETAILS_BODY_TAG).fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange].value()

    private fun aveMariaDetails(): Snapshot {
        val library = Fixture.library()
        val row = library.rows.first { it.title == "Ave Maria" }.copy(
            number = "12",
            titleOrigin = Origin.MANUAL,
            composerOrigin = Origin.MANUAL,
        )
        return library.copy(selection = Selection(row, previous = "key-1", next = "key-3", position = PositionOf(3u, 20u)))
    }

    // Screenshots (app/src/test/screenshots), named after their docs/compose-parity references. Landscape goes
    // through the shell, so it shows the details beside the library or the set (R37); portrait shows them alone.

    private fun shot(state: String, snapshot: Snapshot, landscape: Boolean, dark: Boolean) {
        RuntimeEnvironment.setQualifiers(if (landscape) "+w1280dp-h800dp-xhdpi" else "+w800dp-h1280dp-xhdpi")
        shown = snapshot
        compose.setContent {
            NotaeTheme(dark = dark) {
                shown?.let {
                    if (landscape) NotaeNavDisplay(it, onAction = {}, onBack = {}) else DetailsScreen(it, { action -> actions += action })
                }
            }
        }
        compose.waitForIdle()
        val orientation = if (landscape) "landscape" else "portrait"
        compose.onRoot().captureRoboImage("src/test/screenshots/$state-$orientation-${if (dark) "dark" else "light"}.png")
    }

    /** Ave Maria in All scores, as in details-library: edited title and composer, a favorite, no number. */
    private val library: Snapshot
        get() = aveMariaDetails().let { it.copy(selection = it.selection!!.copy(row = it.selection!!.row.copy(number = null))) }

    /** Kyrie eleison, the second piece of "Niedziela Palmowa", as in details-in-set. */
    private val inSet: Snapshot
        get() {
            // A score without a composer has no composer origin, as the core reports it.
            val kyrie = Fixture.palmRows[1].copy(composerOrigin = Origin.UNSET)
            return Fixture.palmSunday().copy(
                selection = Selection(kyrie, previous = "item-1", next = "item-3", position = PositionOf(2u, 9u)),
            )
        }

    @Test fun shot_library_landscape_light() = shot("details-library", library, landscape = true, dark = false)

    @Test fun shot_library_landscape_dark() = shot("details-library", library, landscape = true, dark = true)

    @Test fun shot_library_portrait_light() = shot("details-library", library, landscape = false, dark = false)

    @Test fun shot_library_portrait_dark() = shot("details-library", library, landscape = false, dark = true)

    @Test fun shot_in_set_landscape_light() = shot("details-in-set", inSet, landscape = true, dark = false)

    @Test fun shot_in_set_landscape_dark() = shot("details-in-set", inSet, landscape = true, dark = true)

    @Test fun shot_in_set_portrait_light() = shot("details-in-set", inSet, landscape = false, dark = false)

    @Test fun shot_in_set_portrait_dark() = shot("details-in-set", inSet, landscape = false, dark = true)
}
