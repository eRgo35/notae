package com.c2yz.notae.ui.sets

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.InputMode
import androidx.compose.ui.input.InputModeManager
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalInputModeManager
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.isFocused
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performScrollToIndex
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.withKeyDown
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.NotaeCore
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.core.StatusMessage
import com.c2yz.notae.ui.assertAllClickablesLabelled
import com.c2yz.notae.ui.assertClickTargetsAtLeast48dp
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.library.GRID_TAG
import com.c2yz.notae.ui.library.LibraryScreen
import com.c2yz.notae.ui.library.REORDER_TAG
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.nio.file.Files

/** Reorder mode inside a set: the list with handles, up, down and remove, and Done (favorites spec §5.4). */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
@OptIn(ExperimentalComposeUiApi::class)
class ReorderTest {
    @get:Rule
    val compose = createComposeRule()

    private val actions = mutableListOf<Action>()
    private var shown by mutableStateOf<Snapshot?>(null)
    private var density = Density(1f)
    private var inputMode: InputModeManager? = null

    private fun show(snapshot: Snapshot, onAction: (Action) -> Unit = { actions += it }) {
        shown = snapshot
        compose.setContent {
            density = LocalDensity.current
            inputMode = LocalInputModeManager.current
            NotaeTheme { shown?.let { LibraryScreen(it, onAction) } }
        }
        compose.assertAllClickablesLabelled()
        compose.assertClickTargetsAtLeast48dp(density)
    }

    private val reordering = Fixture.palmSunday().copy(reordering = true)

    @Test
    fun up_disabled_on_first_row() {
        show(reordering)

        // Ave Maria is item 1 and item 4.
        compose.onAllNodesWithContentDescription("Move Ave Maria up")[0].assertIsNotEnabled()
        compose.onAllNodesWithContentDescription("Move Ave Maria down")[0].assertIsEnabled()
        compose.onAllNodesWithContentDescription("Move Ave Maria up")[1].assertIsEnabled()
        compose.onNodeWithContentDescription("Drag to reorder Kyrie eleison").assertExists()
        compose.onNodeWithContentDescription("Remove Kyrie eleison from set").performClick()

        // Alleluja is item 5 and the last, item 9.
        compose.onNodeWithTag(REORDER_TAG).performScrollToIndex(8)
        compose.onAllNodesWithContentDescription("Move Alleluja down")[1].assertIsNotEnabled()
        compose.onAllNodesWithContentDescription("Move Alleluja up")[1].assertIsEnabled()
        assertEquals(listOf<Action>(Action.RemoveSetItem("item-2")), actions)
    }

    @Test
    fun move_down_then_up_restores_order() {
        val dir = Files.createTempDirectory("notae-reorder").toFile()
        val core = NotaeCore(dir.path, Scores(listOf("Ave Maria", "Barka", "Credo")), NoListener)
        try {
            core.libraryRootPicked("Scores", "tree")
            core.await { it.screen == Screen.Library }
            core.dispatch(Action.Rescan)
            val library = core.await { it.banner.message is StatusMessage.Finished && it.rows.size == 3 }
            core.dispatch(Action.NewSet)
            core.dispatch(Action.SaveSetDialog("Mass", null, null))
            core.dispatch(Action.OpenAddPicker)
            library.rows.forEach { core.dispatch(Action.ToggleAddPickerPiece(it.noteKey)) }
            core.dispatch(Action.CloseAddPicker)
            core.dispatch(Action.ReorderStarted)
            val titles = { core.snapshot().rows.map { it.title } }
            assertEquals(listOf("Ave Maria", "Barka", "Credo"), titles())
            show(core.snapshot()) { action ->
                actions += action
                core.dispatch(action)
                shown = core.snapshot()
            }

            compose.onNodeWithContentDescription("Move Ave Maria down").performClick()
            assertEquals(listOf("Barka", "Ave Maria", "Credo"), titles())
            compose.onNodeWithContentDescription("Move Ave Maria up").performClick()

            assertEquals(listOf("Ave Maria", "Barka", "Credo"), titles())
            val ave = core.snapshot().rows.first().itemId!!
            assertEquals(listOf<Action>(Action.MoveSetItem(ave, 1), Action.MoveSetItem(ave, -1)), actions)
            compose.onNodeWithContentDescription("Move Ave Maria up").assertIsNotEnabled()
        } finally {
            core.close()
            dir.deleteRecursively()
        }
    }

    @Test
    fun done_restores_grid_layout() {
        show(Fixture.palmSunday())
        compose.onNodeWithTag(GRID_TAG).assertExists()
        compose.onNodeWithText("Reorder").performClick()
        assertEquals(listOf<Action>(Action.ReorderStarted), actions)

        // The grid choice stays saved; reorder mode shows the list over it.
        shown = reordering
        compose.waitForIdle()
        compose.onNodeWithTag(REORDER_TAG).assertExists()
        compose.onNodeWithTag(GRID_TAG).assertDoesNotExist()
        compose.onNodeWithText("Drag a row, or use the arrows, to change the order").assertExists()
        compose.onNodeWithText("Reorder").assertDoesNotExist()
        compose.onNodeWithText("Done").performClick()
        assertEquals(listOf(Action.ReorderStarted, Action.ReorderFinished), actions)

        shown = Fixture.palmSunday()
        compose.waitForIdle()
        compose.onNodeWithTag(GRID_TAG).assertExists()
        compose.onNodeWithTag(REORDER_TAG).assertDoesNotExist()
    }

    @Test
    fun drag_to_index_dispatches_move_to() {
        show(reordering)
        val row = with(density) { 80.dp.toPx() }
        val handle = compose.onAllNodesWithContentDescription("Drag to reorder Ave Maria")[0]

        handle.performTouchInput {
            down(center)
            moveBy(Offset(0f, row))
            // Two rows and a quarter past the touch slop: the drop rounds to two rows whatever the slop.
            moveBy(Offset(0f, row * 1.5f))
        }
        // Nothing is sent while the row moves.
        assertEquals(emptyList<Action>(), actions)
        handle.performTouchInput { up() }

        assertEquals(listOf<Action>(Action.MoveSetItemTo("item-1", 2u)), actions)
    }

    @Test
    fun reorder_keys_work_without_a_click() {
        show(reordering)

        compose.onRoot().performKeyInput { withKeyDown(Key.CtrlLeft) { pressKey(Key.K) } }
        // Escape leaves reorder mode, not the set.
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        assertEquals(listOf(Action.OpenSearch, Action.ReorderFinished), actions)

        // Tab walks the screen to Kyrie's down arrow, and Enter presses it.
        tabTo("Move Kyrie eleison down")
        compose.onRoot().performKeyInput { pressKey(Key.Enter) }
        assertEquals(Action.MoveSetItem("item-2", 1), actions.last())
    }

    @Test
    fun keyboard_move_to_the_end_keeps_focus_in_the_row() {
        showLive()

        tabTo("Move Kyrie eleison down")
        compose.onRoot().performKeyInput { pressKey(Key.Enter) }
        assertEquals(listOf("Ave Maria", "Magnificat", "Kyrie eleison"), shown?.rows?.map { it.title })
        // Down is now disabled; its row's up arrow takes the focus.
        assertFocused("Move Kyrie eleison up")
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        assertEquals(Action.ReorderFinished, actions.last())
    }

    @Test
    fun keyboard_remove_moves_focus_to_a_neighbour() {
        showLive()

        tabTo("Remove Kyrie eleison from set")
        compose.onRoot().performKeyInput { pressKey(Key.Enter) }
        assertFocused("Remove Magnificat from set")
        // The last row's removal falls back to the row above.
        compose.onRoot().performKeyInput { pressKey(Key.Enter) }
        assertFocused("Remove Ave Maria from set")
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        assertEquals(Action.ReorderFinished, actions.last())
        // An empty set leaves the keys on the screen.
        compose.onRoot().performKeyInput { pressKey(Key.Enter) }
        assertEquals(emptyList<String>(), shown?.rows?.map { it.title })
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        assertEquals(listOf(Action.ReorderFinished, Action.ReorderFinished), actions.filter { it == Action.ReorderFinished })
    }

    /** Three pieces of Niedziela Palmowa, with moves and removals applied as the core would. */
    private fun showLive() {
        var rows = Fixture.palmRows.take(3)
        fun snapshot() = reordering.copy(rows = rows.mapIndexed { index, row -> row.copy(position = (index + 1).toUInt()) })
        show(snapshot()) { action ->
            actions += action
            when (action) {
                is Action.MoveSetItem -> {
                    val from = rows.indexOfFirst { it.itemId == action.itemId }
                    val moved = rows.toMutableList()
                    moved.add((from + action.delta).coerceIn(0, rows.lastIndex), moved.removeAt(from))
                    rows = moved
                }
                is Action.RemoveSetItem -> rows = rows.filter { it.itemId != action.itemId }
                else -> Unit
            }
            shown = snapshot()
        }
    }

    /** Presses Tab until the control named [label] has the focus, with the keyboard input mode a hardware keyboard sets. */
    private fun tabTo(label: String) {
        // A hardware keyboard takes Android out of touch mode; Robolectric's key input does not, so ask for it here.
        compose.runOnIdle { inputMode?.requestInputMode(InputMode.Keyboard) }
        var tabs = 0
        while (compose.onAllNodes(hasContentDescription(label) and isFocused()).fetchSemanticsNodes().isEmpty()) {
            check(tabs++ < 60) { "Tab never reached $label" }
            compose.onRoot().performKeyInput { pressKey(Key.Tab) }
        }
    }

    private fun assertFocused(label: String) {
        compose.waitForIdle()
        val focused = compose.onAllNodes(isFocused()).fetchSemanticsNodes().map { it.config.getOrNull(SemanticsProperties.ContentDescription) }
        assertEquals(listOf(listOf(label)), focused)
    }
}
