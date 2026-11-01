package com.c2yz.notae.nav

import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
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
import com.c2yz.notae.R
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * The app's screens, as the snapshot's back stack ([backStackFor]). The core owns navigation: every
 * back, gesture or key, goes to [onBack], which asks the core and finishes the activity when it has
 * nothing left to close. Navigation 3 animates the change when the next snapshot arrives.
 */
@Composable
fun NotaeNavDisplay(snapshot: Snapshot, onBack: () -> Unit, modifier: Modifier = Modifier) {
    val backStack = backStackFor(snapshot)
    NavDisplay(
        backStack = backStack,
        modifier = modifier.background(NotaeTheme.colors.background),
        onBack = onBack,
        sceneStrategies = listOf(ListDetailSceneStrategy()),
        entryProvider = ::entryFor,
    )
    // Registered after NavDisplay's own handler, so it wins while enabled: a dialog or sheet closes
    // before any screen, without the predictive animation of the screen below, and on a single
    // screen (where NavDisplay handles nothing) the core still decides whether the activity ends.
    NavigationBackHandler(
        state = rememberNavigationEventState(NavigationEventInfo.None),
        isBackEnabled = backStack.size == 1 || overlayOpen(snapshot),
        onBackCompleted = onBack,
    )
}

/** A dialog or sheet, which the core closes before any screen. */
private fun overlayOpen(snapshot: Snapshot): Boolean =
    snapshot.setDialog != null || snapshot.deleteSet != null || snapshot.metadataEditor != null ||
        snapshot.addToSet != null || snapshot.addPicker != null

private fun entryFor(key: NavKey): NavEntry<NavKey> = when (key) {
    Route.Loading -> placeholder(key, R.string.screen_loading)
    Route.Setup -> placeholder(key, R.string.screen_setup)
    is Route.Library -> placeholder(key, R.string.screen_library, metadata { put(ListPane, true) })
    is Route.Set -> placeholder(key, R.string.screen_set, metadata { put(ListPane, true) })
    is Route.Details -> placeholder(key, R.string.screen_details, metadata { put(DetailPane, true) })
    Route.Search -> placeholder(key, R.string.screen_search)
    Route.Settings -> placeholder(key, R.string.screen_settings)
    // backStackFor puts only routes on the stack.
    else -> placeholder(key, R.string.screen_loading)
}

/** Shows only the screen's name until the screen itself arrives. */
private fun placeholder(key: NavKey, @StringRes name: Int, metadata: Map<String, Any> = emptyMap()) =
    NavEntry(key, metadata = metadata) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text(stringResource(name))
        }
    }

private data object ListPane : NavMetadataKey<Boolean>

private data object DetailPane : NavMetadataKey<Boolean>

/**
 * Library (or a set) and Details side by side, as today's details pane: when the details are on
 * top of a list. The app runs only on tablets, so the window is always wide enough.
 */
private class ListDetailSceneStrategy : SceneStrategy<NavKey> {
    override fun SceneStrategyScope<NavKey>.calculateScene(entries: List<NavEntry<NavKey>>): Scene<NavKey>? {
        val detail = entries.lastOrNull()?.takeIf { DetailPane in it.metadata } ?: return null
        val list = entries.getOrNull(entries.size - 2)?.takeIf { ListPane in it.metadata } ?: return null
        return ListDetailScene(list, detail, previousEntries = entries.dropLast(1))
    }
}

/** Keyed by the list, so moving between scores changes the pane, not the scene. */
private class ListDetailScene(
    private val list: NavEntry<NavKey>,
    private val detail: NavEntry<NavKey>,
    override val previousEntries: List<NavEntry<NavKey>>,
) : Scene<NavKey> {
    override val key: Any = list.contentKey
    override val entries: List<NavEntry<NavKey>> = listOf(list, detail)
    override val content: @Composable () -> Unit = {
        Row(Modifier.fillMaxSize()) {
            Box(Modifier.weight(LIST_WEIGHT).fillMaxHeight()) { list.Content() }
            Box(Modifier.weight(1f - LIST_WEIGHT).fillMaxHeight()) { detail.Content() }
        }
    }

    private companion object {
        const val LIST_WEIGHT = 0.6f
    }
}
