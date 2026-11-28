package com.c2yz.notae.ui.library

import android.content.res.Configuration
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.Density
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.ui.assertAllClickablesLabelled
import com.c2yz.notae.ui.assertClickTargetsAtLeast48dp
import com.c2yz.notae.ui.assertInsideWindow
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** The library in a short window (a phone on its side): the slim rail, no side panel, composers behind a filter. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w640dp-h360dp-xhdpi")
class SlimLayoutTest {
    @get:Rule
    val compose = createComposeRule()

    private val actions = mutableListOf<Action>()

    private fun show(snapshot: Snapshot) {
        var density = Density(1f)
        compose.setContent {
            density = LocalDensity.current
            NotaeTheme { LibraryScreen(snapshot, { actions += it }) }
        }
        compose.assertAllClickablesLabelled()
        compose.assertClickTargetsAtLeast48dp(density)
    }

    @Test
    fun short_window_gets_the_slim_rail_not_the_side_panel() {
        show(Fixture.library())

        compose.onNodeWithTag(SLIM_RAIL_TAG).assertIsDisplayed()
        compose.onNodeWithTag(SIDE_PANEL_TAG).assertDoesNotExist()
        compose.onNodeWithTag(RAIL_TAG).assertDoesNotExist()
        compose.onNodeWithText("All composers").assertDoesNotExist()
    }

    @Test
    @Config(qualifiers = "w915dp-h412dp-xhdpi")
    fun short_wide_window_gets_the_slim_rail_too() {
        show(Fixture.library())

        compose.onNodeWithTag(SLIM_RAIL_TAG).assertIsDisplayed()
        compose.onNodeWithTag(SIDE_PANEL_TAG).assertDoesNotExist()
    }

    @Test
    fun slim_rail_items_are_labelled_and_settings_fits() {
        show(Fixture.library())

        compose.onNodeWithContentDescription("Favorites").assertIsDisplayed().performClick()
        compose.onNodeWithContentDescription("Settings").assertInsideWindow(compose)
        assertEquals(Action.SelectSection("favorites"), actions.last())
    }

    @Test
    @Config(qualifiers = "w640dp-h360dp-xhdpi", fontScale = 1.3f)
    fun slim_rail_fits_360dp_at_font_scale_1_3() {
        show(Fixture.library())

        compose.onNodeWithContentDescription("Settings").assertInsideWindow(compose)
    }

    @Test
    fun filter_opens_the_composer_sheet() {
        show(Fixture.library())

        compose.onNodeWithTag(FILTER_TAG).performClick()
        compose.onNodeWithText("Bach").performClick()

        assertEquals(Action.SelectSection("composer:bach"), actions.last())
    }

    @Test
    @Config(qualifiers = "w384dp-h832dp-xhdpi")
    fun composer_sheet_survives_rotation_to_landscape() {
        var landscape by mutableStateOf(false)
        compose.setContent {
            val base = LocalConfiguration.current
            val configuration = Configuration(base).apply {
                if (landscape) {
                    screenWidthDp = 832
                    screenHeightDp = 384
                }
            }
            CompositionLocalProvider(LocalConfiguration provides configuration) {
                NotaeTheme { LibraryScreen(Fixture.library(), { actions += it }) }
            }
        }
        compose.onNodeWithContentDescription("Choose composer, All composers").performClick()
        compose.onNodeWithText("Bach").assertIsDisplayed()

        landscape = true
        compose.waitForIdle()

        compose.onNodeWithTag(SLIM_RAIL_TAG).assertIsDisplayed()
        compose.onNodeWithText("Bach").assertIsDisplayed()
    }

    @Test
    @Config(qualifiers = "pl-w832dp-h384dp-xhdpi")
    fun polish_rail_is_labelled() {
        show(Fixture.library())

        compose.onNodeWithContentDescription("Wszystkie utwory").assertIsDisplayed()
        compose.onNodeWithContentDescription("Ustawienia").assertInsideWindow(compose)
    }

    private fun titleKeepsItsRoom() {
        show(Fixture.palmSunday())

        val title = compose.onNodeWithText("Niedziela Palmowa").fetchSemanticsNode().boundsInRoot
        assertTrue("title ${title.width} px", title.width >= HEADER_TITLE_MIN * compose.density.density - 0.5f)
        compose.onNodeWithTag(GRID_TAG).assertIsDisplayed()
    }

    @Test
    fun set_header_keeps_its_title() = titleKeepsItsRoom()

    @Test
    @Config(qualifiers = "w840dp-h600dp-xhdpi")
    fun set_header_keeps_its_title_at_840dp() = titleKeepsItsRoom()

    @Test
    @Config(qualifiers = "pl-w1280dp-h800dp-xhdpi")
    fun set_header_keeps_its_title_in_polish_on_a_tablet() = titleKeepsItsRoom()

    @Test
    fun overflowed_actions_stay_reachable() {
        show(Fixture.palmSunday())

        compose.onNodeWithTag(HEADER_OVERFLOW_TAG).performClick()
        compose.onNodeWithText("Scan library").performClick()

        assertEquals(Action.Rescan, actions.last())
    }

    @Test
    @Config(qualifiers = "w915dp-h412dp-xhdpi")
    fun set_header_with_notes_fits_a_short_window() {
        show(Fixture.palmWithNotes())

        compose.onNodeWithTag(GRID_TAG).assertIsDisplayed()
        compose.onNodeWithText("Niedziela Palmowa").assertInsideWindow(compose)
    }
}
