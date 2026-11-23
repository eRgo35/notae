package com.c2yz.notae.ui.viewer

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MenuDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.ViewerLayout
import com.c2yz.notae.core.ViewerPages
import com.c2yz.notae.core.ViewerScale
import com.c2yz.notae.ui.theme.NotaeTheme

/** The percentages the View menu offers. */
private val PERCENTS = listOf(50, 75, 100, 125, 150, 200)

/**
 * The viewer's View menu: Layout (Pages / Scroll), On screen (1 / 2 pages) and Scale (fit page(s), width or height, or
 * a percentage). [look] marks the current choice; once pinched, [livePercent] heads the scale group and no scale is
 * marked. A pick dispatches [onPick] with the picked group changed and the other two as [stored], then closes the menu.
 */
@Composable
fun ViewMenu(
    look: ViewLook,
    stored: Triple<ViewerLayout, ViewerPages, ViewerScale>,
    livePercent: Int?,
    expanded: Boolean,
    onDismiss: () -> Unit,
    onPick: (ViewerLayout, ViewerPages, ViewerScale) -> Unit,
) {
    val c = NotaeTheme.colors
    val (layout, pages, scale) = stored
    val pick = { l: ViewerLayout, p: ViewerPages, s: ViewerScale ->
        onDismiss()
        onPick(l, p, s)
    }
    val shownScale = look.scale.takeIf { livePercent == null }
    DropdownMenu(
        expanded = expanded,
        onDismissRequest = onDismiss,
        shape = RoundedCornerShape(20.dp),
        containerColor = c.card,
        border = BorderStroke(1.dp, c.divider),
        modifier = Modifier.widthIn(min = 260.dp),
    ) {
        GroupTitle(stringResource(R.string.viewer_view_layout))
        Choice(stringResource(R.string.viewer_view_pages), !look.scroll) { pick(ViewerLayout.PAGES, pages, scale) }
        Choice(stringResource(R.string.viewer_view_scroll), look.scroll) { pick(ViewerLayout.SCROLL, pages, scale) }
        GroupTitle(stringResource(R.string.viewer_view_on_screen))
        Choice(stringResource(R.string.viewer_view_one_page), !look.twoPages) { pick(layout, ViewerPages.ONE, scale) }
        Choice(stringResource(R.string.viewer_view_two_pages), look.twoPages) { pick(layout, ViewerPages.TWO, scale) }
        GroupTitle(livePercent?.let { stringResource(R.string.viewer_view_scale_now, it) } ?: stringResource(R.string.viewer_view_scale))
        val fitPage = if (look.twoPages) R.string.viewer_view_fit_pages else R.string.viewer_view_fit_page
        Choice(stringResource(fitPage), shownScale == ViewerScale.FitPage) { pick(layout, pages, ViewerScale.FitPage) }
        Choice(stringResource(R.string.viewer_view_fit_width), shownScale == ViewerScale.FitWidth) { pick(layout, pages, ViewerScale.FitWidth) }
        Choice(stringResource(R.string.viewer_view_fit_height), shownScale == ViewerScale.FitHeight) { pick(layout, pages, ViewerScale.FitHeight) }
        FlowRow(
            Modifier.padding(horizontal = 12.dp, vertical = 8.dp).widthIn(max = 300.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            for (percent in PERCENTS) {
                val value = ViewerScale.Percent(percent.toUShort())
                PercentPill(stringResource(R.string.viewer_view_percent, percent), shownScale == value) { pick(layout, pages, value) }
            }
        }
    }
}

@Composable
private fun GroupTitle(text: String) {
    Text(
        text,
        Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 4.dp),
        color = NotaeTheme.colors.muted,
        fontSize = 14.sp,
        fontWeight = FontWeight.SemiBold,
    )
}

/** A menu row with a tick when [selected]. */
@Composable
private fun Choice(label: String, selected: Boolean, onClick: () -> Unit) {
    val c = NotaeTheme.colors
    DropdownMenuItem(
        text = { Text(label, fontSize = 16.sp) },
        onClick = onClick,
        leadingIcon = {
            Box(Modifier.size(24.dp)) {
                if (selected) Icon(painterResource(R.drawable.ic_check), null, Modifier.size(24.dp), tint = c.accent)
            }
        },
        colors = MenuDefaults.itemColors(textColor = c.foreground),
        modifier = Modifier.heightIn(min = 48.dp).semantics(mergeDescendants = true) {
            this.selected = selected
            role = Role.RadioButton
        },
    )
}

/** A percentage as a small pill, filled when [selected]. */
@Composable
private fun PercentPill(label: String, selected: Boolean, onClick: () -> Unit) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(16.dp)
    Text(
        label,
        Modifier
            .clip(shape)
            .then(if (selected) Modifier.background(c.accent) else Modifier.border(1.dp, c.divider, shape))
            .semantics(mergeDescendants = true) {
                this.selected = selected
                role = Role.RadioButton
            }
            .clickable(onClick = onClick)
            .heightIn(min = 48.dp)
            .wrapContentHeight()
            .padding(horizontal = 14.dp),
        color = if (selected) c.onAccent else c.foreground,
        fontSize = 15.sp,
    )
}
