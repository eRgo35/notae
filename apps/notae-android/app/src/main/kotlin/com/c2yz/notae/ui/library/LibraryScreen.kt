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
import com.c2yz.notae.core.SetCard
import com.c2yz.notae.core.SetEntry
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.core.Subtitle
import com.c2yz.notae.ui.components.Chip
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.components.NotaeCard
import com.c2yz.notae.ui.components.OperationBanner
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.components.SidebarItem
import com.c2yz.notae.ui.components.toCount
import com.c2yz.notae.ui.details.detailsKeyAction
import com.c2yz.notae.ui.sets.SetActionsSheet
import com.c2yz.notae.ui.sets.overlayOpen
import com.c2yz.notae.ui.sets.SetsPane
import com.c2yz.notae.ui.sets.setSubtitle
import com.c2yz.notae.ui.theme.NotaeTheme

private const val SET_PREFIX = "set:"

/**
 * 0.3.0's `LibraryScreen`: the side panel (or the chips when it is hidden or the window is under 840 dp), the large
 * title with its header actions, the operation banner, the PDF launch error, and the scores as a grid or a list, the
 * sets as folder cards or rows, or the section's empty state. Inside a set the header gains Back to sets, the set's
 * subtitle and notes, and Add. The ⋮ sheets are the screen's own state, as in 0.3.0; Back and Escape close them first.
 */
@Composable
fun LibraryScreen(snapshot: Snapshot, onAction: (Action) -> Unit, modifier: Modifier = Modifier) {
    val c = NotaeTheme.colors
    var menuFor by remember { mutableStateOf<String?>(null) }
    val menuRow = menuFor?.let { id -> snapshot.rows.firstOrNull { it.selectionId == id } }
    var setMenuFor by remember { mutableStateOf<String?>(null) }
    val menuSet = setMenuFor?.let { id -> snapshot.setCards.firstOrNull { it.id == id } }
    val inSet = snapshot.sectionKey.startsWith(SET_PREFIX)
    // Inside a set, its pieces and text notes in set order; elsewhere the section's scores.
    val entries = remember(inSet, snapshot.rows, snapshot.setEntries) {
        if (inSet) snapshot.setEntries else snapshot.rows.map { SetEntry.Piece(it) }
    }
    // A note's ⋮ sheet and its editor, by item id; they close when the note leaves the set.
    var noteMenuFor by remember { mutableStateOf<String?>(null) }
    var noteEditFor by remember { mutableStateOf<String?>(null) }
    val note = { id: String? -> id?.let { entries.firstOrNull { it is SetEntry.Note && it.itemId == id } as SetEntry.Note? } }
    val menuNote = note(noteMenuFor)
    val editNote = note(noteEditFor)
    val sheetOpen = menuRow != null || menuSet != null || menuNote != null || editNote != null
    val closeSheets = {
        menuFor = null
        setMenuFor = null
        noteMenuFor = null
        noteEditFor = null
    }
    NavigationBackHandler(
        state = rememberNavigationEventState(NavigationEventInfo.None),
        isBackEnabled = sheetOpen,
        onBackCompleted = closeSheets,
    )
    val reordering = inSet && snapshot.reordering
    // The keys work as soon as the library shows, as 0.3.0 focuses its shortcuts, and again once a sheet,
    // dialog or picker closes. While one is open, 0.3.0 ignored them (Escape closes a ⋮ sheet).
    val overlay = overlayOpen(snapshot)
    // On a wide window the details sit beside the library (NotaeNavDisplay); then they take focus after the library
    // does, and the library passes their keys on when it holds focus itself (after its own ⋮ sheet closes).
    val selection = snapshot.selection
    val keys = remember { FocusRequester() }
    // Also when reorder mode starts or ends (the Reorder or Done button that held focus is gone), and when the last piece
    // is removed in reorder mode (the list holding the focus is gone).
    val reorderEmpty = reordering && entries.isEmpty()
    // Also when a ⋮ sheet opens: in touch mode its actions cannot take focus, which would otherwise stay on the details
    // beside the library, so Escape would close the details instead of the sheet.
    LaunchedEffect(sheetOpen, overlay, snapshot.sectionKey, reordering, reorderEmpty) { if (!overlay) keys.requestFocus() }
    // Only when the details close (beside the library, which stays put); never when they open, since the outgoing
    // library stays composed during the portrait transition and would compete with the details for focus.
    LaunchedEffect(selection == null) { if (selection == null && !sheetOpen && !overlay) keys.requestFocus() }
    BoxWithConstraints(
        modifier
            .fillMaxSize()
            .background(c.background)
            .onPreviewKeyEvent { event ->
                val escape = event.type == KeyEventType.KeyDown && event.key == Key.Escape
                when {
                    sheetOpen -> escape.also { if (it) closeSheets() }
                    overlay -> false
                    selection != null -> detailsKeyAction(event, selection)?.also(onAction) != null
                    event.isCtrlK() -> true.also { onAction(Action.OpenSearch) }
                    // Escape leaves reorder mode first, as Back does.
                    escape && reordering -> true.also { onAction(Action.ReorderFinished) }
                    // Inside a set, Escape returns to the list of sets.
                    escape && inSet -> true.also { onAction(Action.SelectSection("sets")) }
                    else -> false
                }
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
                Header(snapshot, onAction, expanded, sidebar, portrait, inSet, reordering)
                if (!sidebar) Chips(snapshot, onAction)
                OperationBanner(snapshot.banner, onAction)
                snapshot.pdfOpenError?.let { Text(pdfErrorText(it), color = c.error, fontSize = 15.sp) }
                Content(
                    snapshot,
                    entries,
                    onAction,
                    reordering,
                    onMore = { menuFor = it.selectionId },
                    onNoteMore = { noteMenuFor = it.itemId },
                    onSetMore = { setMenuFor = it.id },
                )
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
        if (menuNote != null) {
            NoteActionsSheet(
                menuNote,
                onDismiss = { noteMenuFor = null },
                onEdit = {
                    noteMenuFor = null
                    noteEditFor = menuNote.itemId
                },
                onRemove = {
                    noteMenuFor = null
                    onAction(Action.RemoveSetItem(menuNote.itemId))
                },
            )
        }
        if (editNote != null) {
            NoteEditor(
                editNote,
                onCancel = { noteEditFor = null },
                onSave = {
                    noteEditFor = null
                    onAction(Action.EditNote(editNote.itemId, it))
                },
            )
        }
        if (menuSet != null) {
            SetActionsSheet(
                menuSet,
                onDismiss = { setMenuFor = null },
                onAction = {
                    setMenuFor = null
                    onAction(it)
                },
            )
        }
    }
}

@Composable
private fun Header(
    snapshot: Snapshot,
    onAction: (Action) -> Unit,
    expanded: Boolean,
    sidebar: Boolean,
    portrait: Boolean,
    inSet: Boolean,
    reordering: Boolean,
) {
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
        if (inSet) {
            IconButton(stringResource(R.string.set_back), painterResource(R.drawable.ic_arrow_back), { onAction(Action.SelectSection("sets")) })
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
            Text(if (reordering) stringResource(R.string.set_reorder_hint) else subtitleText(snapshot.subtitle), color = c.muted, fontSize = 15.sp)
            val notes = snapshot.setNotes
            if (inSet && !notes.isNullOrEmpty()) {
                Text(notes, color = c.foreground, fontSize = 15.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(stringResource(R.string.screen_search), painterResource(R.drawable.ic_search), { onAction(Action.OpenSearch) })
            // Reorder mode always shows the list; the saved choice comes back with Done.
            if (!reordering) {
                IconButton(
                    stringResource(if (list) R.string.library_grid_view else R.string.library_list_view),
                    painterResource(if (list) R.drawable.ic_layout_grid else R.drawable.ic_layout_list),
                    { onAction(Action.SetLibraryView(if (list) LibraryView.GRID else LibraryView.LIST)) },
                )
            }
            if (snapshot.sectionKey == "sets") {
                PillButton(stringResource(R.string.library_new_set), { onAction(Action.NewSet) }, icon = painterResource(R.drawable.ic_add), primary = true)
            }
            if (reordering) {
                PillButton(stringResource(R.string.picker_done), { onAction(Action.ReorderFinished) }, icon = painterResource(R.drawable.ic_check), primary = true)
            } else if (inSet) {
                // Under 840 dp (portrait, or the pane beside the details) the labelled icon leaves room for the set's name.
                if (expanded) {
                    PillButton(stringResource(R.string.set_reorder), { onAction(Action.ReorderStarted) }, icon = painterResource(R.drawable.ic_reorder))
                } else {
                    IconButton(stringResource(R.string.set_reorder), painterResource(R.drawable.ic_reorder), { onAction(Action.ReorderStarted) })
                }
                PillButton(stringResource(R.string.set_add), { onAction(Action.OpenAddPicker) }, icon = painterResource(R.drawable.ic_add), primary = true)
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

/** 0.3.0's `SidePanel`: the logo, the sections with counts, the composers when there are any, and Settings. */
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
private fun ColumnScope.Content(
    snapshot: Snapshot,
    entries: List<SetEntry>,
    onAction: (Action) -> Unit,
    reordering: Boolean,
    onMore: (com.c2yz.notae.core.Row) -> Unit,
    onNoteMore: (SetEntry.Note) -> Unit,
    onSetMore: (SetCard) -> Unit,
) {
    val area = Modifier.weight(1f).fillMaxWidth()
    val open = { row: com.c2yz.notae.core.Row -> onAction(Action.OpenPdf(row.relativePath, row.noteKey, row.itemId)) }
    // The score shown in the details pane beside the library.
    val selected = snapshot.selection?.row?.selectionId
    when {
        snapshot.sectionKey == "sets" && snapshot.setCards.isEmpty() -> EmptyState(snapshot, onAction, area)
        snapshot.sectionKey == "sets" -> SetsPane(
            snapshot.setCards,
            snapshot.layout.libraryView == LibraryView.LIST,
            { onAction(Action.OpenSet(it.id)) },
            onSetMore,
            area,
        )
        entries.isEmpty() -> EmptyState(snapshot, onAction, area)
        reordering -> ReorderList(entries, onAction, area)
        snapshot.layout.libraryView == LibraryView.LIST -> ScoreList(entries, open, onMore, onNoteMore, area, selected)
        else -> ScoreGrid(entries, open, onMore, onNoteMore, area, selected)
    }
}

/** The section's empty card: folder glyph, title and detail; All scores also offers Scan library. */
@Composable
private fun EmptyState(snapshot: Snapshot, onAction: (Action) -> Unit, modifier: Modifier) {
    val c = NotaeTheme.colors
    val key = snapshot.sectionKey
    val (title, detail) = when {
        key == "all" -> R.string.library_empty_all to R.string.library_empty_all_detail
        key == "recent" -> R.string.library_empty_recent to R.string.library_empty_recent_detail
        key == "favorites" -> R.string.library_empty_favorites to R.string.library_empty_favorites_detail
        key == "sets" -> R.string.library_empty_sets to R.string.library_empty_sets_detail
        key.startsWith(SET_PREFIX) -> R.string.set_empty to R.string.set_empty_detail
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

/** Ctrl K, which opens search from the library and the details (0.3.0's root shortcut scope). */
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

/** "N scores", "N sets · recently edited first", or the open set's date and counts. */
@Composable
private fun subtitleText(subtitle: Subtitle): String = when (subtitle) {
    is Subtitle.Scores -> pluralStringResource(R.plurals.library_scores, subtitle.count.toCount(), subtitle.count.toLong())
    is Subtitle.Sets -> pluralStringResource(R.plurals.library_sets_subtitle, subtitle.count.toCount(), subtitle.count.toLong())
    is Subtitle.Set -> setSubtitle(subtitle)
}

/** Why the PDF could not be opened, in the English it has always carried (texts.rs `viewer_error`). */
@Composable
internal fun pdfErrorText(error: PdfOpenError): String = when (error) {
    PdfOpenError.InvalidContentUri -> stringResource(R.string.viewer_error_invalid_uri)
    PdfOpenError.NoCompatibleViewer -> stringResource(R.string.viewer_error_no_viewer)
    is PdfOpenError.Platform -> stringResource(R.string.viewer_error_platform, error.detail)
}
