package com.c2yz.notae.nav

import com.c2yz.notae.core.Origin
import com.c2yz.notae.core.Row
import com.c2yz.notae.core.RowStatus
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.Selection
import com.c2yz.notae.core.SetupScreen
import com.c2yz.notae.ui.library.Fixture
import org.junit.Assert.assertEquals
import org.junit.Test

/** One assertion per row of the back stack table in the design spec (§4.4). */
class BackStackTest {

    @Test
    fun back_stack_for_each_screen() {
        // A fresh core's snapshot, without loading the core outside a Robolectric sandbox (CoreSmokeTest explains).
        val base = Fixture.base

        assertEquals(listOf(Route.Loading), backStackFor(base.copy(screen = Screen.Loading)))
        assertEquals(listOf(Route.Setup), backStackFor(base.copy(screen = Screen.Setup(SetupScreen.Unconfigured))))
        assertEquals(
            listOf(Route.Library("favorites")),
            backStackFor(base.copy(screen = Screen.Library, sectionKey = "favorites")),
        )
        assertEquals(
            listOf(Route.Library("sets"), Route.Set("0123456789abcdef")),
            backStackFor(base.copy(screen = Screen.Library, sectionKey = "set:0123456789abcdef")),
        )
        assertEquals(
            listOf(Route.Library("all"), Route.Details("score")),
            backStackFor(base.copy(screen = Screen.Library, sectionKey = "all", selection = selection("score"))),
        )
        assertEquals(
            listOf(Route.Library("recent"), Route.Search),
            backStackFor(base.copy(screen = Screen.Search, sectionKey = "recent")),
        )
        assertEquals(
            listOf(Route.Library("sets"), Route.Set("0123456789abcdef"), Route.Settings),
            backStackFor(base.copy(screen = Screen.Settings, sectionKey = "set:0123456789abcdef")),
        )
        // The details close before search does (NotaeCore.back), so they sit above it.
        assertEquals(
            listOf(Route.Library("all"), Route.Search, Route.Details("score")),
            backStackFor(base.copy(screen = Screen.Search, sectionKey = "all", selection = selection("score"))),
        )
    }

    private fun selection(id: String) = Selection(
        row = Row(
            selectionId = id,
            noteKey = id,
            number = null,
            title = "Ave Maria",
            composer = null,
            tags = emptyList(),
            status = RowStatus.READY,
            textPending = false,
            relativePath = "Ave Maria.pdf",
            titleOrigin = Origin.FILENAME_DERIVED,
            composerOrigin = Origin.FILENAME_DERIVED,
            snippet = null,
            itemId = null,
            justOpened = false,
            position = null,
            missing = false,
            favorite = false,
            thumbnail = null,
        ),
        previous = null,
        next = null,
        position = null,
    )
}
