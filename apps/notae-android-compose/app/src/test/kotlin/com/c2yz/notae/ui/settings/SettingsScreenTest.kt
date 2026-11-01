package com.c2yz.notae.ui.settings

import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.app.AppCompatDelegate
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.withKeyDown
import androidx.compose.ui.unit.Density
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.DocumentEntry
import com.c2yz.notae.core.Effect
import com.c2yz.notae.core.ExportStatus
import com.c2yz.notae.core.NotaeCore
import com.c2yz.notae.core.PlatformLibrary
import com.c2yz.notae.core.RecordingListener
import com.c2yz.notae.core.ScanSummary
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.SettingsInfo
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.platform.setAppLanguage
import com.c2yz.notae.ui.assertAllClickablesLabelled
import com.c2yz.notae.ui.assertClickTargetsAtLeast48dp
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.theme.NotaeTheme
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.nio.file.Files

/** The Settings screen: its cards from the snapshot, Change, Export, the language list and the keys. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class SettingsScreenTest {
    @get:Rule
    val compose = createComposeRule()

    private val actions = mutableListOf<Action>()
    private var shown by mutableStateOf<Snapshot?>(null)

    private fun show(snapshot: Snapshot, onAction: (Action) -> Unit = { actions += it }) {
        shown = snapshot
        var density = Density(1f)
        compose.setContent {
            density = LocalDensity.current
            NotaeTheme { shown?.let { SettingsScreen(it, onAction, ABOUT) } }
        }
        compose.waitForIdle()
        compose.assertAllClickablesLabelled()
        compose.assertClickTargetsAtLeast48dp(density)
    }

    @After
    fun followTheSystemAgain() = setAppLanguage("")

    @Test
    @Config(sdk = [30, 34])
    fun language_switch_dispatches_appcompat_locale() {
        // From API 33 AppCompat reaches Android's LocaleManager through a live AppCompat activity, as MainActivity is.
        Robolectric.buildActivity(AppCompatActivity::class.java).setup()
        setAppLanguage("")
        show(settings())
        compose.onNodeWithText("System default").assertIsSelected()

        compose.onNodeWithText("Polski").performScrollTo().performClick()

        assertEquals("pl", AppCompatDelegate.getApplicationLocales().toLanguageTags())
        compose.onNodeWithText("Polski").assertIsSelected()
        // Below API 33 AppCompat re-translates the screen at once; Robolectric's LocaleManager (API 33+) does not.
        val system = compose.onNode(hasText("System default") or hasText("Język systemu"))
        system.assertIsNotSelected()

        system.performScrollTo().performClick()

        assertTrue(AppCompatDelegate.getApplicationLocales().isEmpty)
        system.assertIsSelected()
        // The language is not the core's (R5): nothing was dispatched.
        assertEquals(emptyList<Action>(), actions)
    }

    @Test
    fun export_emits_write_export() {
        val listener = RecordingListener()
        val dir = Files.createTempDirectory("notae-settings").toFile()
        val core = NotaeCore(dir.path, NoPlatform, listener)
        try {
            show(settings()) { action ->
                actions += action
                core.dispatch(action)
            }

            compose.onNodeWithText("Export sets").performScrollTo().performClick()
            compose.waitForIdle()
            compose.waitUntil {
                listener.rethrow()
                listener.effects.isNotEmpty()
            }

            assertEquals(listOf<Action>(Action.ExportSets), actions)
            val export = listener.effects.single() as Effect.WriteExport
            assertTrue(export.suggestedName, Regex("""notae-sets-\d{4}-\d{2}-\d{2}\.json""").matches(export.suggestedName))
            assertTrue(export.json, export.json.contains("\"favorites\""))

            // The platform saved it: the core's next snapshot shows the outcome under the button.
            core.fileWritten(null)
            shown = settings().copy(settings = listener.snapshots.last().settings.copy(libraryName = "NotaeRef"))
            compose.onNodeWithText("Favorites and sets exported").performScrollTo().assertIsDisplayed()
            listener.rethrow()
        } finally {
            core.close()
            dir.deleteRecursively()
        }
    }

    @Test
    fun export_failure_names_the_reason() {
        show(settings(export = ExportStatus.Failed("disk full")))

        compose.onNodeWithText("Could not export sets: disk full").performScrollTo().assertIsDisplayed()
    }

    @Test
    fun change_and_back_dispatch_their_actions() {
        show(settings())

        compose.onNodeWithText("NotaeRef").assertIsDisplayed()
        compose.onNodeWithText("Change").performClick()
        compose.onNodeWithContentDescription("Back to library").performClick()

        assertEquals(listOf(Action.PickLibraryRoot, Action.CloseSettings), actions)
    }

    @Test
    fun maintenance_shows_counts_and_the_last_scan() {
        show(settings())
        compose.onNodeWithText("No scan this session").assertIsDisplayed()
        compose.onNodeWithText("ID conflicts").assertDoesNotExist()

        shown = settings(scan = ScanSummary(added = 3u, updated = 1u, moved = 0u, deleted = 2u, conflicts = 1u, failures = 0u))
        compose.waitForIdle()
        compose.onNodeWithText("3 added, 1 updated, 0 moved, 2 deleted").assertIsDisplayed()
        compose.onNodeWithText("ID conflicts").assertIsDisplayed()
        compose.onNodeWithText("Failures").assertIsDisplayed()
    }

    @Test
    fun about_shows_the_build() {
        show(settings())

        compose.onNodeWithText("0.5.0").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("abc1234").assertIsDisplayed()
        compose.onNodeWithText("MIT License").assertIsDisplayed()
        compose.onNodeWithText("https://github.com/eRgo35/notae").assertIsDisplayed()
    }

    @Test
    fun ctrl_k_and_escape_work_without_a_click() {
        show(settings())

        // Nothing clicked first: Settings has the keys as soon as it shows, as in Slint's root shortcut scope.
        compose.onRoot().performKeyInput { withKeyDown(Key.CtrlLeft) { pressKey(Key.K) } }
        compose.onRoot().performKeyInput { pressKey(Key.Escape) }

        assertEquals(listOf(Action.OpenSearch, Action.CloseSettings), actions)
    }

    @Test
    fun the_banner_shows_here_too() {
        show(settings().copy(banner = Fixture.running))

        compose.onNodeWithText("Scanning library…").assertIsDisplayed()
    }

    // Screenshots (app/src/test/screenshots), named after their docs/compose-parity/slint/ references.

    private fun shot(state: String, snapshot: Snapshot, landscape: Boolean, dark: Boolean, scrollTo: String? = null) {
        RuntimeEnvironment.setQualifiers(if (landscape) "+w1280dp-h800dp-xhdpi" else "+w800dp-h1280dp-xhdpi")
        shown = snapshot
        compose.setContent { NotaeTheme(dark = dark) { shown?.let { SettingsScreen(it, { action -> actions += action }, ABOUT) } } }
        compose.waitForIdle()
        scrollTo?.let { compose.onNodeWithText(it).performScrollTo() }
        compose.waitForIdle()
        val orientation = if (landscape) "landscape" else "portrait"
        compose.onRoot().captureRoboImage("src/test/screenshots/$state-$orientation-${if (dark) "dark" else "light"}.png")
    }

    @Test fun shot_landscape_light() = shot("settings", settings(), landscape = true, dark = false)

    @Test fun shot_landscape_dark() = shot("settings", settings(), landscape = true, dark = true)

    @Test fun shot_portrait_light() = shot("settings", settings(), landscape = false, dark = false)

    @Test fun shot_portrait_dark() = shot("settings", settings(), landscape = false, dark = true)

    @Test fun shot_export_done_landscape_light() =
        shot("settings-export-done", settings(export = ExportStatus.Exported), landscape = true, dark = false, scrollTo = EXPORTED)

    @Test fun shot_export_done_landscape_dark() =
        shot("settings-export-done", settings(export = ExportStatus.Exported), landscape = true, dark = true, scrollTo = EXPORTED)

    private object NoPlatform : PlatformLibrary {
        override fun checkRoot(handle: String) = error("no library folder")

        override fun enumerate(handle: String): List<DocumentEntry> = error("no library folder")

        override fun extractTextLayer(handle: String, path: String): String? = error("no library folder")

        override fun renderThumbnail(handle: String, path: String, outPng: String): Boolean = error("no library folder")
    }

    private companion object {
        const val EXPORTED = "Favorites and sets exported"
        val ABOUT = AboutInfo(version = "0.5.0", build = "abc1234")

        /** The parity fixture's Settings: the NotaeRef folder with its 20 scores, before any scan this session. */
        fun settings(scan: ScanSummary? = null, export: ExportStatus? = null): Snapshot = Fixture.library().copy(
            screen = Screen.Settings,
            banner = Fixture.library().banner.copy(message = null),
            settings = SettingsInfo(libraryName = "NotaeRef", indexedCount = 20u, noTextCount = 0u, latestScan = scan, export = export),
        )
    }
}
