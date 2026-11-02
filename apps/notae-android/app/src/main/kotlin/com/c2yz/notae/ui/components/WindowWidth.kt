package com.c2yz.notae.ui.components

import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalConfiguration

/** The window's width class, as Material 3 names them: a phone, a small tablet or a phone on its side, a tablet. */
enum class WidthClass { COMPACT, MEDIUM, EXPANDED }

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
