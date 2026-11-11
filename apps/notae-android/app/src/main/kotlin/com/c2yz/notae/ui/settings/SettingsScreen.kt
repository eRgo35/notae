package com.c2yz.notae.ui.settings

import com.c2yz.notae.nav.screenBackground
import com.c2yz.notae.nav.screenChrome
import com.c2yz.notae.nav.screenContent
import android.content.res.Resources
import com.c2yz.notae.ui.components.collapsingGaps
import com.c2yz.notae.ui.theme.NotaeMotion
import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.ExportStatus
import com.c2yz.notae.core.ImportConfirmation
import com.c2yz.notae.core.ImportError
import com.c2yz.notae.core.ImportStatus
import com.c2yz.notae.core.SettingsInfo
import com.c2yz.notae.core.SharedSetReview
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.platform.appLanguage
import com.c2yz.notae.platform.setAppLanguage
import com.c2yz.notae.ui.components.COMPACT_WIDTH
import com.c2yz.notae.ui.components.DialogButtons
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.components.ModalSurface
import com.c2yz.notae.ui.components.NotaeCard
import com.c2yz.notae.ui.components.OperationBanner
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.library.ListScrollbar
import com.c2yz.notae.ui.library.isCtrlK
import com.c2yz.notae.ui.sets.closeOnEscape
import com.c2yz.notae.ui.sets.overlayOpen
import com.c2yz.notae.ui.theme.NotaeTheme

/** The build's version name and git identity, for the About card. */
data class AboutInfo(val version: String, val build: String)

/**
 * 0.3.0's `SettingsScreen`: back and "Settings", the operation banner, then a scrolling column (at most 720 dp,
 * centred) of cards: Library folder with Change, Library maintenance, About Notae ([about]), Language and Sets and
 * favorites with Export sets and its outcome. The language is Android's per-app language (AppCompat), not the core's.
 * The screen has the keys as soon as it shows, and again when the import confirmation closes: Escape closes it, Ctrl K
 * asks for search, as 0.3.0's root shortcuts do.
 */
@Composable
fun SettingsScreen(snapshot: Snapshot, onAction: (Action) -> Unit, about: AboutInfo, modifier: Modifier = Modifier) {
    val c = NotaeTheme.colors
    val keys = remember { FocusRequester() }
    // The import confirmation over Settings keeps the focus while open, and hands it back when it closes.
    val overlay = overlayOpen(snapshot)
    LaunchedEffect(overlay) { if (!overlay) keys.requestFocus() }
    BoxWithConstraints(
        modifier
            .fillMaxSize()
            .screenBackground(c.background)
            .onPreviewKeyEvent { event ->
                val action = when {
                    event.isCtrlK() -> Action.OpenSearch
                    event.type == KeyEventType.KeyDown && event.key == Key.Escape -> Action.CloseSettings
                    else -> null
                }
                action?.let(onAction)
                action != null
            }
            .focusRequester(keys)
            .focusable(),
    ) {
        val narrow = maxWidth < 420.dp
        Column(Modifier.fillMaxSize().padding(start = 16.dp, top = 16.dp, end = 16.dp), verticalArrangement = collapsingGaps(12.dp)) {
            Row(Modifier.screenChrome(), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                IconButton(stringResource(R.string.details_back), painterResource(R.drawable.ic_arrow_back), { onAction(Action.CloseSettings) })
                Text(stringResource(R.string.screen_settings), color = c.foreground, fontSize = if (narrow) 26.sp else 32.sp, fontWeight = FontWeight.SemiBold)
            }
            OperationBanner(snapshot.banner, onAction, Modifier.screenChrome())
            Box(Modifier.weight(1f).fillMaxWidth().screenContent(c.background)) {
                val state = rememberLazyListState()
                LazyColumn(
                    Modifier.fillMaxSize(),
                    state = state,
                    contentPadding = PaddingValues(bottom = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    val card = Modifier.widthIn(max = 720.dp).fillMaxWidth()
                    item { LibraryFolderCard(snapshot.settings.libraryName.orEmpty(), onAction, card) }
                    item { MaintenanceCard(snapshot.settings, card) }
                    item { AboutCard(about, card) }
                    item { LanguageCard(card) }
                    item { SetsCard(snapshot.settings, onAction, card) }
                }
                ListScrollbar(state, Modifier.align(Alignment.TopEnd))
            }
        }
    }
}

@Composable
private fun SettingsCard(modifier: Modifier, spacing: Int = 8, content: @Composable ColumnScope.() -> Unit) {
    NotaeCard(modifier) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 16.dp), verticalArrangement = Arrangement.spacedBy(spacing.dp), content = content)
    }
}

@Composable
private fun CardTitle(text: String) {
    // One UI names a settings group in the accent colour.
    Text(text, color = NotaeTheme.colors.accentText, fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
}

@Composable
private fun LibraryFolderCard(name: String, onAction: (Action) -> Unit, modifier: Modifier) {
    val c = NotaeTheme.colors
    SettingsCard(modifier, spacing = 4) {
        Text(stringResource(R.string.settings_library_folder), color = c.muted, fontSize = 13.sp)
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(name, Modifier.weight(1f), color = c.foreground, fontSize = 15.sp)
            PillButton(stringResource(R.string.settings_change), { onAction(Action.PickLibraryRoot) }, tonal = true)
        }
    }
}

@Composable
private fun MaintenanceCard(settings: SettingsInfo, modifier: Modifier) {
    SettingsCard(modifier) {
        CardTitle(stringResource(R.string.settings_maintenance))
        FactRow(stringResource(R.string.settings_indexed), settings.indexedCount.toString())
        FactRow(stringResource(R.string.settings_no_text), settings.noTextCount.toString())
        val scan = settings.latestScan
        val summary = if (scan == null) {
            stringResource(R.string.settings_no_scan)
        } else {
            stringResource(R.string.settings_scan_summary, scan.added.toLong(), scan.updated.toLong(), scan.moved.toLong(), scan.deleted.toLong())
        }
        FactRow(stringResource(R.string.settings_last_scan), summary)
        if (scan != null) {
            FactRow(stringResource(R.string.settings_conflicts), scan.conflicts.toString(), alarm = scan.conflicts != 0u)
            FactRow(stringResource(R.string.settings_failures), scan.failures.toString(), alarm = scan.failures != 0u)
        }
    }
}

@Composable
private fun AboutCard(about: AboutInfo, modifier: Modifier) {
    SettingsCard(modifier) {
        CardTitle(stringResource(R.string.settings_about))
        FactRow(stringResource(R.string.settings_version), about.version)
        FactRow(stringResource(R.string.settings_build), about.build)
        FactRow(stringResource(R.string.settings_license), stringResource(R.string.settings_license_name))
        FactRow(stringResource(R.string.settings_repository), stringResource(R.string.settings_repository_url))
        Text(stringResource(R.string.settings_local_note), color = NotaeTheme.colors.muted, fontSize = 14.sp)
    }
}

/**
 * System default, English and Polski, the current one checked. A choice goes straight to AppCompat, which switches
 * the whole UI and keeps it (on API 33+ as Android's per-app language). The check follows the configuration, so a
 * change made in Android's own settings shows too.
 */
@Composable
private fun LanguageCard(modifier: Modifier) {
    var current by remember(LocalConfiguration.current) { mutableStateOf(appLanguage().substringBefore('-')) }
    SettingsCard(modifier) {
        CardTitle(stringResource(R.string.settings_language))
        Column(Modifier.selectableGroup()) {
            LANGUAGES.forEach { (tag, label) ->
                ChoiceRow(stringResource(label), selected = current == tag) {
                    setAppLanguage(tag)
                    current = tag
                }
            }
        }
    }
}

@Composable
private fun SetsCard(settings: SettingsInfo, onAction: (Action) -> Unit, modifier: Modifier) {
    SettingsCard(modifier) {
        CardTitle(stringResource(R.string.settings_sets))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            PillButton(stringResource(R.string.settings_export), { onAction(Action.ExportSets) }, tonal = true)
            PillButton(stringResource(R.string.settings_import), { onAction(Action.ImportRequested) }, tonal = true)
        }
        val status = when (val export = settings.export) {
            null -> null
            ExportStatus.Exported -> stringResource(R.string.settings_exported)
            is ExportStatus.Failed -> stringResource(R.string.settings_export_failed, export.detail)
        }
        status?.let { Text(it, color = NotaeTheme.colors.muted, fontSize = 14.sp) }
        settings.importStatus?.let { Text(importStatusText(it), color = NotaeTheme.colors.muted, fontSize = 14.sp) }
    }
}

/** "Favorites and sets imported", or "Could not import sets: …" with why; shown in Settings and on the setup screen. */
@Composable
fun importStatusText(status: ImportStatus): String = importStatusText(LocalResources.current, status)

/** [importStatusText] outside composition: the toast for a file opened with or shared to Notae that fails. */
fun importStatusText(resources: Resources, status: ImportStatus): String = when (status) {
    ImportStatus.Imported -> resources.getString(R.string.settings_imported)
    is ImportStatus.Failed -> resources.getString(
        R.string.settings_import_failed,
        when (val error = status.error) {
            ImportError.NotNotae -> resources.getString(R.string.import_error_not_notae)
            ImportError.UnsupportedVersion -> resources.getString(R.string.import_error_version)
            ImportError.Invalid -> resources.getString(R.string.import_error_invalid)
            is ImportError.Unreadable -> error.detail
        },
    )
}

/**
 * The import confirmation, drawn by the shell over Settings or the setup screen. For a backup: "Import N sets?", the
 * current names of the sets the file replaces (when it replaces any), then Cancel and Import. For a shared set, the
 * review: "Import “name”?", how many of its scores matched, the unmatched ones by number and title, then Cancel and
 * Import, or, when a set with its id exists, its current name and Cancel, Replace and Keep both. The dialog takes the focus when it opens, so
 * Escape cancels at once, as Back does through the core (buttons cannot hold focus in touch mode); Tab reaches Cancel.
 */
@Composable
fun ImportConfirmationDialog(confirmation: ImportConfirmation, onAction: (Action) -> Unit) {
    val c = NotaeTheme.colors
    val keys = remember { FocusRequester() }
    LaunchedEffect(Unit) { keys.requestFocus() }
    val cancel = { onAction(Action.ImportCancelled) }
    val shared = confirmation.shared
    ModalSurface(
        stringResource(R.string.import_dialog),
        onDismiss = null,
        bottomSheet = false,
        cardWidth = 480.dp,
        modifier = Modifier.closeOnEscape(cancel).focusRequester(keys).focusable(),
    ) {
        if (shared != null) {
            SharedSetSummary(shared, confirmation.replaced.firstOrNull())
        } else {
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                val sets = confirmation.sets.toInt()
                Text(pluralStringResource(R.plurals.import_title, sets, sets), color = c.foreground, fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
                if (confirmation.replaced.isNotEmpty()) {
                    Text(stringResource(R.string.import_replaced, confirmation.replaced.joinToString(", ")), color = c.muted, fontSize = 14.sp)
                }
            }
        }
        val confirm = { onAction(Action.ImportConfirmed) }
        if (shared?.exists == true) {
            DialogButtons(
                stringResource(R.string.set_cancel),
                cancel,
                stringResource(R.string.keep_both),
                { onAction(Action.ImportKeepBoth) },
                middle = stringResource(R.string.replace),
                onMiddle = confirm,
            )
        } else {
            DialogButtons(stringResource(R.string.set_cancel), cancel, stringResource(R.string.import_confirm), confirm)
        }
    }
}

/**
 * The shared-set review's text: the title, "N of M scores matched", the name of the set Replace would change (when one
 * exists), and the unmatched pieces in a list that scrolls.
 */
@Composable
private fun SharedSetSummary(shared: SharedSetReview, replaced: String?) {
    val c = NotaeTheme.colors
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        val matched = shared.matched.toInt()
        val total = matched + shared.unmatched.size
        Text(stringResource(R.string.shared_set_review_title, shared.name), color = c.foreground, fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
        Text(pluralStringResource(R.plurals.shared_set_matched, total, matched, total), color = c.muted, fontSize = 14.sp)
        if (replaced != null) {
            Text(stringResource(R.string.shared_set_exists, replaced), color = c.muted, fontSize = 14.sp)
        }
        if (shared.unmatched.isNotEmpty()) {
            Text(
                stringResource(R.string.shared_set_unmatched),
                Modifier.padding(top = 10.dp),
                color = c.foreground,
                fontSize = 15.sp,
                fontWeight = FontWeight.SemiBold,
            )
            Column(Modifier.heightIn(max = 200.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                shared.unmatched.forEach { piece ->
                    Row(Modifier.fillMaxWidth().semantics(mergeDescendants = true) {}, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        piece.number?.let { Text(it, color = c.muted, fontSize = 15.sp) }
                        Text(piece.title, Modifier.weight(1f), color = c.foreground, fontSize = 15.sp)
                    }
                }
            }
        }
    }
}

/**
 * 0.3.0's `FactRow`: the muted label, the value at the right (red for [alarm]); read as one "label, value". On a phone a
 * long value (the last scan's summary) goes below its label instead of squeezing beside it.
 */
@Composable
private fun FactRow(label: String, value: String, alarm: Boolean = false) {
    val c = NotaeTheme.colors
    if (LocalConfiguration.current.screenWidthDp < COMPACT_WIDTH.value && value.length > 24) {
        Column(Modifier.fillMaxWidth().semantics(mergeDescendants = true) {}, verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label, color = c.muted, fontSize = 15.sp)
            Text(value, color = if (alarm) c.error else c.foreground, fontSize = 15.sp)
        }
        return
    }
    Row(Modifier.fillMaxWidth().semantics(mergeDescendants = true) {}, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(label, color = c.muted, fontSize = 15.sp)
        Text(value, Modifier.weight(1f), color = if (alarm) c.error else c.foreground, fontSize = 15.sp, textAlign = TextAlign.End)
    }
}

/** 0.3.0's `ChoiceRow` at the 48 dp target (52 with 8 between left the list twice as tall as the cards' other rows): 18 radius, the field colour while pressed, an accent ring on focus, a check when chosen. */
@Composable
private fun ChoiceRow(label: String, selected: Boolean, onClick: () -> Unit) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(18.dp)
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val focused by source.collectIsFocusedAsState()
    Row(
        Modifier
            .fillMaxWidth()
            .height(48.dp)
            .clip(shape)
            .background(animateColorAsState(if (pressed) c.field else c.field.copy(alpha = 0f), NotaeMotion.short(), label = "choice fill").value)
            .then(if (focused) Modifier.border(BorderStroke(2.dp, c.accent), shape) else Modifier)
            .selectable(selected, source, null, role = Role.RadioButton, onClick = onClick)
            .padding(horizontal = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, Modifier.weight(1f), color = c.foreground, fontSize = 16.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        if (selected) Icon(painterResource(R.drawable.ic_check), null, Modifier.size(24.dp), tint = c.accent)
    }
}

/** 0.3.0's order, with the AppCompat tag each sets: `""` follows the system. */
private val LANGUAGES = listOf(
    "" to R.string.settings_language_system,
    "en" to R.string.settings_language_english,
    "pl" to R.string.settings_language_polish,
)
