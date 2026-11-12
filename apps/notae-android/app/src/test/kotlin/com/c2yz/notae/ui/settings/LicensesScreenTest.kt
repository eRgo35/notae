package com.c2yz.notae.ui.settings

import androidx.activity.ComponentActivity
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.hasScrollAction
import androidx.compose.ui.unit.Density
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Screen
import com.c2yz.notae.nav.NotaeNavDisplay
import com.c2yz.notae.ui.assertAllClickablesLabelled
import com.c2yz.notae.ui.assertClickTargetsAtLeast48dp
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.theme.NotaeTheme
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** The open-source licenses: the packaged notices, the back arrow, Escape and Back. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class LicensesScreenTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val actions = mutableListOf<Action>()

    private fun show() {
        var density = Density(1f)
        compose.setContent {
            density = LocalDensity.current
            NotaeTheme { LicensesScreen({ actions += it }) }
        }
        compose.waitForIdle()
        compose.assertAllClickablesLabelled()
        compose.assertClickTargetsAtLeast48dp(density)
    }

    @Test
    fun shows_the_packaged_notices() {
        show()

        compose.onNodeWithText("Open-source licenses").assertIsDisplayed()
        compose.onNodeWithText("Copyright (c) 2026 Notae contributors", substring = true).assertIsDisplayed()
        // The MPL-2.0 crates name where their source is.
        val uniffi = hasText("- uniffi 0.32.2: https://github.com/mozilla/uniffi-rs", substring = true)
        compose.onNode(hasScrollAction()).performScrollToNode(uniffi)
        compose.onNode(uniffi).assertIsDisplayed()
        compose.onNode(hasScrollAction()).performScrollToNode(hasText("Apache License 2.0", substring = true))
    }

    @Test
    fun back_arrow_and_escape_close_it() {
        show()

        compose.onNodeWithContentDescription("Back to settings").performClick()
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }

        assertEquals(listOf(Action.CloseLicenses, Action.CloseLicenses), actions)
    }

    @Test
    fun back_goes_to_the_core() {
        var backs = 0
        compose.setContent {
            NotaeTheme { NotaeNavDisplay(Fixture.library().copy(screen = Screen.Licenses), onAction = {}, onBack = { backs++ }) }
        }
        compose.onNodeWithText("Open-source licenses").assertIsDisplayed()

        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()

        assertEquals(1, backs)
    }

    // Screenshots (app/src/test/screenshots).

    private fun shot(landscape: Boolean, dark: Boolean) {
        RuntimeEnvironment.setQualifiers(if (landscape) "+w1280dp-h800dp-xhdpi" else "+w800dp-h1280dp-xhdpi")
        compose.setContent { NotaeTheme(dark = dark) { LicensesScreen({}) } }
        compose.waitForIdle()
        val orientation = if (landscape) "landscape" else "portrait"
        compose.onRoot().captureRoboImage("src/test/screenshots/licenses-$orientation-${if (dark) "dark" else "light"}.png")
    }

    @Test fun shot_landscape_light() = shot(landscape = true, dark = false)

    @Test fun shot_landscape_dark() = shot(landscape = true, dark = true)

    @Test fun shot_portrait_light() = shot(landscape = false, dark = false)

    @Test fun shot_portrait_dark() = shot(landscape = false, dark = true)
}
