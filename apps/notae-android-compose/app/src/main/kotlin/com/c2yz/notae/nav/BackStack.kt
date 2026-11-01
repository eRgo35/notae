package com.c2yz.notae.nav

import androidx.navigation3.runtime.NavKey
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.Snapshot

/** A destination of the Navigation 3 shell. The core decides which are open; Kotlin never pushes one. */
sealed interface Route : NavKey {
    /** The saved library is being restored. */
    data object Loading : Route

    /** No library is open. */
    data object Setup : Route

    /** A library section, by its key (`all`, `recent`, `favorites`, `sets`, `composer:<name>`). */
    data class Library(val section: String) : Route

    /** An open set, above the list of sets. */
    data class Set(val id: String) : Route

    /** The details of a score, by its row's selection id. */
    data class Details(val selectionId: String) : Route

    data object Search : Route

    data object Settings : Route
}

private const val SET_PREFIX = "set:"

/**
 * The back stack the snapshot shows (design spec §4.4). It mirrors `NotaeCore.back`: what the core
 * closes first is last, so the details sit above search or settings, which sit above the open set.
 */
fun backStackFor(snapshot: Snapshot): List<NavKey> {
    val section = when (snapshot.screen) {
        Screen.Loading -> return listOf(Route.Loading)
        is Screen.Setup -> return listOf(Route.Setup)
        Screen.Library, Screen.Search, Screen.Settings -> snapshot.sectionKey
    }
    val stack = mutableListOf<NavKey>()
    if (section.startsWith(SET_PREFIX)) {
        stack += Route.Library("sets")
        stack += Route.Set(section.removePrefix(SET_PREFIX))
    } else {
        stack += Route.Library(section)
    }
    when (snapshot.screen) {
        Screen.Search -> stack += Route.Search
        Screen.Settings -> stack += Route.Settings
        else -> Unit
    }
    snapshot.selection?.let { stack += Route.Details(it.row.selectionId) }
    return stack
}
