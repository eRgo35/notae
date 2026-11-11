package com.c2yz.notae.ui.settings

import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.withKeyDown
import androidx.compose.ui.unit.Density
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.ImportConfirmation
import com.c2yz.notae.core.ImportError
import com.c2yz.notae.core.ImportStatus
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.SetupScreen
import com.c2yz.notae.core.SharedSetReview
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.core.UnmatchedPiece
import com.c2yz.notae.nav.NotaeNavDisplay
import com.c2yz.notae.ui.assertAllClickablesLabelled
import com.c2yz.notae.ui.assertClickTargetsAtLeast48dp
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.setup.WelcomeScreen
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

/** Import sets: Settings' button and outcome, the confirmation the shell draws over Settings or the setup screen. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class ImportTest {
    @get:Rule
    val compose = createComposeRule()

    private val actions = mutableListOf<Action>()
    private var shown by mutableStateOf<Snapshot?>(null)

    /** The whole shell, as MainActivity draws it. */
    private fun show(snapshot: Snapshot, dark: Boolean = false) {
        shown = snapshot
        var density = Density(1f)
        compose.setContent {
            density = LocalDensity.current
            NotaeTheme(dark = dark) { shown?.let { NotaeNavDisplay(it, onAction = { action -> actions += action }, onBack = {}) } }
        }
        compose.waitForIdle()
        compose.assertAllClickablesLabelled()
        compose.assertClickTargetsAtLeast48dp(density)
    }

    @Test
    fun import_confirmation_names_replaced_sets() {
        show(confirming)

        compose.onNodeWithText("Import 3 sets?").assertIsDisplayed()
        compose.onNodeWithText("These sets will be replaced: Niedziela Palmowa, Pusty zestaw").assertIsDisplayed()
        compose.onNodeWithText("Import").performClick()

        assertEquals(listOf<Action>(Action.ImportConfirmed), actions)
    }

    @Test
    fun a_file_that_replaces_nothing_only_asks() {
        show(settings.copy(importConfirmation = ImportConfirmation(sets = 1u, replaced = emptyList(), shared = null)))

        compose.onNodeWithText("Import 1 set?").assertIsDisplayed()
        compose.onNodeWithText("These sets will be replaced", substring = true).assertDoesNotExist()
    }

    @Test
    fun escape_cancels_without_a_click_and_so_does_cancel() {
        show(confirming)

        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        compose.onNodeWithText("Cancel").performClick()

        assertEquals(listOf<Action>(Action.ImportCancelled, Action.ImportCancelled), actions)
    }

    @Test
    fun settings_keys_return_after_the_confirmation_closes() {
        show(confirming)
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }
        shown = settings
        compose.waitForIdle()

        // Nothing clicked: Ctrl K works again at once, as before the confirmation opened.
        compose.onRoot().performKeyInput { withKeyDown(Key.CtrlLeft) { pressKey(Key.K) } }

        assertEquals(listOf(Action.ImportCancelled, Action.OpenSearch), actions)
    }

    @Test
    fun the_confirmation_also_shows_over_the_setup_screen() {
        show(Fixture.base.copy(importConfirmation = ImportConfirmation(sets = 2u, replaced = emptyList(), shared = null)))

        compose.onNodeWithText("Import 2 sets?").assertIsDisplayed()
    }

    @Test
    fun import_sets_asks_for_a_file_and_shows_the_outcome() {
        show(settings)

        compose.onNodeWithText("Import sets").performScrollTo().performClick()
        assertEquals(listOf<Action>(Action.ImportRequested), actions)

        shown = settings.withImport(ImportStatus.Imported)
        compose.onNodeWithText("Favorites and sets imported").performScrollTo().assertIsDisplayed()
        mapOf(
            ImportError.NotNotae to "Could not import sets: this is not a Notae sets file",
            ImportError.UnsupportedVersion to "Could not import sets: this sets file is from a newer version of Notae",
            ImportError.Invalid to "Could not import sets: this sets file is damaged",
            ImportError.Unreadable("No such file") to "Could not import sets: No such file",
        ).forEach { (error, text) ->
            shown = settings.withImport(ImportStatus.Failed(error))
            compose.onNodeWithText(text).performScrollTo().assertIsDisplayed()
        }
    }

    @Test
    fun the_setup_screen_shows_why_an_import_failed() {
        show(Fixture.base.withImport(ImportStatus.Failed(ImportError.NotNotae)))

        compose.onNodeWithText("Could not import sets: this is not a Notae sets file").assertIsDisplayed()
    }

    @Test
    fun sharedSetReviewListsUnmatched() {
        show(reviewing(exists = true))

        compose.onNodeWithText("Import “Choir Mass”?").assertIsDisplayed()
        compose.onNodeWithText("2 of 4 scores matched").assertIsDisplayed()
        compose.onNodeWithText("Not in your library").assertIsDisplayed()
        compose.onNodeWithText("117").assertIsDisplayed()
        compose.onNodeWithText("Ojcze nasz").assertIsDisplayed()
        compose.onNodeWithText("Ave verum").assertIsDisplayed()
        compose.onNodeWithText("Import").assertDoesNotExist()
        compose.onNodeWithText("Replace").performClick()
        compose.onNodeWithText("Keep both").performClick()
        compose.onNodeWithText("Cancel").performClick()
        assertEquals(listOf(Action.ImportConfirmed, Action.ImportKeepBoth, Action.ImportCancelled), actions)

        actions.clear()
        shown = reviewing(exists = false)
        compose.waitForIdle()
        compose.onNodeWithText("Replace").assertDoesNotExist()
        compose.onNodeWithText("Keep both").assertDoesNotExist()
        compose.onNodeWithText("Import").performClick()
        assertEquals(listOf<Action>(Action.ImportConfirmed), actions)
    }

    // Screenshots (app/src/test/screenshots). No reference: 0.3.0 had no import.

    /** The screen and the confirmation as the shell draws them, with a fixed About (the shell's shows the build hash). */
    private fun shot(snapshot: Snapshot, name: String, dark: Boolean, qualifiers: String = "+w1280dp-h800dp-xhdpi") {
        RuntimeEnvironment.setQualifiers(qualifiers)
        compose.setContent {
            NotaeTheme(dark = dark) {
                Box {
                    when (val screen = snapshot.screen) {
                        is Screen.Setup -> WelcomeScreen(screen.setup, snapshot.importAvailable, {})
                        else -> SettingsScreen(snapshot, {}, AboutInfo(version = "0.5.0", build = "abc1234"))
                    }
                    snapshot.importConfirmation?.let { ImportConfirmationDialog(it) {} }
                }
            }
        }
        compose.waitForIdle()
        compose.onRoot().captureRoboImage("src/test/screenshots/$name-${if (dark) "dark" else "light"}.png")
    }

    @Test fun shot_confirm_light() = shot(confirming, "import-confirm-landscape", dark = false)

    @Test fun shot_confirm_dark() = shot(confirming, "import-confirm-landscape", dark = true)

    @Test fun shot_setup_confirm_light() = shot(setupConfirming, "setup-import-confirm-landscape", dark = false)

    @Test fun shot_setup_confirm_dark() = shot(setupConfirming, "setup-import-confirm-landscape", dark = true)

    @Test fun shot_shared_set_review_light() = shot(reviewing(exists = true), "shared-set-review-landscape", dark = false)

    @Test fun shot_shared_set_review_dark() = shot(reviewing(exists = true), "shared-set-review-landscape", dark = true)

    /** A phone in Polish, the longest labels: the three buttons stack so each is whole. */
    @Test fun shot_shared_set_review_phone_light() = shot(reviewing(exists = true), "shared-set-review-phone-pl", dark = false, qualifiers = PHONE_PL)

    @Test fun shot_shared_set_review_phone_dark() = shot(reviewing(exists = true), "shared-set-review-phone-pl", dark = true, qualifiers = PHONE_PL)

    private companion object {
        const val PHONE_PL = "+pl-w384dp-h832dp-xhdpi"

        val settings: Snapshot = Fixture.library().copy(
            screen = Screen.Settings,
            settings = Fixture.base.settings.copy(libraryName = "NotaeRef", indexedCount = 20u),
        )
        val confirming = settings.copy(
            importConfirmation = ImportConfirmation(sets = 3u, replaced = listOf("Niedziela Palmowa", "Pusty zestaw"), shared = null),
        )
        val setupConfirming = Fixture.base.copy(
            screen = Screen.Setup(SetupScreen.Unconfigured),
            importConfirmation = ImportConfirmation(sets = 3u, replaced = emptyList(), shared = null),
        )

        /** "Choir Mass": 2 of its 4 scores in the library; one missing piece has a number, one does not. */
        fun reviewing(exists: Boolean) = settings.copy(
            importConfirmation = ImportConfirmation(
                sets = 1u,
                replaced = if (exists) listOf("Choir Mass") else emptyList(),
                shared = SharedSetReview(
                    name = "Choir Mass",
                    matched = 2u,
                    unmatched = listOf(UnmatchedPiece(number = "117", title = "Ojcze nasz"), UnmatchedPiece(number = null, title = "Ave verum")),
                    exists = exists,
                ),
            ),
        )

        fun Snapshot.withImport(status: ImportStatus) = copy(settings = settings.copy(importStatus = status))
    }
}
