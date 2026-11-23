package com.c2yz.notae.ui.viewer

import com.c2yz.notae.ui.components.HeightClass
import com.c2yz.notae.ui.components.WidthClass
import org.junit.Assert.assertEquals
import org.junit.Test

/** Which pages the viewer shows together. */
class SpreadsTest {
    @Test
    fun two_pages_only_on_a_landscape_tablet() {
        assertEquals(PageMode.TWO, pageModeFor(true, WidthClass.EXPANDED, HeightClass.REGULAR))
        assertEquals(PageMode.ONE, pageModeFor(false, WidthClass.MEDIUM, HeightClass.REGULAR))
        assertEquals(PageMode.ONE, pageModeFor(false, WidthClass.EXPANDED, HeightClass.REGULAR))
        assertEquals(PageMode.ONE, pageModeFor(true, WidthClass.MEDIUM, HeightClass.REGULAR))
        assertEquals(PageMode.FIT_WIDTH, pageModeFor(true, WidthClass.MEDIUM, HeightClass.COMPACT))
        assertEquals(PageMode.FIT_WIDTH, pageModeFor(true, WidthClass.EXPANDED, HeightClass.COMPACT))
    }

    @Test
    fun spreads_pair_one_two_three_four() {
        assertEquals(listOf(0..1, 2..3, 4..4), spreadsFor(5, PageMode.TWO))
        assertEquals(listOf(0..1), spreadsFor(2, PageMode.TWO))
        assertEquals(listOf(0..0, 1..1, 2..2), spreadsFor(3, PageMode.ONE))
        assertEquals(listOf(0..0, 1..1), spreadsFor(2, PageMode.FIT_WIDTH))
    }

    @Test
    fun the_spread_holding_a_page_survives_a_mode_change() {
        assertEquals(1, spreadOf(3, spreadsFor(5, PageMode.TWO)))
        assertEquals(3, spreadOf(3, spreadsFor(5, PageMode.ONE)))
        assertEquals(0, spreadOf(9, emptyList()))
    }
}
