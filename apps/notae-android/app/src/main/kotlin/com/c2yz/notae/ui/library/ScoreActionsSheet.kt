package com.c2yz.notae.ui.library

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Row as ScoreRowData
import com.c2yz.notae.ui.components.ActionSheet
import com.c2yz.notae.ui.components.SheetAction
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * The score's ⋮ sheet (0.3.0's `LibraryScreen`'s actions sheet): handle, page, title and "number · composer", then
 * Open score, the favorite toggle, Add to set…, Score details, Edit details and, inside a set, Remove from set. A
 * piece missing from the library offers only Remove from set. Every action closes the sheet; focus starts on the first.
 */
@Composable
internal fun ScoreActionsSheet(row: ScoreRowData, onDismiss: () -> Unit, onAction: (Action) -> Unit) {
    val c = NotaeTheme.colors
    val first = remember { FocusRequester() }
    ActionSheet(
        stringResource(R.string.score_actions_dialog),
        onDismiss,
        padding = PaddingValues(start = 8.dp, top = 8.dp, end = 8.dp, bottom = 16.dp),
        spacing = 0.dp,
    ) {
        Box(Modifier.fillMaxWidth().padding(bottom = 8.dp), Alignment.Center) {
            Box(Modifier.size(32.dp, 4.dp).clip(RoundedCornerShape(2.dp)).background(c.divider))
        }
        Row(
            Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            PagePreview(row.thumbnail, Modifier.size(44.dp, 58.dp), radius = 6.dp)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(row.title, color = c.foreground, fontSize = 18.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                ScoreSubtitle(row, 14, missingInError = false)
            }
        }
        Box(Modifier.fillMaxWidth().height(1.dp).background(c.divider))
        val itemId = row.itemId
        if (row.missing) {
            if (itemId != null) {
                SheetAction(stringResource(R.string.score_remove_from_set), painterResource(R.drawable.ic_delete), { onAction(Action.RemoveSetItem(itemId)) }, Modifier.focusRequester(first))
            }
        } else {
            SheetAction(
                stringResource(R.string.score_open),
                painterResource(R.drawable.ic_external_link),
                { onAction(Action.OpenPdf(row.relativePath, row.noteKey, row.itemId)) },
                Modifier.focusRequester(first),
            )
            SheetAction(
                stringResource(if (row.favorite) R.string.score_remove_favorite else R.string.score_add_favorite),
                painterResource(if (row.favorite) R.drawable.ic_star_filled else R.drawable.ic_star),
                { onAction(Action.ToggleFavorite(row.noteKey)) },
            )
            SheetAction(stringResource(R.string.score_add_to_set), painterResource(R.drawable.ic_folder), { onAction(Action.AddToSet(row.noteKey)) })
            // Inside a set the selection id names this copy of a repeated piece.
            SheetAction(stringResource(R.string.score_details), painterResource(R.drawable.ic_info), { onAction(Action.SelectScore(row.selectionId)) })
            SheetAction(stringResource(R.string.score_edit_details), painterResource(R.drawable.ic_edit), { onAction(Action.EditMetadata(row.noteKey)) })
            if (itemId != null) {
                SheetAction(stringResource(R.string.score_remove_from_set), painterResource(R.drawable.ic_delete), { onAction(Action.RemoveSetItem(itemId)) })
            }
        }
    }
    // A missing piece outside a set (which the core never sends) would have no action to focus.
    val hasAction = !row.missing || row.itemId != null
    LaunchedEffect(row.selectionId) {
        if (hasAction) first.requestFocus()
    }
}
