package com.c2yz.notae.platform

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.util.Locale

@RunWith(RobolectricTestRunner::class)
class LocaleTest {
    @Test
    fun iso_date_formats_per_locale() {
        val english = formatIsoDate("2026-11-01", Locale.ENGLISH)
        val polish = formatIsoDate("2026-11-01", Locale.forLanguageTag("pl"))

        assertEquals("Nov 1, 2026", english)
        assertTrue(polish, polish.startsWith("1 ") && polish.endsWith(" 2026") && polish.length > "1 2026".length)
        assertNotEquals(english, polish)
    }

    @Test
    @Config(sdk = [30])
    fun language_round_trips_through_appcompat() {
        setAppLanguage("pl")

        assertEquals("pl", appLanguage())
    }
}
