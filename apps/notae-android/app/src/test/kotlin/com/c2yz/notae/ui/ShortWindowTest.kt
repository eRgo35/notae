package com.c2yz.notae.ui

import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.Selection
import com.c2yz.notae.core.SettingsInfo
import com.c2yz.notae.ui.details.DetailsScreen
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.library.LibraryScreen
import com.c2yz.notae.ui.settings.AboutInfo
import com.c2yz.notae.ui.settings.LicensesScreen
import com.c2yz.notae.ui.settings.SettingsScreen
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Screens in short windows (a phone on its side) and narrow ones: titles on one line, main content in view. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w640dp-h360dp-xhdpi")
class ShortWindowTest {
    @get:Rule
    val compose = createComposeRule()

    private val settings = Fixture.library().copy(
        screen = Screen.Settings,
        settings = SettingsInfo(libraryName = "NotaeRef", indexedCount = 20u, noTextCount = 0u, latestScan = null, export = null, importStatus = null),
    )

    /** The title's height in sp lines: one line of [sp] text is about 1.17 [sp] tall. */
    private fun titleHeightInSp(text: String): Float =
        compose.onNodeWithText(text).fetchSemanticsNode().size.height / (compose.density.density * compose.density.fontScale)

    @Test
    fun settings_title_is_one_line_in_short_window() {
        compose.setContent { NotaeTheme { SettingsScreen(settings, {}, AboutInfo(version = "0.8.0", build = "abc1234")) } }

        val height = titleHeightInSp("Settings")
        assertTrue("one line at 24 sp: $height sp", height < 1.3f * 24)
    }

    @Test
    @Config(qualifiers = "pl-w360dp-h640dp-xhdpi")
    fun licenses_title_is_one_line_on_a_phone() {
        compose.setContent { NotaeTheme { LicensesScreen({}) } }

        val height = titleHeightInSp("Licencje open source")
        assertTrue("one line at 26 sp: $height sp", height < 1.3f * 26)
    }

    private fun scoreTitleIsVisible() {
        compose.setContent { NotaeTheme { LibraryScreen(Fixture.library(), {}) } }

        // The card reads as its title (the title itself is hidden from TalkBack): the whole card, title included, is in view.
        compose.onNodeWithContentDescription("Agnus Dei").assertInsideWindow(compose)
    }

    private fun setNameIsVisible() {
        compose.setContent { NotaeTheme { LibraryScreen(Fixture.sets(), {}) } }

        compose.onNodeWithContentDescription("Niedziela Palmowa").assertInsideWindow(compose)
    }

    @Test
    fun a_score_title_is_visible() = scoreTitleIsVisible()

    @Test
    @Config(qualifiers = "w832dp-h384dp-xhdpi")
    fun a_score_title_is_visible_at_832x384() = scoreTitleIsVisible()

    @Test
    @Config(qualifiers = "w700dp-h500dp-xhdpi")
    fun a_score_title_is_visible_at_700x500() = scoreTitleIsVisible()

    @Test
    fun a_set_name_is_visible() = setNameIsVisible()

    @Test
    @Config(qualifiers = "w832dp-h384dp-xhdpi")
    fun a_set_name_is_visible_at_832x384() = setNameIsVisible()

    @Test
    @Config(qualifiers = "w700dp-h500dp-xhdpi")
    fun a_set_name_is_visible_at_700x500() = setNameIsVisible()

    private fun detailsActionsVisible() {
        val library = Fixture.library()
        val row = library.rows.first { it.title == "Agnus Dei" }
        compose.setContent { NotaeTheme { DetailsScreen(library.copy(selection = Selection(row, previous = null, next = "key-2", position = null)), {}) } }

        // The headline; the info card further down names the score too.
        compose.onAllNodesWithText("Agnus Dei")[0].assertInsideWindow(compose)
        compose.onNodeWithText("Open score").assertInsideWindow(compose)
        compose.onNodeWithContentDescription("Edit details").assertInsideWindow(compose)
    }

    @Test
    fun details_actions_visible() = detailsActionsVisible()

    @Test
    @Config(qualifiers = "w700dp-h500dp-xhdpi")
    fun details_actions_visible_at_700x500() = detailsActionsVisible()
}
