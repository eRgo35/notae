package com.c2yz.notae.ui.library

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.navigationevent.NavigationEventInfo
import androidx.navigationevent.compose.NavigationBackHandler
import androidx.navigationevent.compose.rememberNavigationEventState
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.LibraryView
import com.c2yz.notae.core.PdfOpenError
import com.c2yz.notae.core.SectionTitle
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.core.Subtitle
import com.c2yz.notae.ui.components.Chip
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.components.NotaeCard
import com.c2yz.notae.ui.components.OperationBanner
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.components.SidebarItem
import com.c2yz.notae.ui.components.toCount
import com.c2yz.notae.ui.theme.NotaeTheme

private const val SET_PREFIX = "set:"

/**
 * Slint `LibraryScreen`: the side panel (or the chips when it is hidden or the window is under 840 dp), the large
 * title with its header actions, the operation banner, the PDF launch error, and the scores as a grid or a list, or
 * the section's empty state. The ⋮ sheet is the screen's own state, as in Slint; Back closes it before anything else.
 */
@Composable
fun LibraryScreen(snapshot: Snapshot, onAction: (Action) -> Unit, modifier: Modifier = Modifier) {
    val c = NotaeTheme.colors
    var menuFor by remember { mutableStateOf<String?>(null) }
    val menuRow = menuFor?.let { id -> snapshot.rows.firstOrNull { it.selectionId == id } }
    NavigationBackHandler(
        state = rememberNavigationEventState(NavigationEventInfo.None),
        isBackEnabled = menuRow != null,
        onBackCompleted = { menuFor = null },
    )
    // Ctrl K opens search as soon as the library shows, as Slint focuses its shortcuts (app.slint), and again once the
    // sheet closes; while the sheet is open, Slint ignores it.
    val keys = remember { FocusRequester() }
    LaunchedEffect(menuRow == null) { if (menuRow == null) keys.requestFocus() }
    BoxWithConstraints(
        modifier
            .fillMaxSize()
            .background(c.background)
            .onPreviewKeyEvent { event ->
                (event.isCtrlK() && menuRow == null).also { if (it) onAction(Action.OpenSearch) }
            }
            .focusRequester(keys)
            .focusable(),
    ) {
        val expanded = maxWidth >= 840.dp
        val portrait = maxHeight > maxWidth
        val layout = snapshot.layout
        val collapsed = if (portrait) layout.sidebarCollapsedPortrait else layout.sidebarCollapsedLandscape
        val sidebar = expanded && !collapsed
        Row(Modifier.fillMaxSize().padding(16.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            if (sidebar) SidePanel(snapshot, onAction, Modifier.width(288.dp).fillMaxHeight())
            Column(Modifier.weight(1f).fillMaxHeight(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Header(snapshot, onAction, expanded, sidebar, portrait)
                if (!sidebar) Chips(snapshot, onAction)
                OperationBanner(snapshot.banner, onAction)
                snapshot.pdfOpenError?.let { Text(pdfErrorText(it), color = c.error, fontSize = 15.sp) }
                Content(snapshot, onAction, onMore = { menuFor = it.selectionId })
            }
        }
        if (menuRow != null) {
            ScoreActionsSheet(
                menuRow,
                onDismiss = { menuFor = null },
                onAction = {
                    menuFor = null
                    onAction(it)
                },
            )
        }
    }
}

@Composable
private fun Header(snapshot: Snapshot, onAction: (Action) -> Unit, expanded: Boolean, sidebar: Boolean, portrait: Boolean) {
    val c = NotaeTheme.colors
    val list = snapshot.layout.libraryView == LibraryView.LIST
    Row(
        Modifier.fillMaxWidth().padding(top = 8.dp, start = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (expanded) {
            IconButton(
                stringResource(if (sidebar) R.string.library_hide_sidebar else R.string.library_show_sidebar),
                painterResource(if (sidebar) R.drawable.ic_panel_left_close else R.drawable.ic_panel_left_open),
                { onAction(Action.SetSidebarCollapsed(portrait = portrait, collapsed = sidebar)) },
            )
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(
                sectionTitle(snapshot.title),
                Modifier.semantics { heading() },
                color = c.foreground,
                fontSize = if (expanded) 40.sp else 30.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(subtitleText(snapshot.subtitle), color = c.muted, fontSize = 15.sp)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(stringResource(R.string.screen_search), painterResource(R.drawable.ic_search), { onAction(Action.OpenSearch) })
            IconButton(
                stringResource(if (list) R.string.library_grid_view else R.string.library_list_view),
                painterResource(if (list) R.drawable.ic_layout_grid else R.drawable.ic_layout_list),
                { onAction(Action.SetLibraryView(if (list) LibraryView.GRID else LibraryView.LIST)) },
            )
            if (snapshot.sectionKey == "sets") {
                PillButton(stringResource(R.string.library_new_set), { onAction(Action.NewSet) }, icon = painterResource(R.drawable.ic_add), primary = true)
            }
            if (!sidebar) {
                IconButton(stringResource(R.string.screen_settings), painterResource(R.drawable.ic_settings), { onAction(Action.OpenSettings) })
            }
            // Scan is always right-most, so it never moves between screens.
            IconButton(
                stringResource(R.string.library_scan),
                painterResource(R.drawable.ic_scan),
                { onAction(Action.Rescan) },
                enabled = snapshot.rescanEnabled,
            )
        }
    }
}

/** Slint `SidePanel`: the logo, the sections with counts, the composers when there are any, and Settings. */
@Composable
private fun SidePanel(snapshot: Snapshot, onAction: (Action) -> Unit, modifier: Modifier) {
    val c = NotaeTheme.colors
    Column(modifier, verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Row(Modifier.height(64.dp).padding(start = 12.dp), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
            Image(painterResource(R.drawable.notae), null, Modifier.size(40.dp), contentScale = ContentScale.Fit)
            Text(stringResource(R.string.brand_name), color = c.foreground, fontSize = 22.sp, fontWeight = FontWeight.Bold)
        }
        NotaeCard(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(8.dp)) {
                snapshot.sections.forEach { section ->
                    SidebarItem(
                        sectionTitle(section.title),
                        { onAction(Action.SelectSection(section.key)) },
                        icon = painterResource(sectionIcon(section.key)),
                        count = section.count.toCount(),
                        selected = isSelected(section.key, snapshot.sectionKey),
                    )
                }
            }
        }
        if (snapshot.composers.isNotEmpty()) {
            NotaeCard(Modifier.fillMaxWidth().weight(1f)) {
                Column(Modifier.padding(8.dp)) {
                    Box(Modifier.height(36.dp).padding(start = 14.dp), contentAlignment = Alignment.CenterStart) {
                        Text(stringResource(R.string.library_composers), color = c.muted, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
                    }
                    LazyColumn {
                        items(snapshot.composers, key = { it.key }) { composer ->
                            SidebarItem(
                                composer.name,
                                { onAction(Action.SelectSection(composer.key)) },
                                count = composer.count.toCount(),
                                selected = composer.key == snapshot.sectionKey,
                            )
                        }
                    }
                }
            }
        } else {
            Spacer(Modifier.weight(1f))
        }
        SidebarItem(stringResource(R.string.screen_settings), { onAction(Action.OpenSettings) }, icon = painterResource(R.drawable.ic_settings))
    }
}

/** The side panel's rows as chips, for when the panel is not shown; they scroll sideways when they do not fit. */
@Composable
private fun Chips(snapshot: Snapshot, onAction: (Action) -> Unit) {
    Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        snapshot.sections.forEach { section ->
            Chip(sectionTitle(section.title), isSelected(section.key, snapshot.sectionKey), { onAction(Action.SelectSection(section.key)) })
        }
        snapshot.composers.forEach { composer ->
            Chip(composer.name, composer.key == snapshot.sectionKey, { onAction(Action.SelectSection(composer.key)) })
        }
    }
}

@Composable
private fun ColumnScope.Content(snapshot: Snapshot, onAction: (Action) -> Unit, onMore: (com.c2yz.notae.core.Row) -> Unit) {
    val area = Modifier.weight(1f).fillMaxWidth()
    val open = { row: com.c2yz.notae.core.Row -> onAction(Action.OpenPdf(row.relativePath, row.noteKey, row.itemId)) }
    when {
        snapshot.sectionKey == "sets" && snapshot.setCards.isEmpty() -> EmptyState(snapshot, onAction, area)
        // The set cards arrive with the sets screens (Task 17).
        snapshot.sectionKey == "sets" -> Box(area, contentAlignment = Alignment.Center) { Text(stringResource(R.string.section_sets)) }
        snapshot.rows.isEmpty() -> EmptyState(snapshot, onAction, area)
        snapshot.layout.libraryView == LibraryView.LIST -> ScoreList(snapshot.rows, open, onMore, area)
        else -> ScoreGrid(snapshot.rows, open, onMore, area)
    }
}

/** The section's empty card: folder glyph, title and detail; All scores also offers Scan library. */
@Composable
private fun EmptyState(snapshot: Snapshot, onAction: (Action) -> Unit, modifier: Modifier) {
    val c = NotaeTheme.colors
    val (title, detail) = when (snapshot.sectionKey) {
        "all" -> R.string.library_empty_all to R.string.library_empty_all_detail
        "recent" -> R.string.library_empty_recent to R.string.library_empty_recent_detail
        "favorites" -> R.string.library_empty_favorites to R.string.library_empty_favorites_detail
        "sets" -> R.string.library_empty_sets to R.string.library_empty_sets_detail
        else -> R.string.library_empty_composer to R.string.library_empty_composer_detail
    }
    NotaeCard(modifier) {
        Column(
            Modifier.fillMaxSize().padding(32.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterVertically),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Icon(painterResource(R.drawable.ic_folder), null, Modifier.size(56.dp), tint = c.muted)
            Text(stringResource(title), color = c.foreground, fontSize = 22.sp, fontWeight = FontWeight.SemiBold, textAlign = TextAlign.Center)
            Text(stringResource(detail), color = c.muted, fontSize = 15.sp, textAlign = TextAlign.Center)
            if (snapshot.sectionKey == "all") {
                PillButton(
                    stringResource(R.string.library_scan),
                    { onAction(Action.Rescan) },
                    Modifier.padding(top = 8.dp),
                    icon = painterResource(R.drawable.ic_scan),
                    primary = true,
                    enabled = snapshot.rescanEnabled,
                )
            }
        }
    }
}

/** Ctrl K, which opens search from the library and the details (Slint's root shortcut scope, app.slint). */
internal fun KeyEvent.isCtrlK(): Boolean = type == KeyEventType.KeyDown && isCtrlPressed && key == Key.K

/** A side-panel row is selected for its own section; the Sets row also inside a set. */
private fun isSelected(key: String, current: String): Boolean = key == current || (key == "sets" && current.startsWith(SET_PREFIX))

private fun sectionIcon(key: String): Int = when (key) {
    "all" -> R.drawable.ic_library
    "recent" -> R.drawable.ic_schedule
    "favorites" -> R.drawable.ic_star
    else -> R.drawable.ic_folder
}

/** A section's title or side-panel label (texts.rs `section_title`). */
@Composable
internal fun sectionTitle(title: SectionTitle): String = when (title) {
    SectionTitle.All -> stringResource(R.string.section_all)
    SectionTitle.Recent -> stringResource(R.string.section_recent)
    SectionTitle.Favorites -> stringResource(R.string.section_favorites)
    SectionTitle.Sets -> stringResource(R.string.section_sets)
    is SectionTitle.Composer -> title.name
    is SectionTitle.Set -> title.name
}

/** "N scores" or "N sets · recently edited first". A set's own subtitle is drawn by the set view (Task 17). */
@Composable
private fun subtitleText(subtitle: Subtitle): String = when (subtitle) {
    is Subtitle.Scores -> pluralStringResource(R.plurals.library_scores, subtitle.count.toCount(), subtitle.count.toLong())
    is Subtitle.Sets -> pluralStringResource(R.plurals.library_sets_subtitle, subtitle.count.toCount(), subtitle.count.toLong())
    is Subtitle.Set -> ""
}

/** Why the PDF could not be opened, in the English it has always carried (texts.rs `viewer_error`). */
@Composable
internal fun pdfErrorText(error: PdfOpenError): String = when (error) {
    PdfOpenError.InvalidContentUri -> stringResource(R.string.viewer_error_invalid_uri)
    PdfOpenError.NoCompatibleViewer -> stringResource(R.string.viewer_error_no_viewer)
    is PdfOpenError.Platform -> stringResource(R.string.viewer_error_platform, error.detail)
}
