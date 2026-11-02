package com.c2yz.notae.ui.library

import com.c2yz.notae.ui.components.backProgress
import com.c2yz.notae.ui.components.LocalBackProgress
import com.c2yz.notae.ui.components.AnimatedOverlay
import androidx.navigationevent.compose.rememberNavigationEventState
import androidx.navigationevent.compose.NavigationBackHandler
import androidx.navigationevent.NavigationEventInfo
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MenuDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.ripple
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.LibraryView
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.ui.components.ActionSheet
import com.c2yz.notae.ui.components.Chip
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.components.OperationBanner
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.components.toCount
import com.c2yz.notae.ui.theme.NotaeTheme

/** The bottom navigation bar's height, which the floating button keeps clear of. */
private val BOTTOM_BAR = 72.dp

private const val COMPOSER_PREFIX = "composer:"

/**
 * The library on a phone, in One UI's shape: a large title over the top of a tall screen that folds into the toolbar as
 * the scores scroll, the toolbar (Back to sets inside a set, Search, Reorder inside a set and ⋮ with Scan library, the
 * grid or list choice and Settings), the composer filter, the banner and the scores; the sections in a bottom bar, or
 * Done at the bottom while reordering; New set or Add as a floating button.
 */
@Composable
internal fun CompactLibrary(
    snapshot: Snapshot,
    onAction: (Action) -> Unit,
    inSet: Boolean,
    reordering: Boolean,
    content: @Composable ColumnScope.() -> Unit,
) {
    val c = NotaeTheme.colors
    var composerSheet by remember { mutableStateOf(false) }
    BoxWithConstraints(Modifier.fillMaxSize()) {
        // A short window (a phone on its side, a split screen) has no room for the large title.
        val titleSpace = if (maxHeight >= 560.dp) (maxHeight * 0.24f).coerceIn(140.dp, 220.dp) else 0.dp
        val density = LocalDensity.current
        val range = with(density) { titleSpace.toPx() }
        var folded by remember(range) { mutableFloatStateOf(0f) }
        val collapse = remember(range) { TitleCollapse(range, { folded }, { folded = it }) }
        val fraction = if (range == 0f) 1f else folded / range
        Column(Modifier.fillMaxSize()) {
            Column(Modifier.weight(1f).fillMaxWidth().nestedScroll(collapse)) {
                if (titleSpace > 0.dp) {
                    LargeTitle(
                        snapshot,
                        inSet,
                        reordering,
                        Modifier
                            .height(titleSpace - with(density) { folded.toDp() })
                            .alpha((1f - fraction * 1.6f).coerceIn(0f, 1f)),
                    )
                }
                Toolbar(snapshot, onAction, inSet, reordering, showTitle = fraction > 0.6f)
                Column(
                    Modifier.weight(1f).fillMaxWidth().padding(horizontal = 12.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    if (showsComposers(snapshot, inSet)) {
                        ComposerChip(composerName(snapshot), { composerSheet = true })
                    }
                    OperationBanner(snapshot.banner, onAction)
                    snapshot.pdfOpenError?.let { Text(pdfErrorText(it), color = c.error, fontSize = 15.sp) }
                    content()
                    Spacer(Modifier.height(4.dp))
                }
            }
            if (reordering) DoneBar(onAction) else BottomNav(snapshot, onAction)
        }
        val fab = when {
            snapshot.sectionKey == "sets" -> Triple(R.string.library_new_set, R.drawable.ic_add, Action.NewSet)
            inSet && !reordering -> Triple(R.string.set_add, R.drawable.ic_add, Action.OpenAddPicker)
            else -> null
        }
        fab?.let { (label, icon, action) ->
            PillButton(
                stringResource(label),
                { onAction(action) },
                Modifier
                    .align(Alignment.BottomEnd)
                    .padding(end = 16.dp, bottom = BOTTOM_BAR + 16.dp)
                    .shadow(6.dp, RoundedCornerShape(26.dp)),
                icon = painterResource(icon),
                primary = true,
            )
        }
        // Back closes the sheet before it reaches the core, which would otherwise leave the app under it.
        val backState = rememberNavigationEventState(NavigationEventInfo.None)
        NavigationBackHandler(state = backState, isBackEnabled = composerSheet, onBackCompleted = { composerSheet = false })
        CompositionLocalProvider(LocalBackProgress provides backProgress(backState)) {
            AnimatedOverlay(if (composerSheet) Unit else null) {
                ComposerSheet(snapshot, { composerSheet = false }) {
                    composerSheet = false
                    onAction(Action.SelectSection(it))
                }
            }
        }
    }
}

/**
 * The library on a small tablet or a phone on its side: a navigation rail with the sections and Settings, then the
 * tablet's header (without the side-panel toggle) and the composers as chips.
 */
@Composable
internal fun MediumLibrary(
    snapshot: Snapshot,
    onAction: (Action) -> Unit,
    header: @Composable () -> Unit,
    inSet: Boolean,
    content: @Composable ColumnScope.() -> Unit,
) {
    val c = NotaeTheme.colors
    Row(Modifier.fillMaxSize()) {
        NavRail(snapshot, onAction, Modifier.width(88.dp).fillMaxHeight())
        Column(
            Modifier.weight(1f).fillMaxHeight().padding(top = 16.dp, end = 16.dp, bottom = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            header()
            if (showsComposers(snapshot, inSet)) {
                Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Chip(stringResource(R.string.all_composers), snapshot.sectionKey == "all", { onAction(Action.SelectSection("all")) })
                    snapshot.composers.forEach { composer ->
                        Chip(composer.name, composer.key == snapshot.sectionKey, { onAction(Action.SelectSection(composer.key)) })
                    }
                }
            }
            OperationBanner(snapshot.banner, onAction)
            snapshot.pdfOpenError?.let { Text(pdfErrorText(it), color = c.error, fontSize = 15.sp) }
            content()
        }
    }
}

/** The composer filter shows on All scores and on a composer's scores, when the library has composers. */
private fun showsComposers(snapshot: Snapshot, inSet: Boolean): Boolean =
    !inSet && snapshot.composers.isNotEmpty() && (snapshot.sectionKey == "all" || snapshot.sectionKey.startsWith(COMPOSER_PREFIX))

@Composable
private fun composerName(snapshot: Snapshot): String =
    snapshot.composers.firstOrNull { it.key == snapshot.sectionKey }?.name ?: stringResource(R.string.all_composers)

/** A bottom bar or rail section is selected for its own section; All scores also for a composer's, Sets inside a set. */
private fun navSelected(key: String, current: String): Boolean =
    key == current || (key == "sets" && current.startsWith("set:")) || (key == "all" && current.startsWith(COMPOSER_PREFIX))

/**
 * Folds the large title as the content scrolls up, before the list moves, and unfolds it once the list is back at its
 * top, as One UI's collapsing app bar does.
 */
private class TitleCollapse(
    private val range: Float,
    private val folded: () -> Float,
    private val setFolded: (Float) -> Unit,
) : NestedScrollConnection {
    override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset {
        if (available.y >= 0f) return Offset.Zero
        return Offset(0f, fold(available.y))
    }

    override fun onPostScroll(consumed: Offset, available: Offset, source: NestedScrollSource): Offset {
        if (available.y <= 0f) return Offset.Zero
        return Offset(0f, fold(available.y))
    }

    /** Moves the title by [delta] (negative folds it) within its range and returns what it took. */
    private fun fold(delta: Float): Float {
        val before = folded()
        val after = (before - delta).coerceIn(0f, range)
        setFolded(after)
        return before - after
    }
}

/** One UI's large title: the section's name and its subtitle at the bottom left of the space above the toolbar. */
@Composable
private fun LargeTitle(snapshot: Snapshot, inSet: Boolean, reordering: Boolean, modifier: Modifier) {
    val c = NotaeTheme.colors
    Box(modifier.fillMaxWidth().clipToBounds().padding(start = 24.dp, end = 24.dp, bottom = 12.dp), Alignment.BottomStart) {
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(
                sectionTitle(snapshot.title),
                Modifier.semantics { heading() },
                color = c.foreground,
                fontSize = 34.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                lineHeight = 40.sp,
            )
            Text(if (reordering) stringResource(R.string.set_reorder_hint) else subtitleText(snapshot.subtitle), color = c.muted, fontSize = 15.sp)
            val notes = snapshot.setNotes
            if (inSet && !notes.isNullOrEmpty()) {
                Text(notes, color = c.foreground, fontSize = 15.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/** The phone's toolbar; the section's name shows in it once the large title has folded away. */
@Composable
private fun Toolbar(snapshot: Snapshot, onAction: (Action) -> Unit, inSet: Boolean, reordering: Boolean, showTitle: Boolean) {
    val c = NotaeTheme.colors
    Row(Modifier.fillMaxWidth().height(64.dp).padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        if (inSet) {
            IconButton(stringResource(R.string.set_back), painterResource(R.drawable.ic_arrow_back), { onAction(Action.SelectSection("sets")) })
        }
        Box(Modifier.weight(1f).padding(start = if (inSet) 4.dp else 16.dp)) {
            if (showTitle) {
                Text(
                    sectionTitle(snapshot.title),
                    Modifier.semantics { heading() },
                    color = c.foreground,
                    fontSize = 20.sp,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        IconButton(stringResource(R.string.screen_search), painterResource(R.drawable.ic_search), { onAction(Action.OpenSearch) })
        if (inSet && !reordering) {
            IconButton(stringResource(R.string.set_reorder), painterResource(R.drawable.ic_reorder), { onAction(Action.ReorderStarted) })
        }
        OverflowMenu(snapshot, onAction, reordering)
    }
}

/** ⋮: Scan library, the grid or list choice (not while reordering, which always shows the list) and Settings. */
@Composable
private fun OverflowMenu(snapshot: Snapshot, onAction: (Action) -> Unit, reordering: Boolean) {
    val c = NotaeTheme.colors
    var open by remember { mutableStateOf(false) }
    val list = snapshot.layout.libraryView == LibraryView.LIST
    val choose = { action: Action ->
        open = false
        onAction(action)
    }
    Box {
        IconButton(stringResource(R.string.more_options), painterResource(R.drawable.ic_more_vertical), { open = true })
        DropdownMenu(
            expanded = open,
            onDismissRequest = { open = false },
            shape = RoundedCornerShape(20.dp),
            containerColor = c.card,
        ) {
            MenuItem(stringResource(R.string.library_scan), painterResource(R.drawable.ic_scan), { choose(Action.Rescan) }, enabled = snapshot.rescanEnabled)
            if (!reordering) {
                MenuItem(
                    stringResource(if (list) R.string.library_grid_view else R.string.library_list_view),
                    painterResource(if (list) R.drawable.ic_layout_grid else R.drawable.ic_layout_list),
                    { choose(Action.SetLibraryView(if (list) LibraryView.GRID else LibraryView.LIST)) },
                )
            }
            MenuItem(stringResource(R.string.screen_settings), painterResource(R.drawable.ic_settings), { choose(Action.OpenSettings) })
        }
    }
}

@Composable
private fun MenuItem(label: String, icon: Painter, onClick: () -> Unit, enabled: Boolean = true) {
    val c = NotaeTheme.colors
    DropdownMenuItem(
        text = { Text(label, fontSize = 16.sp) },
        onClick = onClick,
        leadingIcon = { Icon(icon, null, Modifier.size(24.dp)) },
        enabled = enabled,
        colors = MenuDefaults.itemColors(
            textColor = c.foreground,
            leadingIconColor = c.foreground,
            disabledTextColor = c.muted,
            disabledLeadingIconColor = c.muted,
        ),
        modifier = Modifier.heightIn(min = 52.dp),
    )
}

/** The composer filter: a pill with the composer shown (or All composers) and a chevron, opening [ComposerSheet]. */
@Composable
private fun ComposerChip(name: String, onClick: () -> Unit) {
    val c = NotaeTheme.colors
    val label = stringResource(R.string.choose_composer, name)
    val shape = RoundedCornerShape(20.dp)
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    Box(
        Modifier
            .height(48.dp)
            .semantics { contentDescription = label }
            .clickable(source, null, role = Role.Button, onClick = onClick)
            .padding(vertical = 4.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        Row(
            Modifier
                .height(40.dp)
                .clip(shape)
                .background(c.card)
                .then(if (focused) Modifier.border(BorderStroke(2.dp, c.accent), shape) else Modifier)
                .padding(start = 18.dp, end = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(name, Modifier.clearAndSetSemantics { }, color = c.foreground, fontSize = 15.sp, fontWeight = FontWeight.Medium, maxLines = 1)
            Icon(painterResource(R.drawable.ic_chevron_down), null, Modifier.size(20.dp), tint = c.muted)
        }
    }
}

/** The composers as a radio list in a bottom sheet: All composers first, then each composer with their count. */
@Composable
private fun ComposerSheet(snapshot: Snapshot, onDismiss: () -> Unit, onSelect: (String) -> Unit) {
    val c = NotaeTheme.colors
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val listHeight = maxOf(112.dp, maxHeight - 200.dp)
        ActionSheet(stringResource(R.string.library_composers), onDismiss, spacing = 8.dp) {
            Box(Modifier.fillMaxWidth(), Alignment.Center) {
                Box(Modifier.size(32.dp, 4.dp).clip(RoundedCornerShape(2.dp)).background(c.divider))
            }
            Text(stringResource(R.string.library_composers), color = c.foreground, fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
            LazyColumn(Modifier.heightIn(max = listHeight).selectableGroup()) {
                item { RadioRow(stringResource(R.string.all_composers), null, snapshot.sectionKey == "all") { onSelect("all") } }
                items(snapshot.composers, key = { it.key }) { composer ->
                    RadioRow(composer.name, composer.count.toCount(), composer.key == snapshot.sectionKey) { onSelect(composer.key) }
                }
            }
        }
    }
}

/** A 56 dp radio row: the name, its count and the check when it is the one shown. */
@Composable
private fun RadioRow(label: String, count: Int?, selected: Boolean, onClick: () -> Unit) {
    val c = NotaeTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .height(56.dp)
            .clip(RoundedCornerShape(16.dp))
            .selectable(selected, role = Role.RadioButton, onClick = onClick)
            .padding(horizontal = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            label,
            Modifier.weight(1f),
            color = if (selected) c.accentText else c.foreground,
            fontSize = 16.sp,
            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        count?.let { Text(it.toString(), color = c.muted, fontSize = 14.sp) }
        Box(Modifier.size(24.dp), Alignment.Center) {
            if (selected) Icon(painterResource(R.drawable.ic_check), null, Modifier.size(24.dp), tint = c.accent)
        }
    }
}

/** The sections as One UI's bottom navigation: an icon in a pill over a short label, the current one highlighted. */
@Composable
private fun BottomNav(snapshot: Snapshot, onAction: (Action) -> Unit) {
    val c = NotaeTheme.colors
    Column(Modifier.fillMaxWidth().background(c.background)) {
        Box(Modifier.fillMaxWidth().height(1.dp).background(c.divider))
        Row(Modifier.fillMaxWidth().height(BOTTOM_BAR - 1.dp).selectableGroup(), verticalAlignment = Alignment.CenterVertically) {
            snapshot.sections.forEach { section ->
                NavItem(
                    sectionTitle(section.title),
                    painterResource(sectionIcon(section.key)),
                    navSelected(section.key, snapshot.sectionKey),
                    { onAction(Action.SelectSection(section.key)) },
                    Modifier.weight(1f).fillMaxHeight(),
                )
            }
        }
    }
}

/** Done at the bottom of the screen while reordering, where the thumb is. */
@Composable
private fun DoneBar(onAction: (Action) -> Unit) {
    val c = NotaeTheme.colors
    Box(Modifier.fillMaxWidth().height(BOTTOM_BAR).background(c.background).padding(horizontal = 16.dp, vertical = 8.dp), Alignment.Center) {
        PillButton(
            stringResource(R.string.picker_done),
            { onAction(Action.ReorderFinished) },
            Modifier.fillMaxWidth(),
            icon = painterResource(R.drawable.ic_check),
            primary = true,
        )
    }
}

/** The sections down the side, with Settings at the foot, for a window between a phone's and a tablet's. */
@Composable
private fun NavRail(snapshot: Snapshot, onAction: (Action) -> Unit, modifier: Modifier) {
    Column(modifier.padding(vertical = 16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Column(Modifier.selectableGroup(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            snapshot.sections.forEach { section ->
                NavItem(
                    sectionTitle(section.title),
                    painterResource(sectionIcon(section.key)),
                    navSelected(section.key, snapshot.sectionKey),
                    { onAction(Action.SelectSection(section.key)) },
                    Modifier.width(80.dp).height(68.dp),
                )
            }
        }
        Spacer(Modifier.weight(1f))
        IconButton(stringResource(R.string.screen_settings), painterResource(R.drawable.ic_settings), { onAction(Action.OpenSettings) })
    }
}

/** One destination of the bottom bar or the rail; read as its label, selected or not. */
@Composable
private fun NavItem(label: String, icon: Painter, selected: Boolean, onClick: () -> Unit, modifier: Modifier) {
    val c = NotaeTheme.colors
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    Column(
        modifier
            .selectable(selected = selected, interactionSource = source, indication = ripple(bounded = false, radius = 36.dp), role = Role.Tab, onClick = onClick)
            .padding(vertical = 6.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp, Alignment.CenterVertically),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        val pill = RoundedCornerShape(16.dp)
        Box(
            Modifier
                .size(56.dp, 32.dp)
                .clip(pill)
                .background(if (selected) c.accentSoft else Color.Transparent)
                .then(if (focused) Modifier.border(BorderStroke(2.dp, c.accent), pill) else Modifier),
            Alignment.Center,
        ) {
            Icon(icon, null, Modifier.size(24.dp), tint = if (selected) c.onAccentSoft else c.muted)
        }
        Text(
            label,
            color = if (selected) c.foreground else c.muted,
            fontSize = 12.sp,
            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            textAlign = TextAlign.Center,
        )
    }
}
