package com.c2yz.notae.ui.library

import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class HeaderActionsTest {
    private fun a(key: String, width: Int, priority: Int, pinned: Boolean = false) = HeaderAction(key, width.dp, priority, pinned)

    @Test
    fun everything_fits() = assertEquals(
        listOf("search", "view", "scan") to emptyList<String>(),
        fitActions(listOf(a("search", 48, 9, true), a("view", 48, 2), a("scan", 48, 0)), 400.dp),
    )

    @Test
    fun scan_goes_first_and_reserves_overflow() {
        val actions = listOf(
            a("setBack", 48, 9, true), a("search", 48, 9, true), a("view", 48, 2),
            a("reorder", 140, 3), a("share", 48, 1), a("add", 110, 5), a("scan", 48, 0),
        )
        // All seven need 514 dp; without Scan and Share the rest and ⋮ need 462 dp.
        val (shown, over) = fitActions(actions, 470.dp)

        assertEquals(listOf("scan", "share"), over)
        assertEquals(listOf("setBack", "search", "view", "reorder", "add"), shown)
    }

    @Test
    fun pinned_never_overflow() {
        val (shown, over) = fitActions(listOf(a("setBack", 48, 9, true), a("search", 48, 9, true), a("add", 110, 5), a("scan", 48, 0)), 0.dp)

        assertEquals(listOf("setBack", "search"), shown)
        assertEquals(listOf("scan", "add"), over)
    }

    @Test
    fun nothing_overflows_without_need_for_the_menu() {
        val (_, over) = fitActions(listOf(a("search", 48, 9, true), a("scan", 48, 0)), 100.dp)

        assertTrue(over.isEmpty())
    }
}
