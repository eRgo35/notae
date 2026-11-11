package com.c2yz.notae.nav

import com.c2yz.notae.ui.components.backProgress
import com.c2yz.notae.ui.components.LocalBackProgress
import com.c2yz.notae.ui.components.AnimatedOverlay
import androidx.compose.runtime.CompositionLocalProvider
import androidx.annotation.StringRes
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.ContentTransform
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.updateTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
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
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.navigation3.runtime.NavEntry
import androidx.navigation3.runtime.NavKey
import androidx.navigation3.runtime.NavMetadataKey
import androidx.navigation3.runtime.contains
import androidx.navigation3.runtime.get
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
import com.c2yz.notae.ui.theme.NotaeMotion
import com.c2yz.notae.ui.theme.NotaeTheme
import com.c2yz.notae.ui.theme.Tokens

/**
 * The app's screens, as the snapshot's back stack ([backStackFor]). The core owns navigation: every
 * back, gesture or key, goes to [onBack], which asks the core and sends the app to the background when it has
 * nothing left to close. Navigation 3 animates the change when the next snapshot arrives ([ScreenMotion]).
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
    val backState = rememberNavigationEventState(NavigationEventInfo.None)
    BoxWithConstraints(modifier.shellFrame()) {
        // R37: two panes from 840 dp (a tablet in landscape), Details alone below that (portrait).
        val twoPane = maxWidth >= TWO_PANE_MIN_WIDTH
        val strategy = remember(twoPane) { ListDetailSceneStrategy(twoPane) }
        // The details beside a list leave the banner and the PDF error to the list's pane, which shows them once.
        val besideList = twoPane && isListRoute(backStack.getOrNull(backStack.size - 2))
        // Navigation 3 remembers the entries until the back stack changes, so they read the newest snapshot from
        // here instead of keeping the one they were built with (a banner, a star or new rows change no key).
        val shown = rememberUpdatedState(Shown(snapshot, onAction, voiceAvailable, besideList, backStack))
        val order = rememberUpdatedState(snapshot.sections.map { it.key } + snapshot.composers.map { it.key })
        val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
        val motion = remember(rtl) { ScreenMotion(rtl, order) }
        NavDisplay(
            backStack = backStack,
            modifier = Modifier.fillMaxSize(),
            onBack = onBack,
            sceneStrategies = listOf(strategy),
            transitionSpec = { motion.forward(initialState, targetState) },
            popTransitionSpec = { motion.back(initialState, targetState) },
            predictivePopTransitionSpec = { motion.predictiveBack(initialState, targetState) },
            entryProvider = { key -> entryFor(key, shown, motion) },
        )
        // Snapshot fields, not navigation entries: drawn over whichever screen opened it, and gone when it closes.
        CompositionLocalProvider(LocalBackProgress provides remember(backState) { backProgress(backState) }) {
            AnimatedOverlay(snapshot.metadataEditor) { MetadataEditor(it, onAction) }
            SetOverlays(snapshot, onAction)
            AnimatedOverlay(snapshot.importConfirmation) { ImportConfirmationDialog(it, onAction) }
        }
    }
    // Registered after NavDisplay's own handler, so it wins while enabled: a dialog or sheet closes
    // before any screen, without the predictive animation of the screen below, and on a single
    // screen (where NavDisplay handles nothing) the core still decides whether the activity ends.
    NavigationBackHandler(
        state = backState,
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

/**
 * How screens change, toward the end of the line (flipped in a right-to-left language): only content moves, and the
 * bars stay where they are. Navigation 3 holds both screens still while each moves its own parts ([ScreenFrame]): a
 * deeper screen's content slides in over the content it covers, which stays still, and Back reverses that (the gesture
 * shrinks the content it leaves); switching sections ([lateral]) slides both contents together in the rail's [order].
 * A screen's bars (rail, top bar, bottom bar) fade in place over the other's, so bars both screens share do not move.
 * Setup and Loading fade through as whole screens.
 */
internal class ScreenMotion(rtl: Boolean, private val order: State<List<String>>) {
    /** The change now running, set as Navigation 3 starts it and read as the screens draw it. */
    var change: Change = Change.Fade
        private set

    /** The end of the line: 1 to the right, -1 to the left. */
    val end: Float = if (rtl) -1f else 1f

    private fun start(change: Change, popping: Boolean = false): ContentTransform {
        this.change = change
        if (change == Change.Fade) return fadeThrough()
        // The screen Back takes away stays on top while its content leaves.
        return ContentTransform(EnterTransition.None, ExitTransition.None, targetContentZIndex = if (popping) -1f else 0f)
    }

    private fun lateralChange(from: Scene<NavKey>, to: Scene<NavKey>): Change? =
        when (lateral(from, to, order.value)) {
            Lateral.Fade -> Change.Fade
            Lateral.Forward -> Change.Forward
            Lateral.Backward -> Change.Backward
            null -> null
        }

    fun forward(from: Scene<NavKey>, to: Scene<NavKey>): ContentTransform = start(lateralChange(from, to) ?: Change.Push)

    fun back(from: Scene<NavKey>, to: Scene<NavKey>): ContentTransform =
        lateralChange(from, to)?.let { start(it) } ?: start(Change.Pop, popping = true)

    fun predictiveBack(from: Scene<NavKey>, to: Scene<NavKey>): ContentTransform =
        lateralChange(from, to)?.let { start(it) } ?: start(Change.PredictivePop, popping = true)

    private fun fadeThrough(): ContentTransform =
        fadeIn(tween(Tokens.motionMediumMillis, delayMillis = FADE_OUT_MILLIS)) togetherWith fadeOut(tween(FADE_OUT_MILLIS))
}

private const val FADE_OUT_MILLIS = 90

/** How a change of the stack's root moves: [Fade] through (Setup, Loading), or a section [Forward] or [Backward] in the rail. */
internal enum class Lateral { Fade, Forward, Backward }

/**
 * A change of the stack's root (a section switch, Setup to the library, the first screen after Loading) moves sideways,
 * or `null` when the root is kept and a screen opens or closes above it. Sections are ordered by [order], and one
 * missing from it counts as past the end.
 */
internal fun lateral(from: Scene<*>, to: Scene<*>, order: List<String>): Lateral? {
    val a = rootEntry(from)
    val b = rootEntry(to)
    if (a?.contentKey == b?.contentKey) return null
    val sectionA = a?.metadata?.get(SectionKey)
    val sectionB = b?.metadata?.get(SectionKey)
    if (sectionA == null || sectionB == null) return Lateral.Fade
    fun index(section: String) = order.indexOf(section).let { if (it < 0) order.size else it }
    return if (index(sectionB) > index(sectionA)) Lateral.Forward else Lateral.Backward
}

private fun rootEntry(scene: Scene<*>): NavEntry<*>? = scene.previousEntries.firstOrNull() ?: scene.entries.firstOrNull()

/** The library section a [Route.Library] entry shows, which orders the sections for [lateral]. */
internal data object SectionKey : NavMetadataKey<String>

/** R37: the M3 expanded width, which a 1280×800 tablet reaches in landscape and not in portrait. */
private val TWO_PANE_MIN_WIDTH = 840.dp

private val ABOUT = AboutInfo(version = BuildConfig.VERSION_NAME, build = BuildConfig.BUILD_IDENTITY)

/** What the entries show: the newest snapshot, how to act on it, and the stack it makes. */
private data class Shown(
    val snapshot: Snapshot,
    val onAction: (Action) -> Unit,
    val voiceAvailable: Boolean,
    val besideList: Boolean,
    val stack: List<NavKey>,
)

/**
 * What the entry for [key] draws: the newest [Shown] while the entry is on screen (on top, or the list beside the
 * details), and the last one it was on screen with once it is not, so a screen sliding away keeps its own content
 * instead of drawing the screen it leaves for.
 */
@Composable
private fun shownFor(key: NavKey, shown: State<Shown>): Shown {
    val held = remember { Held(shown.value) }
    val stack = shown.value.stack
    val top = stack.lastOrNull()
    val visible = key == top || (key is Route.Details && top is Route.Details) ||
        (top is Route.Details && stack.getOrNull(stack.size - 2) == key)
    if (visible) held.value = shown.value
    return held.value
}

/** Not state: a new value always comes with a recomposition. */
private class Held(var value: Shown)

private fun entryFor(key: NavKey, shown: State<Shown>, motion: ScreenMotion): NavEntry<NavKey> = when (key) {
    Route.Loading -> placeholder(key, R.string.screen_loading)
    Route.Setup -> NavEntry(key) {
        val (snapshot, onAction) = shownFor(key, shown)
        val setup = (snapshot.screen as? Screen.Setup)?.setup
        if (setup != null) {
            WelcomeScreen(setup, snapshot.importAvailable, onAction, importStatus = snapshot.settings.importStatus)
        }
    }
    // The set view is the library screen inside the set (0.3.0 drew it in the same chrome).
    // ListPane comes from isListRoute, which also decides besideList, so the scene and the details agree.
    is Route.Library, is Route.Set -> NavEntry(key, metadata = metadata {
        if (isListRoute(key)) put(ListPane, true)
        if (key is Route.Library) put(SectionKey, key.section)
    }) {
        val (snapshot, onAction) = shownFor(key, shown)
        ScreenFrame(motion) { LibraryScreen(snapshot, onAction) }
    }
    // One content key for every score: previous and next (and another score chosen in the list beside it) swap the
    // content in place, and the details slide between the scores themselves.
    is Route.Details -> NavEntry(key, contentKey = DETAILS_CONTENT_KEY, metadata = metadata { put(DetailPane, true) }) {
        val it = shownFor(key, shown)
        ScreenFrame(motion) { DetailsScreen(it.snapshot, it.onAction, besideList = it.besideList) }
    }
    Route.Search -> NavEntry(key) {
        val it = shownFor(key, shown)
        ScreenFrame(motion) { SearchScreen(it.snapshot, it.onAction, it.voiceAvailable) }
    }
    Route.Settings -> NavEntry(key) {
        val (snapshot, onAction) = shownFor(key, shown)
        ScreenFrame(motion) { SettingsScreen(snapshot, onAction, ABOUT) }
    }
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
            // Only the detail pane moves: it slides in from the end of the line, full width, as a deeper screen's
            // content does, and Back slides it out again; another score chosen keeps the pane where it is.
            val pane = updateTransition(detail != null, label = "detail pane")
            // The details last shown, which the pane keeps drawing while it slides away.
            val last = remember { LastDetail() }
            if (detail != null) last.entry = detail
            val end = if (LocalLayoutDirection.current == LayoutDirection.Rtl) -1 else 1
            // The list takes its layout for where the change goes as it starts, opening or closing, and the pane slides
            // over it. Not animated: a grid re-flowing its columns at each step of a moving width loses its place.
            val sideWidth = minOf(LIST_PANE_WIDTH, maxWidth * LIST_SHARE)
            val listWidth = if (pane.targetState) sideWidth else maxWidth
            val paneWidth = maxWidth - sideWidth
            Box(Modifier.fillMaxSize()) {
                Box(Modifier.width(listWidth).fillMaxHeight()) { list.Content() }
                if (pane.currentState || pane.targetState) {
                    Box(Modifier.align(Alignment.TopEnd).width(paneWidth).fillMaxHeight().clipToBounds()) {
                        pane.AnimatedVisibility(
                            visible = { it },
                            enter = slideInHorizontally(NotaeMotion.long()) { it * end },
                            exit = slideOutHorizontally(NotaeMotion.long()) { it * end },
                        ) {
                            last.entry?.Content()
                        }
                    }
                }
            }
        }
    }

    private companion object {
        val LIST_PANE_WIDTH = 720.dp
        const val LIST_SHARE = 720f / 1280f
    }
}

/** Not state: a new detail always comes with a recomposition. */
private class LastDetail(var entry: NavEntry<NavKey>? = null)
