package com.c2yz.notae.nav

import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.navigation3.runtime.NavEntry
import androidx.navigation3.runtime.NavKey
import androidx.navigation3.runtime.NavMetadataKey
import androidx.navigation3.runtime.contains
import androidx.navigation3.runtime.metadata
import androidx.navigation3.scene.Scene
import androidx.navigation3.scene.SceneStrategy
import androidx.navigation3.scene.SceneStrategyScope
import androidx.navigation3.ui.NavDisplay
import androidx.navigationevent.NavigationEventInfo
import androidx.navigationevent.compose.NavigationBackHandler
import androidx.navigationevent.compose.rememberNavigationEventState
import com.c2yz.notae.BuildConfig
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.ui.details.DetailsScreen
import com.c2yz.notae.ui.details.MetadataEditor
import com.c2yz.notae.ui.library.LibraryScreen
import com.c2yz.notae.ui.search.SearchScreen
import com.c2yz.notae.ui.settings.AboutInfo
import com.c2yz.notae.ui.settings.ImportConfirmationDialog
import com.c2yz.notae.ui.sets.SetOverlays
import com.c2yz.notae.ui.sets.overlayOpen
import com.c2yz.notae.ui.settings.SettingsScreen
import com.c2yz.notae.ui.setup.WelcomeScreen
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * The app's screens, as the snapshot's back stack ([backStackFor]). The core owns navigation: every
 * back, gesture or key, goes to [onBack], which asks the core and finishes the activity when it has
 * nothing left to close. Navigation 3 animates the change when the next snapshot arrives.
 */
@Composable
fun NotaeNavDisplay(
    snapshot: Snapshot,
    onAction: (Action) -> Unit,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    voiceAvailable: Boolean = false,
) {
    val backStack = backStackFor(snapshot)
    BoxWithConstraints(modifier.shellFrame()) {
        // R37: two panes from 840 dp (a tablet in landscape), Details alone below that (portrait).
        val twoPane = maxWidth >= TWO_PANE_MIN_WIDTH
        val strategy = remember(twoPane) { ListDetailSceneStrategy(twoPane) }
        // The details beside a list leave the banner and the PDF error to the list's pane, which shows them once.
        val besideList = twoPane && isListRoute(backStack.getOrNull(backStack.size - 2))
        // Navigation 3 remembers the entries until the back stack changes, so they read the newest snapshot from
        // here instead of keeping the one they were built with (a banner, a star or new rows change no key).
        val shown = rememberUpdatedState(Shown(snapshot, onAction, voiceAvailable, besideList))
        NavDisplay(
            backStack = backStack,
            modifier = Modifier.fillMaxSize(),
            onBack = onBack,
            sceneStrategies = listOf(strategy),
            entryProvider = { key -> entryFor(key, shown) },
        )
        // Snapshot fields, not navigation entries: drawn over whichever screen opened it, and gone when it closes.
        snapshot.metadataEditor?.let { MetadataEditor(it, onAction) }
        SetOverlays(snapshot, onAction)
        snapshot.importConfirmation?.let { ImportConfirmationDialog(it, onAction) }
    }
    // Registered after NavDisplay's own handler, so it wins while enabled: a dialog or sheet closes
    // before any screen, without the predictive animation of the screen below, and on a single
    // screen (where NavDisplay handles nothing) the core still decides whether the activity ends.
    NavigationBackHandler(
        state = rememberNavigationEventState(NavigationEventInfo.None),
        // Reorder mode is not an entry: Back ends it without previewing the list of sets below the set.
        isBackEnabled = backStack.size == 1 || overlayOpen(snapshot) || snapshot.reordering,
        onBackCompleted = onBack,
    )
}

/**
 * The frame every screen sits in: the background runs edge to edge behind the system bars (the
 * activity draws edge to edge), and the content keeps clear of the bars, the cutout and the keyboard.
 */
@Composable
internal fun Modifier.shellFrame(): Modifier =
    background(NotaeTheme.colors.background).windowInsetsPadding(WindowInsets.safeDrawing)

private const val DETAILS_CONTENT_KEY = "details"

/** R37: the M3 expanded width, which a 1280×800 tablet reaches in landscape and not in portrait. */
private val TWO_PANE_MIN_WIDTH = 840.dp

private val ABOUT = AboutInfo(version = BuildConfig.VERSION_NAME, build = BuildConfig.BUILD_IDENTITY)

/** What the entries show: the newest snapshot and how to act on it. */
private data class Shown(val snapshot: Snapshot, val onAction: (Action) -> Unit, val voiceAvailable: Boolean, val besideList: Boolean)

private fun entryFor(key: NavKey, shown: State<Shown>): NavEntry<NavKey> = when (key) {
    Route.Loading -> placeholder(key, R.string.screen_loading)
    Route.Setup -> NavEntry(key) {
        val (snapshot, onAction) = shown.value
        val setup = (snapshot.screen as? Screen.Setup)?.setup
        if (setup != null) {
            WelcomeScreen(setup, snapshot.importAvailable, onAction, importStatus = snapshot.settings.importStatus)
        }
    }
    // The set view is the library screen inside the set (0.3.0 drew it in the same chrome).
    // ListPane comes from isListRoute, which also decides besideList, so the scene and the details agree.
    is Route.Library, is Route.Set -> NavEntry(key, metadata = metadata { if (isListRoute(key)) put(ListPane, true) }) {
        LibraryScreen(shown.value.snapshot, shown.value.onAction)
    }
    // One content key for every score: previous and next (and another score chosen in the list beside it) swap the
    // content in place, keeping the scroll and without a transition.
    is Route.Details -> NavEntry(key, contentKey = DETAILS_CONTENT_KEY, metadata = metadata { put(DetailPane, true) }) {
        DetailsScreen(shown.value.snapshot, shown.value.onAction, besideList = shown.value.besideList)
    }
    Route.Search -> NavEntry(key) { SearchScreen(shown.value.snapshot, shown.value.onAction, shown.value.voiceAvailable) }
    Route.Settings -> NavEntry(key) { SettingsScreen(shown.value.snapshot, shown.value.onAction, ABOUT) }
    // backStackFor puts only routes on the stack.
    else -> placeholder(key, R.string.screen_loading)
}

/** Shows only the screen's name until the screen itself arrives. */
private fun placeholder(key: NavKey, @StringRes name: Int) =
    NavEntry(key) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text(stringResource(name))
        }
    }

private fun isListRoute(key: NavKey?): Boolean = key is Route.Library || key is Route.Set

private data object ListPane : NavMetadataKey<Boolean>

private data object DetailPane : NavMetadataKey<Boolean>

/**
 * Library (or a set) and Details side by side on a wide window, as the details pane the app had beside the library
 * before 0.2.0 (`ScoreList` + `ScoreDetails`): when the details are on top of a list. A list on top is
 * the same scene without its detail pane, so the details open and close beside the library without moving it, and
 * the library keeps its state and focus. Below [TWO_PANE_MIN_WIDTH], and over Search or Settings, the details stay a
 * screen of their own.
 */
private class ListDetailSceneStrategy(private val twoPane: Boolean) : SceneStrategy<NavKey> {
    override fun SceneStrategyScope<NavKey>.calculateScene(entries: List<NavEntry<NavKey>>): Scene<NavKey>? {
        if (!twoPane) return null
        val top = entries.lastOrNull() ?: return null
        if (ListPane in top.metadata) return ListDetailScene(top, detail = null, previousEntries = entries.dropLast(1))
        if (DetailPane !in top.metadata) return null
        val list = entries.getOrNull(entries.size - 2)?.takeIf { ListPane in it.metadata } ?: return null
        return ListDetailScene(list, top, previousEntries = entries.dropLast(1))
    }
}

/**
 * Keyed by the list, so choosing another score, or opening and closing the details, changes the detail pane, not the
 * scene. The geometry is 0.3.0's split pane at 1280 dp: the library side ends where its 400 dp companion list did
 * (16 + 288 + 16 + 400 = 720 dp), and the details card keeps 0.3.0's 528 dp (`details-width`, window − 752) inside the
 * details' own 16 dp margins. On a narrower window the library keeps the same share of the width.
 */
private class ListDetailScene(
    private val list: NavEntry<NavKey>,
    private val detail: NavEntry<NavKey>?,
    override val previousEntries: List<NavEntry<NavKey>>,
) : Scene<NavKey> {
    override val key: Any = list.contentKey
    override val entries: List<NavEntry<NavKey>> = listOfNotNull(list, detail)
    override val content: @Composable () -> Unit = {
        BoxWithConstraints(Modifier.fillMaxSize()) {
            val listWidth = if (detail == null) maxWidth else minOf(LIST_PANE_WIDTH, maxWidth * LIST_SHARE)
            Row(Modifier.fillMaxSize()) {
                Box(Modifier.width(listWidth).fillMaxHeight()) { list.Content() }
                if (detail != null) Box(Modifier.weight(1f).fillMaxHeight()) { detail.Content() }
            }
        }
    }

    private companion object {
        val LIST_PANE_WIDTH = 720.dp
        const val LIST_SHARE = 720f / 1280f
    }
}
