package com.c2yz.notae.ui.library

import com.c2yz.notae.ui.components.ClaimBack
import androidx.compose.animation.Crossfade
import com.c2yz.notae.ui.components.collapsingGaps
import com.c2yz.notae.ui.components.AnimatedPresence
import com.c2yz.notae.ui.theme.NotaeMotion
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.fadeOut
import androidx.compose.animation.fadeIn
import androidx.compose.animation.expandVertically
import com.c2yz.notae.ui.components.backProgress
import com.c2yz.notae.ui.components.LocalBackProgress
import com.c2yz.notae.ui.components.AnimatedOverlay
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
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
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
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
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.updateTransition
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.navigationevent.NavigationEventInfo
import androidx.navigationevent.compose.NavigationBackHandler
import androidx.navigationevent.compose.rememberNavigationEventState
import com.c2yz.notae.R
import com.c2yz.notae.nav.screenBackground
import com.c2yz.notae.nav.screenBar
import com.c2yz.notae.nav.screenFade
import com.c2yz.notae.nav.screenContent
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.LibraryView
import com.c2yz.notae.core.PdfOpenError
import com.c2yz.notae.core.SectionTitle
import com.c2yz.notae.core.SetCard
import com.c2yz.notae.core.SetEntry
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.core.Subtitle
import com.c2yz.notae.ui.components.EXPANDED_WIDTH
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.components.NotaeCard
import com.c2yz.notae.ui.components.OperationBanner
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.components.SidebarItem
import com.c2yz.notae.ui.components.Chrome
import com.c2yz.notae.ui.components.toCount
import com.c2yz.notae.ui.components.windowChrome
import com.c2yz.notae.ui.details.detailsKeyAction
import com.c2yz.notae.ui.sets.SetActionsSheet
import com.c2yz.notae.ui.sets.ShareSetButton
import com.c2yz.notae.ui.sets.IncludeDrawingsItem
import com.c2yz.notae.ui.sets.setCanReorder
import com.c2yz.notae.ui.sets.setCanShare
import com.c2yz.notae.ui.sets.setHasDrawings
import com.c2yz.notae.ui.sets.overlayOpen
import com.c2yz.notae.ui.sets.SetsPane
import com.c2yz.notae.ui.sets.setSubtitle
import com.c2yz.notae.ui.theme.NotaeTheme

internal const val SET_PREFIX = "set:"

internal const val SIDE_PANEL_TAG = "side-panel"

/** The side panel's slide in or out: a little under a screen's, as the panel crosses less of the window. */
internal const val PANEL_MILLIS = 250

/** Each half of the rail's fade through as the side panel hides or shows: the rail out, then the panel in, or back. */
internal const val RAIL_FADE_MILLIS = 150

internal const val RAIL_TAG = "rail"

private val SIDE_PANEL_WIDTH = 288.dp

/** The side panel and the gap after it. */
private val PANEL_SPAN = SIDE_PANEL_WIDTH + 16.dp

/** The rail that stands in for the side panel, and the gap after it. */
private val RAIL_SPAN = RAIL_WIDTH + 8.dp

/**
 * 0.3.0's `LibraryScreen` on a tablet: the side panel (or the rail when it is hidden or the pane is under 840 dp), the large
 * title with its header actions, the composer filter on All scores and a composer's scores, the operation banner, the PDF launch error, and the scores as a grid or a list, the
 * sets as folder cards or rows, or the section's empty state. Inside a set the header gains Back to sets, the set's
 * subtitle and notes, and Add. The ⋮ sheets are the screen's own state, as in 0.3.0; Back and Escape close them first.
 * A phone's window gets [CompactLibrary] and one between a phone's and a tablet's [MediumLibrary] around the same content.
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
    // A piece's set note editor, by item id.
    var setNoteFor by remember { mutableStateOf<String?>(null) }
    // The composer sheet stays open when the window turns (the bottom bar's chip and the slim layout's filter open it).
    var composerSheet by rememberSaveable { mutableStateOf(false) }
    val setNoteRow = setNoteFor?.let { id -> snapshot.rows.firstOrNull { it.itemId == id } }
    val sheetOpen = menuRow != null || menuSet != null || menuNote != null || editNote != null || setNoteRow != null
    val closeSheets = {
        menuFor = null
        setMenuFor = null
        noteMenuFor = null
        noteEditFor = null
        setNoteFor = null
    }
    val backState = rememberNavigationEventState(NavigationEventInfo.None)
    // A core dialog drawn over the sheet (a set file arriving, say) takes Back first.
    val sheetTakesBack = sheetOpen && !overlayOpen(snapshot)
    NavigationBackHandler(
        state = backState,
        isBackEnabled = sheetTakesBack,
        onBackCompleted = closeSheets,
    )
    ClaimBack(sheetTakesBack)
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
            .screenBackground(c.background)
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
        val expanded = maxWidth >= EXPANDED_WIDTH
        val portrait = maxHeight > maxWidth
        val layout = snapshot.layout
        val collapsed = if (portrait) layout.sidebarCollapsedPortrait else layout.sidebarCollapsedLandscape
        val sidebar = expanded && !collapsed
        val content: @Composable ColumnScope.() -> Unit = {
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
        // The window decides the chrome: a phone gets the bottom bar, a short window (a phone on its side) the slim
        // rail, a window between a phone and a tablet the rail, and a tablet the side panel (or the rail when it is
        // hidden or the library shares the window with the details).
        when (windowChrome()) {
            Chrome.BOTTOM_BAR -> CompactLibrary(snapshot, onAction, inSet, reordering, composerSheet, { composerSheet = it }, content)
            Chrome.SLIM_RAIL -> SlimLibrary(
                snapshot,
                onAction,
                { Header(snapshot, onAction, expanded = false, sidebar = true, portrait, inSet, reordering) },
                inSet,
                composerSheet,
                { composerSheet = it },
                content,
            )
            Chrome.RAIL -> MediumLibrary(
                snapshot,
                onAction,
                { Header(snapshot, onAction, expanded = false, sidebar = true, portrait, inSet, reordering) },
                inSet,
                composerSheet,
                { composerSheet = it },
                content,
            )
            Chrome.SIDE_PANEL -> Box(Modifier.fillMaxSize().clipToBounds().padding(16.dp)) {
                // The side panel slides in from the start over the rail, or out to it, and the rest moves with it. The rest
                // takes its new width at once and is only moved, so its grid changes its columns once, not at every step.
                val panel = updateTransition(sidebar, label = "side panel")
                val shown by panel.animateFloat({ tween(PANEL_MILLIS, easing = NotaeMotion.emphasized) }, label = "side panel shown") {
                    if (it) 1f else 0f
                }
                // The rail fades through, linear in time like a screen's bars: going, it fades out over the first half;
                // coming, it fades in over the second half, as the panel has gone.
                val railTime = panel.animateFloat({ tween(2 * RAIL_FADE_MILLIS, easing = LinearEasing) }, label = "rail") {
                    if (it) 0f else 1f
                }
                val railGoing by remember { derivedStateOf { railTime.value > 0.5f } }
                val toEnd = if (LocalLayoutDirection.current == LayoutDirection.Rtl) -1f else 1f
                if (!sidebar || railGoing) {
                    NavRail(
                        snapshot,
                        onAction,
                        Modifier
                            .width(RAIL_WIDTH)
                            .fillMaxHeight()
                            .graphicsLayer { alpha = ((railTime.value - 0.5f) * 2f).coerceIn(0f, 1f) }
                            .testTag(RAIL_TAG)
                            .screenBar("rail"),
                    )
                }
                if (panel.currentState || panel.targetState) {
                    SidePanel(
                        snapshot,
                        onAction,
                        Modifier
                            .width(SIDE_PANEL_WIDTH)
                            .fillMaxHeight()
                            .graphicsLayer { translationX = (shown - 1f) * PANEL_SPAN.toPx() * toEnd }
                            .testTag(SIDE_PANEL_TAG)
                            .screenBar("sidePanel"),
                    )
                }
                Column(
                    Modifier
                        .fillMaxSize()
                        .padding(start = if (sidebar) PANEL_SPAN else RAIL_SPAN)
                        .graphicsLayer { translationX = (shown - if (sidebar) 1f else 0f) * (PANEL_SPAN - RAIL_SPAN).toPx() * toEnd },
                    verticalArrangement = collapsingGaps(12.dp),
                ) {
                    // Each bar on its own, so the column keeps its children (its gaps count them).
                    Header(snapshot, onAction, expanded, sidebar, portrait, inSet, reordering)
                    if (showsComposers(snapshot, inSet)) {
                        Box(Modifier.screenBar("composers")) { ComposerChip(composerName(snapshot), { composerSheet = true }) }
                    }
                    OperationBanner(snapshot.banner, onAction, Modifier.screenBar("banner"))
                    PdfError(snapshot.pdfOpenError, Modifier.screenBar("pdfError"))
                    content()
                }
            }
        }
        // Over the whole window, not inside the side panel's margin.
        if (windowChrome() == Chrome.SIDE_PANEL) ComposerSheetOverlay(composerSheet, snapshot, onAction) { composerSheet = it }
        CompositionLocalProvider(LocalBackProgress provides remember(backState) { backProgress(backState) }) {
            AnimatedOverlay(menuRow) { row ->
                ScoreActionsSheet(
                    row,
                    openScoreIcon(snapshot.layout.openScoresIn),
                    onDismiss = { menuFor = null },
                    onAction = {
                        menuFor = null
                        onAction(it)
                    },
                    onEditSetNote = {
                        menuFor = null
                        setNoteFor = row.itemId
                    },
                )
            }
            AnimatedOverlay(setNoteRow) { row ->
                NoteEditor(
                    stringResource(R.string.set_note_in, (snapshot.title as? SectionTitle.Set)?.name.orEmpty()),
                    row.setNote.orEmpty(),
                    onCancel = { setNoteFor = null },
                    allowBlank = true,
                    onSave = { text ->
                        setNoteFor = null
                        row.itemId?.let { onAction(Action.SaveSetNote(it, text)) }
                    },
                )
            }
            AnimatedOverlay(menuNote) { note ->
                NoteActionsSheet(
                    note,
                    onDismiss = { noteMenuFor = null },
                    onEdit = {
                        noteMenuFor = null
                        noteEditFor = note.itemId
                    },
                    onRemove = {
                        noteMenuFor = null
                        onAction(Action.RemoveSetItem(note.itemId))
                    },
                )
            }
            AnimatedOverlay(editNote) { note ->
                NoteEditor(
                    stringResource(R.string.note_edit),
                    note.text,
                    onCancel = { noteEditFor = null },
                    onSave = {
                        noteEditFor = null
                        onAction(Action.EditNote(note.itemId, it))
                    },
                )
            }
            AnimatedOverlay(menuSet) { set ->
                SetActionsSheet(
                    set,
                    onDismiss = { setMenuFor = null },
                    onAction = {
                        setMenuFor = null
                        onAction(it)
                    },
                )
            }
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
    val slim = windowChrome() == Chrome.SLIM_RAIL
    val setId = snapshot.sectionKey.removePrefix(SET_PREFIX)
    val hasDrawings = setHasDrawings(snapshot, setId)
    var includeDrawings by remember(setId) { mutableStateOf(false) }
    val viewLabel = stringResource(if (list) R.string.library_grid_view else R.string.library_list_view)
    val viewIcon = if (list) R.drawable.ic_layout_grid else R.drawable.ic_layout_list
    val viewAction = Action.SetLibraryView(if (list) LibraryView.GRID else LibraryView.LIST)
    val reorderLabel = stringResource(R.string.set_reorder)
    // An empty set has nothing to order or share; one item has no order to change.
    val canReorder = setCanReorder(snapshot)
    val canShare = setCanShare(snapshot)
    // R2: the actions that do not fit beside the title move into ⋮: Share first, then the view choice and Reorder;
    // Back, Search and the side-panel toggle stay, and so do New set, Add and Done unless nothing else fits.
    val leading = buildList {
        if (expanded) {
            add(
                HeaderItem("sidebarToggle", priority = 9, pinned = true, {
                    IconButton(
                        stringResource(if (sidebar) R.string.library_hide_sidebar else R.string.library_show_sidebar),
                        painterResource(if (sidebar) R.drawable.ic_panel_left_close else R.drawable.ic_panel_left_open),
                        { onAction(Action.SetSidebarCollapsed(portrait = portrait, collapsed = sidebar)) },
                        Modifier.screenBar("sidebarToggle"),
                    )
                }),
            )
        }
        if (inSet) {
            add(
                HeaderItem("setBack", priority = 9, pinned = true, {
                    IconButton(stringResource(R.string.set_back), painterResource(R.drawable.ic_arrow_back), { onAction(Action.SelectSection("sets")) }, Modifier.screenBar("setBack"))
                }),
            )
        }
    }
    val trailing = buildList {
        add(
            HeaderItem("search", priority = 9, pinned = true, {
                IconButton(stringResource(R.string.screen_search), painterResource(R.drawable.ic_search), { onAction(Action.OpenSearch) }, Modifier.screenBar("search"))
            }),
        )
        // Reorder mode always shows the list; the saved choice comes back with Done.
        if (!reordering) {
            add(
                HeaderItem("view", priority = 2, pinned = false, {
                    IconButton(viewLabel, painterResource(viewIcon), { onAction(viewAction) }, Modifier.screenBar("view"))
                }) { choose -> MenuItem(viewLabel, painterResource(viewIcon), { choose(viewAction) }) },
            )
        }
        if (snapshot.sectionKey == "sets") {
            val label = stringResource(R.string.library_new_set)
            add(
                HeaderItem("newSet", priority = 5, pinned = false, {
                    PillButton(label, { onAction(Action.NewSet) }, Modifier.screenBar("newSet"), icon = painterResource(R.drawable.ic_add), primary = true)
                }) { choose -> MenuItem(label, painterResource(R.drawable.ic_add), { choose(Action.NewSet) }) },
            )
        }
        if (reordering) {
            val label = stringResource(R.string.picker_done)
            add(
                HeaderItem("done", priority = 5, pinned = false, {
                    PillButton(label, { onAction(Action.ReorderFinished) }, Modifier.screenBar("done"), icon = painterResource(R.drawable.ic_check), primary = true)
                }) { choose -> MenuItem(label, painterResource(R.drawable.ic_check), { choose(Action.ReorderFinished) }) },
            )
        } else if (inSet) {
            add(
                HeaderItem("reorder", priority = 3, pinned = false, {
                    // Under 840 dp (portrait, or the pane beside the details) the labelled icon leaves room for the set's name.
                    if (expanded) {
                        PillButton(reorderLabel, { onAction(Action.ReorderStarted) }, Modifier.screenBar("reorder"), icon = painterResource(R.drawable.ic_reorder), enabled = canReorder)
                    } else {
                        IconButton(reorderLabel, painterResource(R.drawable.ic_reorder), { onAction(Action.ReorderStarted) }, Modifier.screenBar("reorder"), enabled = canReorder)
                    }
                }) { choose -> MenuItem(reorderLabel, painterResource(R.drawable.ic_reorder), { choose(Action.ReorderStarted) }, enabled = canReorder) },
            )
            add(
                HeaderItem("share", priority = 1, pinned = false, {
                    Box(Modifier.screenBar("share")) { ShareSetButton(setId, hasDrawings, onAction, enabled = canShare) }
                }) { choose ->
                    if (hasDrawings) IncludeDrawingsItem(includeDrawings) { includeDrawings = !includeDrawings }
                    MenuItem(stringResource(R.string.share_set_plain), painterResource(R.drawable.ic_share), { choose(Action.ShareSet(setId, false, hasDrawings && includeDrawings)) }, enabled = canShare)
                    MenuItem(stringResource(R.string.share_set_with_notes), painterResource(R.drawable.ic_note), { choose(Action.ShareSet(setId, true, hasDrawings && includeDrawings)) }, enabled = canShare)
                },
            )
            val label = stringResource(R.string.set_add)
            add(
                HeaderItem("add", priority = 5, pinned = false, {
                    PillButton(label, { onAction(Action.OpenAddPicker) }, Modifier.screenBar("add"), icon = painterResource(R.drawable.ic_add), primary = true)
                }) { choose -> MenuItem(label, painterResource(R.drawable.ic_add), { choose(Action.OpenAddPicker) }) },
            )
        }
    }
    HeaderRow(leading, trailing, onAction, Modifier.fillMaxWidth().padding(top = 8.dp, start = 8.dp)) {
        // The title changes with the section, so it fades through; the controls that stay put do not move.
        Column(Modifier.screenFade(), verticalArrangement = Arrangement.spacedBy(if (slim) 2.dp else 4.dp)) {
            Text(
                sectionTitle(snapshot.title),
                Modifier.fillMaxWidth().semantics { heading() },
                color = c.foreground,
                fontSize = when {
                    expanded -> 40.sp
                    slim -> 24.sp
                    else -> 30.sp
                },
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            val notes = snapshot.setNotes?.takeIf { inSet && it.isNotEmpty() }
            // A short window has room for one line under the title: the set's notes win over its date and counts, and
            // the reorder hint wins over the notes.
            if (reordering || !(slim && notes != null)) {
                Text(
                    if (reordering) stringResource(R.string.set_reorder_hint) else subtitleText(snapshot.subtitle),
                    color = c.muted,
                    fontSize = 15.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (notes != null && !(slim && reordering)) {
                Text(notes, color = c.foreground, fontSize = 15.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/** 0.3.0's `SidePanel`: the logo, the sections with counts, and Settings. */
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
                        selected = navSelected(section.key, snapshot.sectionKey),
                    )
                }
            }
        }
        Spacer(Modifier.weight(1f))
        SidebarItem(stringResource(R.string.screen_settings), { onAction(Action.OpenSettings) }, icon = painterResource(R.drawable.ic_settings))
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
    val area = Modifier.fillMaxSize()
    val open = { row: com.c2yz.notae.core.Row -> onAction(Action.OpenPdf(row.relativePath, row.noteKey, row.itemId)) }
    // The score shown in the details pane beside the library.
    val selected = snapshot.selection?.row?.selectionId
    val list = snapshot.layout.libraryView == LibraryView.LIST
    val sets = snapshot.sectionKey == "sets"
    val mode = when {
        sets && snapshot.setCards.isEmpty() -> ContentMode.EMPTY
        sets -> ContentMode.SETS
        entries.isEmpty() -> ContentMode.EMPTY
        reordering -> ContentMode.REORDER
        list -> ContentMode.LIST
        else -> ContentMode.GRID
    }
    // Grid and list, reorder mode and an emptied section fade into each other instead of swapping in one frame.
    // The part of the library that slides when the section or the screen changes.
    Crossfade(mode, Modifier.weight(1f).fillMaxWidth().screenContent(NotaeTheme.colors.background), NotaeMotion.short(), label = "library content") { shown ->
        when (shown) {
            ContentMode.EMPTY -> EmptyState(snapshot, onAction, area)
            ContentMode.SETS -> SetsPane(snapshot.setCards, list, { onAction(Action.OpenSet(it.id)) }, onSetMore, area)
            ContentMode.REORDER -> ReorderList(entries, onAction, area)
            ContentMode.LIST -> ScoreList(entries, open, onMore, onNoteMore, area, selected)
            ContentMode.GRID -> ScoreGrid(entries, open, onMore, onNoteMore, area, selected)
        }
    }
}

private enum class ContentMode { EMPTY, SETS, REORDER, LIST, GRID }

/**
 * The section's empty card: glyph, title and detail, and the one action that fills the section: Scan library on All
 * scores, New set on Sets, Add pieces in a set. While a scan runs, All scores says so instead of asking for one.
 */
@Composable
private fun EmptyState(snapshot: Snapshot, onAction: (Action) -> Unit, modifier: Modifier) {
    val c = NotaeTheme.colors
    val key = snapshot.sectionKey
    val scanning = key == "all" && snapshot.banner.busy
    val (title, detail) = when {
        scanning -> R.string.library_scanning to R.string.library_scanning_detail
        key == "all" -> R.string.library_empty_all to R.string.library_empty_all_detail
        key == "recent" -> R.string.library_empty_recent to R.string.library_empty_recent_detail
        key == "favorites" -> R.string.library_empty_favorites to R.string.library_empty_favorites_detail
        key == "sets" -> R.string.library_empty_sets to R.string.library_empty_sets_detail
        key.startsWith(SET_PREFIX) -> R.string.set_empty to R.string.set_empty_detail
        else -> R.string.library_empty_composer to R.string.library_empty_composer_detail
    }
    val button: Triple<Int, Int, Action>? = when {
        scanning -> null
        key == "all" -> Triple(R.string.library_scan, R.drawable.ic_scan, Action.Rescan)
        key == "sets" -> Triple(R.string.library_new_set, R.drawable.ic_add, Action.NewSet)
        key.startsWith(SET_PREFIX) -> Triple(R.string.set_add_pieces, R.drawable.ic_add, Action.OpenAddPicker)
        else -> null
    }
    NotaeCard(modifier) {
        Column(
            Modifier.fillMaxSize().padding(32.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterVertically),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Icon(painterResource(if (scanning) R.drawable.ic_scan else R.drawable.ic_folder), null, Modifier.size(56.dp), tint = c.muted)
            Text(stringResource(title), color = c.foreground, fontSize = 22.sp, fontWeight = FontWeight.SemiBold, textAlign = TextAlign.Center)
            Text(stringResource(detail), color = c.muted, fontSize = 15.sp, textAlign = TextAlign.Center)
            button?.let { (label, icon, action) ->
                PillButton(
                    stringResource(label),
                    { onAction(action) },
                    Modifier.padding(top = 8.dp),
                    icon = painterResource(icon),
                    primary = true,
                    enabled = action != Action.Rescan || snapshot.rescanEnabled,
                )
            }
        }
    }
}

/** Ctrl K, which opens search from the library and the details (0.3.0's root shortcut scope). */
internal fun KeyEvent.isCtrlK(): Boolean = type == KeyEventType.KeyDown && isCtrlPressed && key == Key.K

/** A side-panel row is selected for its own section; the Sets row also inside a set. */
internal fun sectionIcon(key: String): Int = when (key) {
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
internal fun subtitleText(subtitle: Subtitle): String = when (subtitle) {
    is Subtitle.Scores -> pluralStringResource(R.plurals.library_scores, subtitle.count.toCount(), subtitle.count.toLong())
    is Subtitle.Sets -> pluralStringResource(R.plurals.library_sets_subtitle, subtitle.count.toCount(), subtitle.count.toLong())
    is Subtitle.Set -> setSubtitle(subtitle)
}

/** The PDF launch error under the banner, opening and closing in place as the banner does. */
@Composable
internal fun PdfError(error: PdfOpenError?, modifier: Modifier = Modifier) {
    AnimatedPresence(
        error,
        expandVertically(NotaeMotion.medium()) + fadeIn(NotaeMotion.medium()),
        shrinkVertically(NotaeMotion.medium()) + fadeOut(NotaeMotion.short()),
        modifier,
    ) { Text(pdfErrorText(it), color = NotaeTheme.colors.error, fontSize = 15.sp) }
}

/** Why the PDF could not be opened, in the English it has always carried (texts.rs `viewer_error`). */
@Composable
internal fun pdfErrorText(error: PdfOpenError): String = when (error) {
    PdfOpenError.InvalidContentUri -> stringResource(R.string.viewer_error_invalid_uri)
    PdfOpenError.NoCompatibleViewer -> stringResource(R.string.viewer_error_no_viewer)
    is PdfOpenError.Platform -> stringResource(R.string.viewer_error_platform, error.detail)
}
