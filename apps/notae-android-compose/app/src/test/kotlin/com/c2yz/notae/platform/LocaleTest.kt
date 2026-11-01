package com.c2yz.notae.platform

import com.c2yz.notae.R
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
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

    /** Polish one / few / many (texts.rs `polish_plural`) for every pluralised count, 22 and 25 included. */
    @Test
    @Config(qualifiers = "pl")
    fun polish_plurals_1_2_5_22_25() {
        val resources = RuntimeEnvironment.getApplication().resources
        fun forms(id: Int) = listOf(1, 2, 5, 22, 25).map { resources.getQuantityString(id, it, it) }

        assertEquals(listOf("1 pozycja", "2 pozycje", "5 pozycji", "22 pozycje", "25 pozycji"), forms(R.plurals.set_items))
        assertEquals(listOf("1 utwór", "2 utwory", "5 utworów", "22 utwory", "25 utworów"), forms(R.plurals.library_scores))
        assertEquals(
            listOf("1 zestaw", "2 zestawy", "5 zestawów", "22 zestawy", "25 zestawów").map { "$it · ostatnio edytowane najpierw" },
            forms(R.plurals.library_sets_subtitle),
        )
        assertEquals(
            listOf("1 pliku", "2 plików", "5 plików", "22 plików", "25 plików").map { "Indeksowanie $it PDF…" },
            forms(R.plurals.banner_indexing),
        )
        assertEquals(listOf("1 utwór", "2 utwory", "5 utworów", "22 utwory", "25 utworów"), forms(R.plurals.set_pieces))
        assertEquals(listOf("1 notatka", "2 notatki", "5 notatek", "22 notatki", "25 notatek"), forms(R.plurals.set_notes))
        assertEquals(
            listOf("1 dzień", "2 dni", "5 dni", "22 dni", "25 dni").map { "Edytowano $it temu" },
            forms(R.plurals.set_edited_days),
        )
        assertEquals(
            listOf("1 utwór", "2 utwory", "5 utworów", "22 utwory", "25 utworów").map { "Dodano $it na końcu zestawu" },
            forms(R.plurals.picker_added),
        )
    }
}
