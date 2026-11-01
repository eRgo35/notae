package com.c2yz.notae.nav

import com.c2yz.notae.core.CoreListener
import com.c2yz.notae.core.DocumentEntry
import com.c2yz.notae.core.Effect
import com.c2yz.notae.core.NotaeCore
import com.c2yz.notae.core.Origin
import com.c2yz.notae.core.PlatformLibrary
import com.c2yz.notae.core.Row
import com.c2yz.notae.core.RowStatus
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.Selection
import com.c2yz.notae.core.SetupScreen
import com.c2yz.notae.core.Snapshot
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/** One assertion per row of the back stack table in the design spec (§4.4). */
class BackStackTest {
    @get:Rule
    val dataDir = TemporaryFolder()

    @Test
    fun back_stack_for_each_screen() {
        val base = freshSnapshot()

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

    private fun freshSnapshot(): Snapshot {
        val core = NotaeCore(dataDir.root.path, NoPlatform, NoListener)
        try {
            return core.snapshot()
        } finally {
            core.close()
        }
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

private object NoPlatform : PlatformLibrary {
    override fun checkRoot(handle: String) = error("no library folder")

    override fun enumerate(handle: String): List<DocumentEntry> = error("no library folder")

    override fun extractTextLayer(handle: String, path: String): String? = error("no library folder")

    override fun renderThumbnail(handle: String, path: String, outPng: String): Boolean = error("no library folder")
}

private object NoListener : CoreListener {
    override fun onSnapshot(snapshot: Snapshot) = Unit

    override fun onEffect(effect: Effect) = Unit
}
