package com.c2yz.notae.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.Density
import com.c2yz.notae.core.LibraryView
import com.c2yz.notae.core.MetadataDraft
import com.c2yz.notae.core.PickerRow
import com.c2yz.notae.core.AddPicker
import com.c2yz.notae.core.PositionOf
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.SearchBox
import com.c2yz.notae.core.SearchScope
import com.c2yz.notae.core.Selection
import com.c2yz.notae.core.SettingsInfo
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.core.Snippet
import com.c2yz.notae.nav.NotaeNavDisplay
import com.c2yz.notae.nav.shellFrame
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.settings.AboutInfo
import com.c2yz.notae.ui.settings.SettingsScreen
import com.c2yz.notae.ui.theme.NotaeTheme
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.ParameterizedRobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Every key screen at every window of the size matrix ([Windows.MATRIX]), rendered through the app's navigation so the
 * chrome and two-pane rules apply (Settings, which has neither, renders alone with a fixed build): `matrix-<state>-<window>[-pl|-fs13|-dark].png`. Pairs the per-screen screenshot tests
 * already capture at 1280×800 and 800×1280 are skipped ([SKIP]). Every capture also checks labels and 48 dp targets.
 */
@RunWith(ParameterizedRobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class LayoutMatrixScreenshotTest(private val state: String, private val window: String, private val variant: String) {
    @get:Rule
    val compose = createComposeRule()

    @Test
    fun capture() {
        val qualifiers = Windows.MATRIX.getValue(window)
        RuntimeEnvironment.setQualifiers(if (variant == "pl") "+pl-" + qualifiers.removePrefix("+") else qualifiers)
        if (variant == "fs13") RuntimeEnvironment.setFontScale(1.3f)
        val snapshot = STATES.getValue(state)()
        var density = Density(1f)
        compose.setContent {
            density = LocalDensity.current
            NotaeTheme(dark = variant == "dark") {
                // Settings through the app shows this build's commit; a fixed one keeps the capture the same across commits.
                if (state == "settings") {
                    Box(Modifier.shellFrame().fillMaxSize()) { SettingsScreen(snapshot, {}, AboutInfo(version = "0.8.0", build = "abc1234")) }
                } else {
                    NotaeNavDisplay(snapshot, onAction = {}, onBack = {})
                }
            }
        }
        compose.waitForIdle()
        compose.assertAllClickablesLabelled()
        compose.assertClickTargetsAtLeast48dp(density)
        val suffix = if (variant.isEmpty()) "" else "-$variant"
        compose.onRoot().captureRoboImage("src/test/screenshots/matrix-$state-$window$suffix.png")
    }

    companion object {
        private val settings = Fixture.library().copy(
            screen = Screen.Settings,
            settings = SettingsInfo(libraryName = "NotaeRef", indexedCount = 20u, noTextCount = 0u, latestScan = null, export = null, importStatus = null),
        )

        private fun selecting(from: Snapshot, index: Int): Snapshot = from.copy(
            selection = Selection(
                row = from.rows[index],
                previous = from.rows.getOrNull(index - 1)?.selectionId,
                next = from.rows.getOrNull(index + 1)?.selectionId,
                position = PositionOf((index + 1).toUInt(), from.rows.size.toUInt()),
            ),
        )

        private fun searchResults(): Snapshot = Fixture.library().copy(
            screen = Screen.Search,
            search = SearchBox("lyrics", SearchScope.EVERYTHING),
            rows = Fixture.rows.take(10).mapIndexed { line, row -> row.copy(snippet = Snippet("${row.title} ", "lyrics", " line $line")) },
        )

        private fun addPicker(): Snapshot {
            val inPalm = Fixture.palmRows.map { it.title }.toSet()
            val rows = Fixture.rows.map { PickerRow(it.noteKey, it.title, it.number, it.composer, it.title in inPalm, false) }
            return Fixture.palmSunday().copy(addPicker = AddPicker("", rows, 0u))
        }

        /** The key states, as the per-screen tests build them. */
        private val STATES: Map<String, () -> Snapshot> = linkedMapOf(
            "library-grid" to { Fixture.library() },
            "library-list" to { Fixture.library().let { it.copy(layout = it.layout.copy(libraryView = LibraryView.LIST)) } },
            "set-grid" to { Fixture.palmSunday() },
            "sets" to { Fixture.sets() },
            "details-library" to { selecting(Fixture.library(), 2) },
            "details-set" to { selecting(Fixture.palmSunday(), 1) },
            "search-results" to { searchResults() },
            "settings" to { settings },
            "metadata-editor" to {
                Fixture.library().copy(metadataEditor = MetadataDraft("key-2", "", "Ave Maria", "Schubert", emptyList(), null))
            },
            "add-picker" to { addPicker() },
            "welcome" to { Fixture.base },
        )

        private const val TABLET_LANDSCAPE = "w1280dp-h800dp"
        private const val TABLET_PORTRAIT = "w800dp-h1280dp"

        /** State × window pairs the per-screen screenshot tests already capture (library-all-grid-landscape-light, …). */
        private val SKIP: Set<Pair<String, String>> =
            STATES.keys.filter { it != "metadata-editor" && it != "add-picker" }.flatMap { listOf(it to TABLET_LANDSCAPE, it to TABLET_PORTRAIT) }.toSet() +
                setOf("metadata-editor" to TABLET_LANDSCAPE, "add-picker" to TABLET_LANDSCAPE)

        /** Polish and font scale 1.3 for the header-heavy states, where long labels bite first. */
        private val LONG_LABEL_STATES = listOf("library-grid", "set-grid", "settings")
        private val POLISH = listOf("w360dp-h640dp", "w832dp-h384dp", "w840dp-h600dp", TABLET_LANDSCAPE, TABLET_PORTRAIT)
        private val FONT_SCALE = listOf("w384dp-h832dp", TABLET_LANDSCAPE)
        private val DARK = listOf("w384dp-h832dp", TABLET_LANDSCAPE)

        @JvmStatic
        @ParameterizedRobolectricTestRunner.Parameters(name = "{0}-{1}{2}")
        fun cases(): List<Array<Any>> = buildList {
            for (state in STATES.keys) {
                for (window in Windows.MATRIX.keys) if (state to window !in SKIP) add(arrayOf<Any>(state, window, ""))
            }
            for (state in LONG_LABEL_STATES) {
                POLISH.forEach { add(arrayOf<Any>(state, it, "pl")) }
                FONT_SCALE.forEach { add(arrayOf<Any>(state, it, "fs13")) }
            }
            DARK.forEach { add(arrayOf<Any>("library-grid", it, "dark")) }
        }
    }
}
