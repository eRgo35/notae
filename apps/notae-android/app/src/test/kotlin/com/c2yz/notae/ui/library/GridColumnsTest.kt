package com.c2yz.notae.ui.library

import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class GridColumnsTest {
    @Test
    fun tall_window_keeps_width_columns() = assertEquals(3, gridColumns(600.dp, 16.dp, 176.dp, 900.dp, 1.3f, 64.dp))

    @Test
    fun short_window_adds_columns_until_a_card_fits() {
        val n = gridColumns(496.dp, 16.dp, 176.dp, 240.dp, 1.3f, 64.dp)
        val cell = (496.dp - 16.dp * (n - 1)) / n

        assertTrue("$n columns, cell $cell", cell * 1.3f + 64.dp <= 240.dp)
        assertTrue("$n columns, cell $cell", cell >= 96.dp)
    }

    @Test
    fun never_narrower_than_96dp() {
        val n = gridColumns(496.dp, 16.dp, 176.dp, 80.dp, 1.3f, 64.dp)
        val cell = (496.dp - 16.dp * (n - 1)) / n

        assertTrue("$n columns, cell $cell", cell >= 96.dp)
    }

    @Test
    fun a_narrow_grid_keeps_one_column() = assertEquals(1, gridColumns(100.dp, 16.dp, 176.dp, 80.dp, 1.3f, 64.dp))
}
