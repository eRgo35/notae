package com.c2yz.notae.ui.components

import org.junit.Assert.assertEquals
import org.junit.Test

class WindowClassTest {
    @Test
    fun chrome_follows_width_and_height() {
        assertEquals(Chrome.BOTTOM_BAR, chromeFor(360, 640))
        assertEquals(Chrome.BOTTOM_BAR, chromeFor(412, 915))
        assertEquals(Chrome.SLIM_RAIL, chromeFor(640, 360))
        assertEquals(Chrome.SLIM_RAIL, chromeFor(832, 384))
        assertEquals(Chrome.SLIM_RAIL, chromeFor(915, 412))
        assertEquals(Chrome.RAIL, chromeFor(700, 500))
        assertEquals(Chrome.RAIL, chromeFor(600, 960))
        assertEquals(Chrome.RAIL, chromeFor(800, 1280))
        assertEquals(Chrome.SIDE_PANEL, chromeFor(840, 600))
        assertEquals(Chrome.SIDE_PANEL, chromeFor(1280, 800))
    }

    @Test
    fun height_class_boundary_is_480() {
        assertEquals(HeightClass.COMPACT, heightClassFor(479))
        assertEquals(HeightClass.REGULAR, heightClassFor(480))
    }
}
