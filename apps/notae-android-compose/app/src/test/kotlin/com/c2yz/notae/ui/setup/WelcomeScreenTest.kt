package com.c2yz.notae.ui.setup

import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.Density
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.SetupError
import com.c2yz.notae.core.SetupScreen
import com.c2yz.notae.ui.assertAllClickablesLabelled
import com.c2yz.notae.ui.assertClickTargetsAtLeast48dp
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

/** The three setup states: the Action each button sends, the labels, 48 dp targets and screenshots (app/src/test/screenshots). */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class WelcomeScreenTest {
    @get:Rule
    val compose = createComposeRule()

    private val actions = mutableListOf<Action>()

    private fun show(setup: SetupScreen, dark: Boolean = false, importAvailable: Boolean = false): Density {
        var density = Density(1f)
        compose.setContent {
            density = LocalDensity.current
            NotaeTheme(dark = dark) {
                WelcomeScreen(setup, importAvailable, { actions += it })
            }
        }
        compose.assertAllClickablesLabelled()
        compose.assertClickTargetsAtLeast48dp(density)
        return density
    }

    @Test
    fun unconfigured_pick_folder_emits_effect() {
        show(SetupScreen.Unconfigured)

        compose.onNodeWithText("Welcome to Notae").assertIsDisplayed()
        compose.onNodeWithText("Choose score folder").performClick()

        assertEquals(listOf<Action>(Action.PickLibraryRoot), actions)
    }

    @Test
    fun permission_lost_offers_pick_again() {
        show(SetupScreen.PermissionLost("Scores"))

        compose.onNodeWithText("Reconnect your library").assertIsDisplayed()
        compose.onNodeWithText("Notae no longer has access to “Scores”. Choose it again to reconnect your scores.").assertIsDisplayed()
        compose.onNodeWithText("Reconnect folder").performClick()

        assertEquals(listOf<Action>(Action.PickLibraryRoot), actions)
    }

    @Test
    fun error_shows_the_text_of_each_storage_error_above_the_button() {
        show(SetupScreen.Error(SetupError.Other("cannot open local index: sqlite error: file is not a database")))

        compose.onNodeWithText("storage error: cannot open local index: sqlite error: file is not a database").assertIsDisplayed()
        compose.onNodeWithText("Choose score folder").performClick()

        assertEquals(listOf<Action>(Action.PickLibraryRoot), actions)
    }

    @Test
    fun import_button_shows_only_when_available() {
        show(SetupScreen.Unconfigured, importAvailable = true)

        compose.onNodeWithText("Import from a previous Notae").performClick()

        // The same import as Settings' "Import sets".
        assertEquals(listOf<Action>(Action.ImportRequested), actions)
    }

    @Test
    fun import_button_is_hidden_otherwise() {
        show(SetupScreen.Unconfigured, importAvailable = false)

        compose.onNodeWithText("Import from a previous Notae").assertDoesNotExist()
    }

    private fun shot(state: String, setup: SetupScreen, landscape: Boolean, dark: Boolean) {
        RuntimeEnvironment.setQualifiers(if (landscape) "+w1280dp-h800dp-xhdpi" else "+w800dp-h1280dp-xhdpi")
        show(setup, dark)
        val orientation = if (landscape) "landscape" else "portrait"
        compose.onRoot().captureRoboImage("src/test/screenshots/setup-$state-$orientation-${if (dark) "dark" else "light"}.png")
    }

    private val unconfigured = SetupScreen.Unconfigured
    private val lost = SetupScreen.PermissionLost("Scores")
    private val failed = SetupScreen.Error(SetupError.Other("cannot open local index: sqlite error: file is not a database"))

    @Test
    fun shot_unconfigured_landscape_light() = shot("unconfigured", unconfigured, landscape = true, dark = false)

    @Test
    fun shot_unconfigured_landscape_dark() = shot("unconfigured", unconfigured, landscape = true, dark = true)

    @Test
    fun shot_unconfigured_portrait_light() = shot("unconfigured", unconfigured, landscape = false, dark = false)

    @Test
    fun shot_unconfigured_portrait_dark() = shot("unconfigured", unconfigured, landscape = false, dark = true)

    @Test
    fun shot_permission_lost_landscape_light() = shot("permission-lost", lost, landscape = true, dark = false)

    @Test
    fun shot_permission_lost_landscape_dark() = shot("permission-lost", lost, landscape = true, dark = true)

    @Test
    fun shot_permission_lost_portrait_light() = shot("permission-lost", lost, landscape = false, dark = false)

    @Test
    fun shot_permission_lost_portrait_dark() = shot("permission-lost", lost, landscape = false, dark = true)

    @Test
    fun shot_error_landscape_light() = shot("error", failed, landscape = true, dark = false)

    @Test
    fun shot_error_landscape_dark() = shot("error", failed, landscape = true, dark = true)

    @Test
    fun shot_error_portrait_light() = shot("error", failed, landscape = false, dark = false)

    @Test
    fun shot_error_portrait_dark() = shot("error", failed, landscape = false, dark = true)
}
