package com.c2yz.notae.nav

import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.navigation3.runtime.NavEntry
import androidx.navigation3.runtime.NavKey
import androidx.navigation3.ui.NavDisplay
import androidx.navigationevent.NavigationEventInfo
import androidx.navigationevent.compose.NavigationBackHandler
import androidx.navigationevent.compose.rememberNavigationEventState
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.ui.details.DetailsScreen
import com.c2yz.notae.ui.details.MetadataEditor
import com.c2yz.notae.ui.library.LibraryScreen
import com.c2yz.notae.ui.setup.WelcomeScreen
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * The app's screens, as the snapshot's back stack ([backStackFor]). The core owns navigation: every
 * back, gesture or key, goes to [onBack], which asks the core and finishes the activity when it has
 * nothing left to close. Navigation 3 animates the change when the next snapshot arrives.
 */
@Composable
fun NotaeNavDisplay(snapshot: Snapshot, onAction: (Action) -> Unit, onBack: () -> Unit, modifier: Modifier = Modifier) {
    val backStack = backStackFor(snapshot)
    Box(modifier.shellFrame()) {
        NavDisplay(
            backStack = backStack,
            modifier = Modifier.fillMaxSize(),
            onBack = onBack,
            entryProvider = { key -> entryFor(key, snapshot, onAction) },
        )
        // A snapshot field, not a navigation entry: drawn over whichever screen opened it, and gone when it closes.
        snapshot.metadataEditor?.let { MetadataEditor(it, onAction) }
    }
    // Registered after NavDisplay's own handler, so it wins while enabled: a dialog or sheet closes
    // before any screen, without the predictive animation of the screen below, and on a single
    // screen (where NavDisplay handles nothing) the core still decides whether the activity ends.
    NavigationBackHandler(
        state = rememberNavigationEventState(NavigationEventInfo.None),
        isBackEnabled = backStack.size == 1 || overlayOpen(snapshot),
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

/** A dialog or sheet, which the core closes before any screen. */
private fun overlayOpen(snapshot: Snapshot): Boolean =
    snapshot.setDialog != null || snapshot.deleteSet != null || snapshot.metadataEditor != null ||
        snapshot.addToSet != null || snapshot.addPicker != null

private fun entryFor(key: NavKey, snapshot: Snapshot, onAction: (Action) -> Unit): NavEntry<NavKey> = when (key) {
    Route.Loading -> placeholder(key, R.string.screen_loading)
    Route.Setup -> setupEntry(key, snapshot, onAction)
    is Route.Library -> NavEntry(key) { LibraryScreen(snapshot, onAction) }
    is Route.Set -> placeholder(key, R.string.screen_set)
    // Full width, as Slint's details replace the whole library chrome (docs/compose-parity/README.md).
    is Route.Details -> NavEntry(key) { DetailsScreen(snapshot, onAction) }
    Route.Search -> placeholder(key, R.string.screen_search)
    Route.Settings -> placeholder(key, R.string.screen_settings)
    // backStackFor puts only routes on the stack.
    else -> placeholder(key, R.string.screen_loading)
}

private fun setupEntry(key: NavKey, snapshot: Snapshot, onAction: (Action) -> Unit): NavEntry<NavKey> =
    NavEntry(key) {
        val setup = (snapshot.screen as? Screen.Setup)?.setup
        if (setup != null) {
            // The import flow arrives with Task 20; until then the button does nothing.
            WelcomeScreen(setup, snapshot.importAvailable, onAction, onImport = {})
        }
    }

/** Shows only the screen's name until the screen itself arrives. */
private fun placeholder(key: NavKey, @StringRes name: Int) =
    NavEntry(key) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text(stringResource(name))
        }
    }
