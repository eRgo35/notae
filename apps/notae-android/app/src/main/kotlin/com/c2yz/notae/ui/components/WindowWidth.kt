package com.c2yz.notae.ui.components

import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.unit.dp

/** The window's width class, as Material 3 names them: a phone, a small tablet or a phone on its side, a tablet. */
enum class WidthClass { COMPACT, MEDIUM, EXPANDED }

/** The window's height class: a short window (a phone on its side) or any other. */
enum class HeightClass { COMPACT, REGULAR }

/**
 * The library's chrome, chosen from the window's width and height by [chromeFor]:
 * - width under [COMPACT_WIDTH] → [BOTTOM_BAR] (a phone in portrait);
 * - height under [COMPACT_HEIGHT] → [SLIM_RAIL], an icon-only rail, never the side panel (a phone on its side);
 * - width under [EXPANDED_WIDTH] → [RAIL];
 * - otherwise → [SIDE_PANEL].
 */
enum class Chrome { BOTTOM_BAR, SLIM_RAIL, RAIL, SIDE_PANEL }

/** Windows shorter than this are short: they get the slim rail and never show two panes. */
internal val COMPACT_HEIGHT = 480.dp

/** The height class of a window [heightDp] tall. */
fun heightClassFor(heightDp: Int): HeightClass =
    if (heightDp < COMPACT_HEIGHT.value) HeightClass.COMPACT else HeightClass.REGULAR

/** The chrome for a window [widthDp] × [heightDp] (see [Chrome]). */
fun chromeFor(widthDp: Int, heightDp: Int): Chrome = when {
    widthDp < COMPACT_WIDTH.value -> Chrome.BOTTOM_BAR
    heightClassFor(heightDp) == HeightClass.COMPACT -> Chrome.SLIM_RAIL
    widthDp < EXPANDED_WIDTH.value -> Chrome.RAIL
    else -> Chrome.SIDE_PANEL
}

/**
 * The window's width class. It is the window's, not the pane's: the library beside the details on a tablet is narrow,
 * but it keeps the tablet's chrome.
 */
@Composable
fun windowWidthClass(): WidthClass {
    val width = LocalConfiguration.current.screenWidthDp
    return when {
        width < COMPACT_WIDTH.value -> WidthClass.COMPACT
        width < EXPANDED_WIDTH.value -> WidthClass.MEDIUM
        else -> WidthClass.EXPANDED
    }
}

/** The window's height class. */
@Composable
fun windowHeightClass(): HeightClass = heightClassFor(LocalConfiguration.current.screenHeightDp)

/** The window's chrome (see [Chrome]). */
@Composable
fun windowChrome(): Chrome {
    val configuration = LocalConfiguration.current
    return chromeFor(configuration.screenWidthDp, configuration.screenHeightDp)
}
