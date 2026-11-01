package com.c2yz.notae.ui.search

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Row as ScoreRowData
import com.c2yz.notae.core.SearchScope
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.core.Snippet
import com.c2yz.notae.ui.components.Chip
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.components.NotaeCard
import com.c2yz.notae.ui.components.OperationBanner
import com.c2yz.notae.ui.components.SearchField
import com.c2yz.notae.ui.library.ListScrollbar
import com.c2yz.notae.ui.library.PagePreview
import com.c2yz.notae.ui.library.ScoreSubtitle
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * 0.3.0's `SearchScreen`, the app's one search: back, the search field (focused, with the keyboard, on open) and the
 * microphone when a speech recognizer exists ([voiceAvailable]); the operation banner; the scope chips; then the
 * results, "No scores found", or the empty prompt. Every edit and chip goes to the core, which runs the search;
 * a result closes search and opens its details. Escape closes search, as Back does.
 */
@Composable
fun SearchScreen(snapshot: Snapshot, onAction: (Action) -> Unit, voiceAvailable: Boolean, modifier: Modifier = Modifier) {
    val c = NotaeTheme.colors
    val field = remember { FocusRequester() }
    val keyboard = LocalSoftwareKeyboardController.current
    LaunchedEffect(Unit) {
        field.requestFocus()
        keyboard?.show()
    }
    BoxWithConstraints(
        modifier
            .fillMaxSize()
            .background(c.background)
            .onPreviewKeyEvent { event ->
                (event.type == KeyEventType.KeyDown && event.key == Key.Escape).also { if (it) onAction(Action.CloseSearch) }
            },
    ) {
        val side = if (maxWidth < 600.dp) 16.dp else 24.dp
        Column(
            Modifier.fillMaxSize().padding(start = side, top = side, end = side, bottom = 16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                IconButton(stringResource(R.string.details_back), painterResource(R.drawable.ic_arrow_back), { onAction(Action.CloseSearch) })
                SearchField(
                    snapshot.search.query,
                    { onAction(Action.SearchChanged(it)) },
                    stringResource(R.string.search_placeholder),
                    field,
                    Modifier.weight(1f),
                )
                if (voiceAvailable) {
                    IconButton(stringResource(R.string.search_voice), painterResource(R.drawable.ic_mic), { onAction(Action.VoiceSearch) })
                }
            }
            OperationBanner(snapshot.banner, onAction)
            ScopeChips(snapshot.search.scope, onAction)
            Results(snapshot, onAction)
        }
    }
}

@Composable
private fun ScopeChips(selected: SearchScope, onAction: (Action) -> Unit) {
    Row(
        Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(start = 56.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        SCOPES.forEach { (scope, label) ->
            Chip(stringResource(label), scope == selected, { onAction(Action.SearchScopeChanged(scope)) })
        }
    }
}

@Composable
private fun ColumnScope.Results(snapshot: Snapshot, onAction: (Action) -> Unit) {
    val area = Modifier.weight(1f).fillMaxWidth()
    when {
        snapshot.search.query.isEmpty() -> Message(R.string.search_empty_title, R.string.search_empty_detail, area, glyph = true)
        snapshot.rows.isEmpty() -> Message(R.string.search_no_results_title, R.string.search_no_results_detail, area, glyph = false)
        else -> NotaeCard(area) {
            val state = rememberLazyListState()
            LazyColumn(Modifier.fillMaxSize().padding(8.dp), state = state) {
                items(snapshot.rows, key = { it.selectionId }) { row ->
                    ResultRow(row) { onAction(Action.SearchResultChosen(row.noteKey)) }
                }
            }
            ListScrollbar(state, Modifier.align(Alignment.TopEnd).padding(vertical = 8.dp))
        }
    }
}

/** The empty prompt (with the search glyph) or "No scores found", centred in the rest of the screen. */
@Composable
private fun Message(title: Int, detail: Int, modifier: Modifier, glyph: Boolean) {
    val c = NotaeTheme.colors
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterVertically), horizontalAlignment = Alignment.CenterHorizontally) {
        if (glyph) Icon(painterResource(R.drawable.ic_search), null, Modifier.size(40.dp), tint = c.muted)
        Text(stringResource(title), color = c.foreground, fontSize = 20.sp, fontWeight = FontWeight.SemiBold, textAlign = TextAlign.Center)
        Text(stringResource(detail), color = c.muted, fontSize = 15.sp, textAlign = TextAlign.Center)
    }
}

/**
 * 0.3.0's `ResultRow`: the small page, the title and the core's snippet with the match in bold accent text (88 dp), or
 * "number · composer" when the hit is not in the text (76 dp). Named by its title, read with its snippet.
 */
@Composable
private fun ResultRow(row: ScoreRowData, onClick: () -> Unit) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(18.dp)
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val focused by source.collectIsFocusedAsState()
    val snippet = row.snippet?.takeIf { it.hit.isNotEmpty() }
    Row(
        Modifier
            .fillMaxWidth()
            .height(if (snippet != null) 88.dp else 76.dp)
            .clip(shape)
            .background(if (pressed) c.field else Color.Transparent)
            .then(if (focused) Modifier.border(BorderStroke(2.dp, c.accent), shape) else Modifier)
            .semantics { contentDescription = row.title }
            .clickable(source, null, role = Role.Button, onClick = onClick)
            .padding(horizontal = 14.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        PagePreview(row.thumbnail, Modifier.size(40.dp, 52.dp), radius = 6.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(
                row.title,
                Modifier.clearAndSetSemantics { },
                color = c.foreground,
                fontSize = 16.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (snippet != null) SnippetLine(snippet) else ScoreSubtitle(row, 14)
        }
    }
}

@Composable
private fun SnippetLine(snippet: Snippet) {
    val c = NotaeTheme.colors
    val text = buildAnnotatedString {
        append(snippet.before)
        withStyle(SpanStyle(color = c.accentText, fontWeight = FontWeight.Bold)) { append(snippet.hit) }
        append(snippet.after)
    }
    Text(text, color = c.muted, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
}

/** The chips in 0.3.0's order: All, Titles, Composers, Lyrics, Tags. */
private val SCOPES = listOf(
    SearchScope.EVERYTHING to R.string.search_scope_all,
    SearchScope.TITLES to R.string.search_scope_titles,
    SearchScope.COMPOSERS to R.string.search_scope_composers,
    SearchScope.TEXT to R.string.search_scope_lyrics,
    SearchScope.TAGS to R.string.search_scope_tags,
)
