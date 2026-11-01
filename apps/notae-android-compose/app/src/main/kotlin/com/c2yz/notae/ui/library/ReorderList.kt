package com.c2yz.notae.ui.library

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.zIndex
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Row as ScoreRowData
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.components.NotaeCard
import com.c2yz.notae.ui.theme.NotaeTheme
import kotlin.math.roundToInt

private val ROW_HEIGHT = 80.dp

/**
 * A set in reorder mode (favorites spec §5.4): the list layout whatever the grid/list choice, one 80 dp row per piece
 * with a drag handle, up, down and remove. Up and down are the dependable path for TalkBack and keyboards; the handle
 * drags a row by whole rows and sends one [Action.MoveSetItemTo] when it is dropped.
 */
@Composable
internal fun ReorderList(rows: List<ScoreRowData>, onAction: (Action) -> Unit, modifier: Modifier = Modifier) {
    val act by rememberUpdatedState(onAction)
    val current by rememberUpdatedState(rows)
    // The row under the finger and how far it has moved; only the drop reaches the core.
    var dragging by remember { mutableStateOf<String?>(null) }
    var offset by remember { mutableFloatStateOf(0f) }
    // After a keyboard removal, the neighbour whose remove button takes the focus the removed row held.
    var focusRemoveOn by remember { mutableStateOf<String?>(null) }
    NotaeCard(modifier.fillMaxSize()) {
        val state = rememberLazyListState()
        LazyColumn(Modifier.testTag(REORDER_TAG).fillMaxSize().padding(8.dp), state = state) {
            itemsIndexed(rows, key = { _, row -> row.selectionId }) { index, row ->
                val dragged = dragging == row.selectionId
                // Keyed on the item only, so a snapshot arriving mid-drag (a banner, say) does not cancel the gesture.
                val handle: Modifier.() -> Modifier = { pointerInput(row.selectionId) {
                    detectVerticalDragGestures(
                        onDragStart = {
                            dragging = row.selectionId
                            offset = 0f
                        },
                        onDragEnd = {
                            val from = current.indexOfFirst { it.selectionId == row.selectionId }
                            val to = (from + (offset / ROW_HEIGHT.toPx()).roundToInt()).coerceIn(0, current.lastIndex)
                            val itemId = row.itemId
                            val place = current.getOrNull(to)?.position
                            if (from >= 0 && to != from && itemId != null && place != null) {
                                act(Action.MoveSetItemTo(itemId, place - 1u))
                            }
                            dragging = null
                            offset = 0f
                        },
                        onDragCancel = {
                            dragging = null
                            offset = 0f
                        },
                    ) { change, amount ->
                        change.consume()
                        offset += amount
                    }
                } }
                ReorderRow(
                    row,
                    first = index == 0,
                    last = index == rows.lastIndex,
                    onAction = onAction,
                    onRemove = { it, focused ->
                        if (focused) focusRemoveOn = (rows.getOrNull(index + 1) ?: rows.getOrNull(index - 1))?.selectionId
                        act(Action.RemoveSetItem(it))
                    },
                    focusRemove = focusRemoveOn == row.selectionId,
                    onRemoveFocused = { focusRemoveOn = null },
                    handle = handle,
                    dragged = dragged,
                    modifier = Modifier.zIndex(if (dragged) 1f else 0f).graphicsLayer { translationY = if (dragged) offset else 0f },
                )
            }
        }
        ListScrollbar(state, Modifier.align(Alignment.TopEnd).padding(vertical = 8.dp))
    }
}

private enum class Arrow { UP, DOWN }

/** One piece in reorder mode: handle, position, page, title and subtitle, then up, down and remove. */
@Composable
private fun ReorderRow(
    row: ScoreRowData,
    first: Boolean,
    last: Boolean,
    onAction: (Action) -> Unit,
    onRemove: (itemId: String, focused: Boolean) -> Unit,
    focusRemove: Boolean,
    onRemoveFocused: () -> Unit,
    handle: Modifier.() -> Modifier,
    dragged: Boolean,
    modifier: Modifier = Modifier,
) {
    val c = NotaeTheme.colors
    val itemId = row.itemId ?: return
    val title = row.title
    val up = remember { FocusRequester() }
    val down = remember { FocusRequester() }
    val remove = remember { FocusRequester() }
    var removeFocused by remember { mutableStateOf(false) }
    // The arrow that has the keyboard focus. A move that disables it (the row reached an end) hands the focus to the
    // row's other arrow, or to remove when both are disabled, instead of letting it fall back to the screen.
    var arrow by remember { mutableStateOf<Arrow?>(null) }
    val firstNow by rememberUpdatedState(first)
    val lastNow by rememberUpdatedState(last)
    LaunchedEffect(first, last) {
        val lost = (arrow == Arrow.UP && first) || (arrow == Arrow.DOWN && last)
        if (lost) {
            arrow = null
            when {
                !first -> up.requestFocus()
                !last -> down.requestFocus()
                else -> remove.requestFocus()
            }
        }
    }
    LaunchedEffect(focusRemove) {
        if (focusRemove) {
            remove.requestFocus()
            onRemoveFocused()
        }
    }
    Row(
        modifier
            .fillMaxWidth()
            .height(ROW_HEIGHT)
            .clip(RoundedCornerShape(18.dp))
            .background(if (dragged) c.accentSoft else Color.Transparent)
            .padding(end = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val drag = stringResource(R.string.set_drag, title)
        Box(Modifier.handle().size(48.dp).semantics { contentDescription = drag }, Alignment.Center) {
            Icon(painterResource(R.drawable.ic_grip_vertical), null, Modifier.size(24.dp), tint = c.muted)
        }
        row.position?.let { PositionBadge(it) }
        PagePreview(row.thumbnail, Modifier.size(44.dp, 58.dp), radius = 6.dp)
        Column(Modifier.weight(1f).semantics(mergeDescendants = true) {}, verticalArrangement = Arrangement.spacedBy(1.dp)) {
            val label = (row.position?.let { "$it. " } ?: "") + title
            Text(
                title,
                Modifier.semantics { contentDescription = label },
                color = c.foreground,
                fontSize = 16.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            ScoreSubtitle(row, 14)
        }
        IconButton(
            stringResource(R.string.set_move_up, title),
            painterResource(R.drawable.ic_arrow_up),
            { onAction(Action.MoveSetItem(itemId, -1)) },
            Modifier.focusRequester(up).onFocusChanged {
                if (it.isFocused) arrow = Arrow.UP else if (arrow == Arrow.UP && !firstNow) arrow = null
            },
            enabled = !first,
        )
        IconButton(
            stringResource(R.string.set_move_down, title),
            painterResource(R.drawable.ic_arrow_down),
            { onAction(Action.MoveSetItem(itemId, 1)) },
            Modifier.focusRequester(down).onFocusChanged {
                if (it.isFocused) arrow = Arrow.DOWN else if (arrow == Arrow.DOWN && !lastNow) arrow = null
            },
            enabled = !last,
        )
        IconButton(
            stringResource(R.string.picker_remove, title),
            painterResource(R.drawable.ic_delete),
            { onRemove(itemId, removeFocused) },
            Modifier.focusRequester(remove).onFocusChanged { removeFocused = it.isFocused },
        )
    }
}
