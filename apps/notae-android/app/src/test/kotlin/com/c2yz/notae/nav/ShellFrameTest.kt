package com.c2yz.notae.nav

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.activity.ComponentActivity
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** The shell keeps every screen clear of the system bars. */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class ShellFrameTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun content_stays_clear_of_the_system_bars() {
        WindowCompat.setDecorFitsSystemWindows(compose.activity.window, false)
        compose.setContent {
            NotaeTheme { Box(Modifier.shellFrame().fillMaxSize().testTag("content")) }
        }
        val bars = Insets.of(0, 48, 0, 96)
        compose.runOnUiThread {
            val view = compose.activity.window.decorView
            ViewCompat.dispatchApplyWindowInsets(
                view,
                WindowInsetsCompat.Builder().setInsets(WindowInsetsCompat.Type.systemBars(), bars).build(),
            )
        }
        compose.waitForIdle()

        val bounds = compose.onNodeWithTag("content").fetchSemanticsNode().boundsInRoot
        assertEquals(48f, bounds.top, 0.5f)
        assertEquals(800f * 2 - 48f - 96f, bounds.height, 0.5f)
    }
}
