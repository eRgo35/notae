package com.c2yz.notae.ui.viewer

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Viewer
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Notae's viewer: its bars, pages, set navigation, notes, hint and errors. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class ViewerScreenTest {
    @get:Rule
    val compose = createComposeRule()

    private val actions = mutableListOf<Action>()
    private var shown by mutableStateOf<Viewer?>(null)

    private fun show(viewer: Viewer) {
        shown = viewer
        compose.setContent {
            NotaeTheme { shown?.let { ViewerScreen(it, { action -> actions += action }) } }
        }
        compose.waitForIdle()
    }

    @Test
    fun viewer_back_button_dispatches_close() {
        show(Fixture.viewer())

        compose.onNodeWithText(Fixture.rows[0].title).assertIsDisplayed()
        compose.onNodeWithContentDescription("Back").performClick()

        assertEquals(listOf<Action>(Action.CloseViewer), actions)
    }
}
