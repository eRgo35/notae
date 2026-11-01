package com.c2yz.notae.nav

import android.view.View
import androidx.activity.ComponentActivity
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.isSelected
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performScrollToIndex
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.swipeUp
import androidx.compose.ui.test.withKeyDown
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.MetadataDraft
import com.c2yz.notae.core.PositionOf
import com.c2yz.notae.core.Row
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.Selection
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.ui.details.DETAILS_BODY_TAG
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.library.GRID_TAG
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Library (or a set) and Details as one list-detail scene on a wide window (R37: at least 840 dp), and Details alone
 * below that. The core stands in as the snapshot: each test changes it as the core would.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class ListDetailSceneTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val actions = mutableListOf<Action>()
    private var backs = 0
    private var snapshot by mutableStateOf(Fixture.library())

    private fun show(first: Snapshot, onBack: () -> Unit = { backs++ }) {
        snapshot = first
        compose.setContent { NotaeTheme { NotaeNavDisplay(snapshot, onAction = { actions += it }, onBack = onBack) } }
        compose.waitForIdle()
    }

    /** The score at [index] of All scores, with its neighbours and place as the core sends them. */
    private fun selecting(index: Int, from: Snapshot = Fixture.library()): Snapshot {
        val rows = from.rows
        return from.copy(
            selection = Selection(
                row = rows[index],
                previous = rows.getOrNull(index - 1)?.selectionId,
                next = rows.getOrNull(index + 1)?.selectionId,
                position = PositionOf((index + 1).toUInt(), rows.size.toUInt()),
            ),
        )
    }

    private val rows: List<Row> = Fixture.rows
    private val libraryTitle = hasText("All scores") and isHeading()
    private val ave = rows.indexOfFirst { it.title == "Ave Maria" }

    @Test
    fun wide_window_shows_library_and_details_side_by_side() {
        show(selecting(ave))

        // The library keeps its header and scores; the details sit to their right.
        compose.onNode(libraryTitle).assertIsDisplayed()
        compose.onNodeWithTag(GRID_TAG).assertIsDisplayed()
        compose.onNodeWithText("Open score").assertIsDisplayed()
        val grid = compose.onNodeWithTag(GRID_TAG).fetchSemanticsNode().boundsInRoot
        val back = compose.onNodeWithContentDescription("Back to library").fetchSemanticsNode().boundsInRoot
        assertTrue("details right of the library: $grid, $back", back.left >= grid.right)
        // The shown score looks selected in the library, as Slint's list did.
        compose.onNodeWithContentDescription("Ave Maria").assertIsSelected()
        compose.onNodeWithContentDescription(rows[ave + 1].title).assert(!isSelected())
        // The banner shows once, in the library pane.
        snapshot = snapshot.copy(banner = Fixture.running)
        compose.waitForIdle()
        compose.onAllNodesWithText("Scanning library…").assertCountEquals(1)

        // The details hold the keys with nothing clicked: Right, Left, Ctrl K and Escape.
        compose.onRoot().performKeyInput { pressKey(Key.DirectionRight) }
        compose.onRoot().performKeyInput { pressKey(Key.DirectionLeft) }
        compose.onRoot().performKeyInput { withKeyDown(Key.CtrlLeft) { pressKey(Key.K) } }
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        assertEquals(
            listOf(
                Action.SelectScore(rows[ave + 1].selectionId),
                Action.SelectScore(rows[ave - 1].selectionId),
                Action.OpenSearch,
                Action.ClearSelection,
            ),
            actions,
        )
    }

    @Test
    fun a_set_pairs_with_its_details_too() {
        val palm = Fixture.palmSunday()
        show(selecting(1, palm))

        compose.onNodeWithText("Niedziela Palmowa").assertIsDisplayed()
        compose.onNodeWithText("2 of 9").assertIsDisplayed()
        compose.onNodeWithContentDescription("2. Kyrie eleison").assertIsSelected()
        // Escape closes the details first, not the set (the core's back order).
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        assertEquals(listOf<Action>(Action.ClearSelection), actions)
    }

    @Test
    @Config(qualifiers = "w800dp-h1280dp-xhdpi")
    fun portrait_details_take_the_keys_when_they_open() {
        // The outgoing library stays composed during the transition; it must not take focus from the details.
        show(Fixture.library())
        snapshot = selecting(ave)
        compose.waitForIdle()

        compose.onRoot().performKeyInput { pressKey(Key.DirectionRight) }

        assertEquals(listOf<Action>(Action.SelectScore(rows[ave + 1].selectionId)), actions)
    }

    @Test
    @Config(qualifiers = "w800dp-h1280dp-xhdpi")
    fun narrow_window_shows_details_alone() {
        show(selecting(ave))

        compose.onNodeWithText("Open score").assertIsDisplayed()
        compose.onNode(libraryTitle).assertDoesNotExist()
        compose.onNodeWithTag(GRID_TAG).assertDoesNotExist()
        // Full width: the details card starts at the window's own margin.
        val back = compose.onNodeWithContentDescription("Back to library").fetchSemanticsNode().boundsInRoot
        assertTrue("details start at the left edge: $back", back.left < 100f)
    }

    @Test
    @Config(qualifiers = "w800dp-h1280dp-xhdpi")
    fun a_new_snapshot_reaches_the_shown_screen() {
        // Navigation 3 remembers the entries until the back stack changes; a star toggled on the same details must
        // still show.
        show(selecting(ave))
        compose.onNodeWithContentDescription("Remove from favorites").assertIsDisplayed()

        snapshot = snapshot.copy(selection = snapshot.selection!!.copy(row = rows[ave].copy(favorite = false)))
        compose.waitForIdle()

        compose.onNodeWithContentDescription("Add to favorites").assertIsDisplayed()
    }

    @Test
    fun choosing_another_score_updates_the_detail_pane_in_place() {
        show(selecting(ave))
        compose.onNodeWithTag(DETAILS_BODY_TAG).performTouchInput { swipeUp() }
        compose.waitForIdle()
        val scrolled = detailsScroll()
        assertTrue("the details body scrolled", scrolled > 0f)

        // The core answers SelectScore (from the ⋮ sheet, a key or Next) with the next selection.
        snapshot = selecting(ave + 1)
        compose.waitForIdle()

        compose.onNodeWithText("${ave + 2} of ${rows.size}").assertIsDisplayed()
        compose.onNodeWithContentDescription(rows[ave + 1].title).assertIsSelected()
        compose.onNodeWithContentDescription("Ave Maria").assert(!isSelected())
        // The same pane, updated: a new entry would start at the top.
        val range = compose.onNodeWithTag(DETAILS_BODY_TAG).fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
        assertEquals(minOf(scrolled, range.maxValue()), range.value(), 0.5f)
        compose.onNode(libraryTitle).assertIsDisplayed()
    }

    @Test
    fun back_in_two_panes_clears_selection_and_keeps_library() {
        // The core's back() with details open clears the selection.
        show(selecting(ave)) {
            backs++
            snapshot = snapshot.copy(selection = null)
        }
        compose.onNodeWithTag(GRID_TAG).performScrollToIndex(12)
        compose.waitForIdle()
        compose.onNode(libraryTitle).assertIsDisplayed()
        val scrolled = gridScroll()
        assertTrue("the grid scrolled", scrolled > 0f)

        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()

        assertEquals(1, backs)
        assertEquals(emptyList<Action>(), actions)
        compose.onNodeWithText("Open score").assertDoesNotExist()
        compose.onNode(libraryTitle).assertIsDisplayed()
        assertEquals(scrolled, gridScroll(), 0.5f)
        // The library holds the keys again: Ctrl K opens search with nothing clicked.
        compose.onRoot().performKeyInput { withKeyDown(Key.CtrlLeft) { pressKey(Key.K) } }
        assertEquals(listOf<Action>(Action.OpenSearch), actions)
    }

    @Test
    fun keys_step_again_after_an_overlay_closes_beside_the_library() {
        val next = Action.SelectScore(rows[ave + 1].selectionId)
        show(selecting(ave))

        // The metadata editor opens over both panes and closes again.
        val editing = snapshot
        snapshot = editing.copy(metadataEditor = MetadataDraft(rows[ave].noteKey, "", "Ave Maria", "Schubert", emptyList(), null))
        compose.waitForIdle()
        snapshot = editing
        compose.waitForIdle()
        compose.onRoot().performKeyInput { pressKey(Key.DirectionRight) }
        assertEquals(listOf<Action>(next), actions)

        // The library's own ⋮ sheet opens in its pane, and Escape closes it.
        compose.onNodeWithContentDescription("More actions for Ave Maria").performClick()
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        compose.waitForIdle()
        compose.onRoot().performKeyInput { pressKey(Key.DirectionRight) }
        assertEquals(listOf<Action>(next, next), actions)
    }

    @Test
    fun ctrl_k_from_the_details_shows_search_on_top_and_back_returns_to_them() {
        show(selecting(ave))
        compose.onRoot().performKeyInput { withKeyDown(Key.CtrlLeft) { pressKey(Key.K) } }
        assertEquals(listOf<Action>(Action.OpenSearch), actions)

        // The core keeps the selection under Search (ruling R45): Search is shown on top and takes the keys.
        snapshot = snapshot.copy(screen = Screen.Search)
        compose.waitForIdle()
        compose.onNodeWithText("Search your library").assertIsDisplayed()
        compose.onNodeWithText("Open score").assertDoesNotExist()
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        assertEquals(listOf(Action.OpenSearch, Action.CloseSearch), actions)

        // Back goes to the core, which closes Search and keeps the selection: the details show again with the keys.
        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        assertEquals(1, backs)
        snapshot = snapshot.copy(screen = Screen.Library)
        compose.waitForIdle()
        compose.onNodeWithText("Open score").assertIsDisplayed()
        compose.onRoot().performKeyInput { pressKey(Key.DirectionRight) }
        assertEquals(Action.SelectScore(rows[ave + 1].selectionId), actions.last())
    }

    @Test
    fun escape_closes_the_sheet_beside_the_details_not_the_details() {
        show(selecting(ave))
        enterTouchMode()

        compose.onNodeWithContentDescription("More actions for Ave Maria").performClick()
        compose.onNodeWithText("Edit details").assertIsDisplayed()
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        compose.waitForIdle()

        compose.onNodeWithText("Edit details").assertDoesNotExist()
        assertEquals(emptyList<Action>(), actions)
    }

    /**
     * A tablet is in touch mode, where the sheet's actions cannot take the focus. Robolectric never enters it from a
     * click, so this sets it on the window's ViewRootImpl as a touch on a device does.
     */
    private fun enterTouchMode() {
        compose.runOnIdle {
            val root = View::class.java.getMethod("getViewRootImpl").invoke(compose.activity.window.decorView)
            val ensure = root.javaClass.getDeclaredMethod("ensureTouchModeLocally", Boolean::class.javaPrimitiveType)
            ensure.isAccessible = true
            ensure.invoke(root, true)
            assertTrue(compose.activity.window.decorView.isInTouchMode)
        }
    }

    private fun detailsScroll(): Float =
        compose.onNodeWithTag(DETAILS_BODY_TAG).fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange].value()

    private fun gridScroll(): Float =
        compose.onNodeWithTag(GRID_TAG).fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange].value()
}
