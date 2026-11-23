package com.c2yz.notae.ui.viewer

import android.content.res.Configuration
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** The viewer's notes: a side panel on a wide window, a bottom sheet otherwise. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class NotesPanelTest {
    @get:Rule
    val compose = createComposeRule()

    private var widthDp by mutableIntStateOf(1280)

    private fun show() {
        compose.setContent {
            val base = LocalConfiguration.current
            val configuration = Configuration(base).apply {
                screenWidthDp = widthDp
                orientation = if (widthDp > screenHeightDp) Configuration.ORIENTATION_LANDSCAPE else Configuration.ORIENTATION_PORTRAIT
            }
            CompositionLocalProvider(LocalConfiguration provides configuration) {
                NotaeTheme { ViewerScreen(Fixture.viewer(), {}, { _, _ -> FakePages(3) }) }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun notes_open_as_a_side_panel() {
        show()
        compose.onNodeWithContentDescription("Notes").performClick()
        compose.waitForIdle()
        compose.onNodeWithContentDescription("Close notes").assertIsDisplayed()
        compose.onNodeWithText("Add score note").assertIsDisplayed()
        compose.onNodeWithText("1 / 3").assertIsDisplayed()
    }

    @Test
    fun notes_button_closes_the_panel() {
        show()
        compose.onNodeWithContentDescription("Notes").performClick()
        compose.waitForIdle()
        compose.onNodeWithContentDescription("Notes").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Add score note").assertDoesNotExist()
        compose.onNodeWithContentDescription("Notes").performClick()
        compose.onNodeWithContentDescription("Close notes").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Add score note").assertDoesNotExist()
    }

    @Test
    fun notes_open_as_a_sheet_when_narrower() {
        widthDp = 600
        show()
        compose.onNodeWithContentDescription("Notes").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Add score note").assertIsDisplayed()
        compose.onNodeWithContentDescription("Close notes").assertDoesNotExist()
    }

    @Test
    fun panel_becomes_sheet_when_narrower() {
        show()
        compose.onNodeWithContentDescription("Notes").performClick()
        compose.waitForIdle()
        widthDp = 600
        compose.waitForIdle()
        compose.onNodeWithContentDescription("Close notes").assertDoesNotExist()
        compose.onNodeWithText("Add score note").assertIsDisplayed()
    }
}
