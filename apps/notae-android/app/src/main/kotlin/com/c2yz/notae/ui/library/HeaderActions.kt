package com.c2yz.notae.ui.library

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.Placeable
import androidx.compose.ui.layout.SubcomposeLayout
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.theme.NotaeTheme

/** The least room (dp) the library header's title keeps; actions that would squeeze it further move into ⋮. */
internal const val HEADER_TITLE_MIN = 160

/** The library header's ⋮, shown when some of its actions do not fit. */
internal const val HEADER_OVERFLOW_TAG = "headerOverflow"

/** One header action as [fitActions] sees it: its [width], and whether it may move into ⋮ ([pinned] never does). */
internal data class HeaderAction(val key: String, val width: Dp, val priority: Int, val pinned: Boolean)

/**
 * Which actions fit in [available] width, [gap] apart: returns the keys shown, in their order, and the keys moved into
 * ⋮, in the order they moved. The lowest [HeaderAction.priority] moves first (the later one of equal priority first);
 * pinned actions never move; once any moves, room for the ⋮ ([overflowWidth]) is kept.
 */
internal fun fitActions(
    actions: List<HeaderAction>,
    available: Dp,
    overflowWidth: Dp = 48.dp,
    gap: Dp = 4.dp,
): Pair<List<String>, List<String>> {
    fun width(shown: List<HeaderAction>): Dp =
        shown.fold(0.dp) { sum, it -> sum + it.width } + gap * (shown.size - 1).coerceAtLeast(0)
    if (width(actions) <= available) return actions.map { it.key } to emptyList()
    val order = actions.withIndex().filter { !it.value.pinned }.sortedWith(compareBy({ it.value.priority }, { -it.index }))
    val shown = actions.toMutableList()
    val over = mutableListOf<String>()
    for ((_, action) in order) {
        if (width(shown) + gap + overflowWidth <= available) break
        shown.remove(action)
        over += action.key
    }
    return shown.map { it.key } to over
}

/**
 * A header button that [HeaderRow] may move into ⋮: its [button] in the row, or its [menu] items in ⋮ (each closing
 * the menu with the action it chose).
 */
internal class HeaderItem(
    val key: String,
    val priority: Int,
    val pinned: Boolean,
    val button: @Composable () -> Unit,
    val menu: @Composable ColumnScope.(choose: (Action) -> Unit) -> Unit = {},
)

/**
 * The library header's row: the [leading] buttons, the [title] (at least [HEADER_TITLE_MIN] dp, then whatever is
 * left), the [trailing] buttons that fit and a ⋮ with the rest ([fitActions]). Buttons are measured at their own width;
 * the ones in ⋮ are measured but not placed.
 */
@Composable
internal fun HeaderRow(
    leading: List<HeaderItem>,
    trailing: List<HeaderItem>,
    onAction: (Action) -> Unit,
    modifier: Modifier = Modifier,
    title: @Composable () -> Unit,
) {
    val items = leading + trailing
    SubcomposeLayout(modifier) { constraints ->
        val gap = 4.dp.roundToPx()
        val loose = Constraints(maxHeight = constraints.maxHeight)
        val measured = items.associate { item -> item.key to subcompose(item.key, item.button).map { it.measure(loose) } }
        val actions = items.map { item ->
            HeaderAction(item.key, (measured.getValue(item.key).maxOfOrNull { it.width } ?: 0).toDp(), item.priority, item.pinned)
        }
        // The title has a gap on each side.
        val (shown, over) = fitActions(actions, constraints.maxWidth.toDp() - HEADER_TITLE_MIN.dp - 8.dp)
        val overflowItems = items.filter { it.key in over }
        val overflow = if (overflowItems.isEmpty()) {
            emptyList()
        } else {
            subcompose("overflow") { HeaderOverflow(overflowItems, onAction) }.map { it.measure(loose) }
        }
        val before = leading.filter { it.key in shown }.flatMap { measured.getValue(it.key) }
        val after = trailing.filter { it.key in shown }.flatMap { measured.getValue(it.key) } + overflow
        val sides = (before + after).sumOf { it.width + gap }
        val titleWidth = (constraints.maxWidth - sides).coerceAtLeast(0)
        val titled = subcompose("title", title).map {
            it.measure(Constraints(minWidth = titleWidth, maxWidth = titleWidth, maxHeight = constraints.maxHeight))
        }
        val height = (before + after + titled).maxOfOrNull { it.height } ?: 0
        layout(constraints.maxWidth, height) {
            var x = 0
            fun place(placeable: Placeable, width: Int = placeable.width) {
                placeable.placeRelative(x, (height - placeable.height) / 2)
                x += width + gap
            }
            before.forEach { place(it) }
            titled.forEach { place(it, titleWidth) }
            after.forEach { place(it) }
        }
    }
}

/** ⋮ with the header actions that did not fit, each with its own label and icon. */
@Composable
private fun HeaderOverflow(items: List<HeaderItem>, onAction: (Action) -> Unit) {
    val c = NotaeTheme.colors
    var open by remember { mutableStateOf(false) }
    val choose = { action: Action ->
        open = false
        onAction(action)
    }
    Box {
        IconButton(stringResource(R.string.more_options), painterResource(R.drawable.ic_more_vertical), { open = true }, Modifier.testTag(HEADER_OVERFLOW_TAG))
        DropdownMenu(expanded = open, onDismissRequest = { open = false }, shape = RoundedCornerShape(20.dp), containerColor = c.card) {
            items.forEach { it.menu(this, choose) }
        }
    }
}
