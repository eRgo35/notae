package com.c2yz.notae.ui

import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.SettingsInfo
import com.c2yz.notae.ui.library.Fixture
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
}
