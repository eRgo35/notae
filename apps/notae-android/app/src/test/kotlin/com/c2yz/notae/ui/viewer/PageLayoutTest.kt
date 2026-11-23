package com.c2yz.notae.ui.viewer

import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import com.c2yz.notae.core.ViewerLayout
import com.c2yz.notae.core.ViewerPages
import com.c2yz.notae.core.ViewerScale
import com.c2yz.notae.ui.components.HeightClass.COMPACT
import com.c2yz.notae.ui.components.HeightClass.REGULAR
import com.c2yz.notae.ui.components.WidthClass.EXPANDED
import com.c2yz.notae.ui.components.WidthClass.MEDIUM
import org.junit.Assert.assertEquals
import org.junit.Test

/** How the viewer resolves its look and sizes a row of pages. */
class PageLayoutTest {
    private val auto = Triple(ViewerLayout.AUTO, ViewerPages.AUTO, ViewerScale.Auto)

    @Test
    fun auto_matches_today() {
        val (l, p, s) = auto
        assertEquals(ViewLook(false, true, ViewerScale.FitPage), resolveLook(l, p, s, true, EXPANDED, REGULAR))
        assertEquals(ViewLook(false, false, ViewerScale.FitPage), resolveLook(l, p, s, false, MEDIUM, REGULAR))
        assertEquals(ViewLook(false, false, ViewerScale.FitPage), resolveLook(l, p, s, false, EXPANDED, REGULAR))
        assertEquals(ViewLook(true, false, ViewerScale.FitWidth), resolveLook(l, p, s, true, MEDIUM, COMPACT))
        assertEquals(ViewLook(true, false, ViewerScale.FitWidth), resolveLook(l, p, s, true, EXPANDED, COMPACT))
    }

    @Test
    fun chosen_values_win_over_auto() {
        assertEquals(
            ViewLook(true, true, ViewerScale.FitHeight),
            resolveLook(ViewerLayout.SCROLL, ViewerPages.TWO, ViewerScale.FitHeight, false, MEDIUM, REGULAR),
        )
        assertEquals(
            ViewLook(false, false, ViewerScale.Percent(150u)),
            resolveLook(ViewerLayout.PAGES, ViewerPages.ONE, ViewerScale.Percent(150u), true, EXPANDED, COMPACT),
        )
    }

    @Test
    fun rows_pair_pages_odd_last_alone_in_scroll() {
        assertEquals(listOf(0..1, 2..3, 4..4), rowsFor(5, twoPages = true))
        assertEquals(listOf(0..0, 1..1, 2..2), rowsFor(3, twoPages = false))
        assertEquals(emptyList<IntRange>(), rowsFor(0, twoPages = true))
    }

    @Test
    fun the_row_holding_a_page_survives_a_change() {
        assertEquals(1, rowOf(3, rowsFor(5, true)))
        assertEquals(3, rowOf(3, rowsFor(5, false)))
        assertEquals(0, rowOf(9, emptyList()))
    }

    private fun size(pages: Int, scale: ViewerScale, area: Size = Size(1000f, 800f), dpi: Float = 320f) =
        rowSize(List(pages) { 1.414f }, List(pages) { 595f }, area, 10f, scale, dpi)

    private fun assertSize(expected: Size, actual: Size) {
        assertEquals(expected.width, actual.width, 0.5f)
        assertEquals(expected.height, actual.height, 0.5f)
    }

    @Test
    fun fit_page_fits_both_sides() {
        assertSize(Size(565.8f, 800f), size(1, ViewerScale.FitPage))
        // Two A4 side by side in a narrow area: the width limits them.
        assertSize(Size(600f, (295f * 1.414f)), size(2, ViewerScale.FitPage, Size(600f, 800f)))
    }

    @Test
    fun fit_width_fills_the_width() = assertSize(Size(1000f, 1414f), size(1, ViewerScale.FitWidth))

    @Test
    fun fit_height_fills_the_height() = assertSize(Size(2 * 565.8f + 10f, 800f), size(2, ViewerScale.FitHeight))

    @Test
    fun hundred_percent_is_printed_size() {
        assertEquals(2644.4f, size(1, ViewerScale.Percent(100u)).width, 0.5f)
        assertEquals(1322.2f, size(1, ViewerScale.Percent(50u)).width, 0.5f)
        assertEquals(2 * 1322.2f + 10f, size(2, ViewerScale.Percent(50u)).width, 0.5f)
    }

    @Test
    fun boxes_sit_side_by_side_with_the_gap() {
        val boxes = pageBoxes(listOf(1.414f, 1.414f), Size(2 * 565.8f + 10f, 800f), 10f)
        assertEquals(2, boxes.size)
        assertEquals(Rect(0f, 0f, 565.8f, 800f).right, boxes[0].right, 0.5f)
        assertEquals(575.8f, boxes[1].left, 0.5f)
        assertEquals(800f, boxes[1].bottom, 0.5f)
    }
}
