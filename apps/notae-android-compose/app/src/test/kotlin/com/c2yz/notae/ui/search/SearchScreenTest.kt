package com.c2yz.notae.ui.search

import android.content.res.Configuration
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextInputSelection
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.withKeyDown
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.unit.Density
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Banner
import com.c2yz.notae.core.Row
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.SearchBox
import com.c2yz.notae.core.SearchScope
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.core.Snippet
import com.c2yz.notae.ui.assertAllClickablesLabelled
import com.c2yz.notae.ui.assertClickTargetsAtLeast48dp
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.library.LibraryScreen
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

/** The search screen: the field and chips send their actions, results show the core's snippet parts, and the keys. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class SearchScreenTest {
    @get:Rule
    val compose = createComposeRule()

    private val actions = mutableListOf<Action>()
    private var shown by mutableStateOf<Snapshot?>(null)
    private val field get() = compose.onNodeWithContentDescription(PLACEHOLDER)

    private fun show(snapshot: Snapshot, voiceAvailable: Boolean = true) {
        shown = snapshot
        var density = Density(1f)
        compose.setContent {
            density = LocalDensity.current
            NotaeTheme { shown?.let { SearchScreen(it, { action -> actions += action }, voiceAvailable) } }
        }
        compose.waitForIdle()
        compose.assertAllClickablesLabelled()
        compose.assertClickTargetsAtLeast48dp(density)
    }

    @Test
    fun typing_dispatches_query() {
        show(search(""))

        // The field has the focus (and so the keyboard) as soon as search opens.
        field.assertIsFocused()
        field.performTextInput("l")
        shown = search("l")
        field.performTextInput("y")
        shown = search("ly")
        compose.waitForIdle()

        assertEquals(listOf<Action>(Action.SearchChanged("l"), Action.SearchChanged("ly")), actions)
        assertEquals("ly", text())
    }

    @Test
    fun a_late_echo_keeps_the_typed_text_and_the_cursor() {
        show(search("lyrics", results))

        // A snapshot with the same query (new results) leaves the cursor where the user put it.
        field.performTextInputSelection(TextRange(2))
        shown = search("lyrics", results.take(2))
        compose.waitForIdle()
        assertEquals(TextRange(2), selection())

        // Typing faster than the snapshots: the echo of the first key arrives after the second key.
        field.performTextInput("X")
        field.performTextInput("Y")
        shown = search("lyXrics")
        compose.waitForIdle()
        assertEquals("lyXYrics", text())
        assertEquals(TextRange(4), selection())
        shown = search("lyXYrics")
        compose.waitForIdle()
        assertEquals("lyXYrics", text())
        assertEquals(listOf<Action>(Action.SearchChanged("lyXrics"), Action.SearchChanged("lyXYrics")), actions)

        // A query the user did not type (a voice result) replaces the text, with the cursor at its end.
        shown = search("ave maria")
        compose.waitForIdle()
        assertEquals("ave maria", text())
        assertEquals(TextRange(9), selection())
    }

    @Test
    fun scope_chip_dispatches_scope() {
        show(search("lyrics", results))

        compose.onNodeWithText("All").assertIsSelected()
        compose.onNodeWithText("Lyrics").assertIsNotSelected().performClick()
        compose.onNodeWithText("Composers").performClick()

        assertEquals(listOf<Action>(Action.SearchScopeChanged(SearchScope.TEXT), Action.SearchScopeChanged(SearchScope.COMPOSERS)), actions)
        shown = search("lyrics", results, SearchScope.TEXT)
        compose.waitForIdle()
        compose.onNodeWithText("Lyrics").assertIsSelected()
        compose.onNodeWithText("All").assertIsNotSelected()
    }

    @Test
    fun ctrl_k_opens_search() {
        compose.setContent { NotaeTheme { LibraryScreen(Fixture.library(), { action -> actions += action }) } }
        compose.waitForIdle()

        // Nothing clicked first: the library has the keys as soon as it shows.
        compose.onRoot().performKeyInput { withKeyDown(Key.CtrlLeft) { pressKey(Key.K) } }

        assertEquals(listOf<Action>(Action.OpenSearch), actions)

        // While the ⋮ sheet is open, Slint ignores it.
        compose.onNodeWithContentDescription("More actions for Ave Maria").performClick()
        compose.onRoot().performKeyInput { withKeyDown(Key.CtrlLeft) { pressKey(Key.K) } }
        assertEquals(listOf<Action>(Action.OpenSearch), actions)
    }

    @Test
    fun escape_closes_search() {
        show(search("lyrics", results))

        compose.onRoot().performKeyInput { pressKey(Key.Escape) }

        assertEquals(listOf<Action>(Action.CloseSearch), actions)
    }

    @Test
    fun results_show_the_snippet_and_open_the_details() {
        show(search("lyrics", results))

        compose.onNodeWithContentDescription("Barka").assertIsDisplayed()
        compose.onNodeWithText("Barka lyrics line 8").assertIsDisplayed()
        compose.onNodeWithContentDescription("Barka").performClick()

        assertEquals(listOf<Action>(Action.SearchResultChosen(results[1].noteKey)), actions)
    }

    @Test
    fun clear_back_and_voice_dispatch_their_actions() {
        show(search("lyrics", results))

        compose.onNodeWithContentDescription("Search by voice").performClick()
        compose.onNodeWithContentDescription("Back to library").performClick()
        compose.onNodeWithContentDescription("Clear search").performClick()

        assertEquals(listOf(Action.VoiceSearch, Action.CloseSearch, Action.SearchChanged("")), actions)
        assertEquals("", text())
        field.assertIsFocused()
    }

    @Test
    fun no_microphone_without_a_recognizer() {
        show(search(""), voiceAvailable = false)

        compose.onNodeWithContentDescription("Search by voice").assertDoesNotExist()
    }

    @Test
    fun empty_and_no_results_texts() {
        show(search(""))
        compose.onNodeWithText("Search your library").assertIsDisplayed()
        compose.onNodeWithText("Find scores by title, composer, tag or lyrics.").assertIsDisplayed()

        shown = search("zzzz")
        compose.waitForIdle()
        compose.onNodeWithText("No scores found").assertIsDisplayed()
        compose.onNodeWithText("Try another word, or switch the scope above.").assertIsDisplayed()
    }

    @Test
    fun the_banner_shows_here_too() {
        show(search("lyrics", results).copy(banner = Fixture.running))

        compose.onNodeWithText("Scanning library…").assertIsDisplayed()
    }

    private fun text(): String = field.fetchSemanticsNode().config[SemanticsProperties.EditableText].text

    private fun selection(): TextRange = field.fetchSemanticsNode().config[SemanticsProperties.TextSelectionRange]

    // Screenshots (app/src/test/screenshots), named after their docs/compose-parity/slint/ references.

    private fun shot(state: String, snapshot: Snapshot, landscape: Boolean, dark: Boolean) {
        // The reference emulator has a hardware keyboard, so the empty field shows the "Ctrl K" hint.
        RuntimeEnvironment.setQualifiers(if (landscape) "+w1280dp-h800dp-xhdpi" else "+w800dp-h1280dp-xhdpi")
        shown = snapshot
        compose.setContent {
            val keyboard = Configuration(LocalConfiguration.current).apply {
                this.keyboard = Configuration.KEYBOARD_QWERTY
                hardKeyboardHidden = Configuration.HARDKEYBOARDHIDDEN_NO
            }
            CompositionLocalProvider(LocalConfiguration provides keyboard) {
                NotaeTheme(dark = dark) { shown?.let { SearchScreen(it, { action -> actions += action }, voiceAvailable = true) } }
            }
        }
        compose.waitForIdle()
        val orientation = if (landscape) "landscape" else "portrait"
        compose.onRoot().captureRoboImage("src/test/screenshots/$state-$orientation-${if (dark) "dark" else "light"}.png")
    }

    @Test fun shot_empty_landscape_light() = shot("search-empty", search(""), landscape = true, dark = false)

    @Test fun shot_empty_landscape_dark() = shot("search-empty", search(""), landscape = true, dark = true)

    @Test fun shot_results_landscape_light() = shot("search-results", search("lyrics", results), landscape = true, dark = false)

    @Test fun shot_results_landscape_dark() = shot("search-results", search("lyrics", results), landscape = true, dark = true)

    @Test fun shot_results_portrait_light() = shot("search-results", search("lyrics", results), landscape = false, dark = false)

    @Test fun shot_results_portrait_dark() = shot("search-results", search("lyrics", results), landscape = false, dark = true)

    @Test fun shot_no_results_landscape_light() = shot("search-no-results", search("zzzz"), landscape = true, dark = false)

    @Test fun shot_no_results_landscape_dark() = shot("search-no-results", search("zzzz"), landscape = true, dark = true)

    private companion object {
        const val PLACEHOLDER = "Search titles, composers, tags or lyrics"

        /**
         * "lyrics" in All, in the order of search-results-portrait: each PDF's one line is "<title> lyrics line <N>",
         * and the core sends it split around the match.
         */
        val results: List<Row> = listOf(
            "Alleluja" to 5, "Barka" to 8, "Bogurodzica" to 1, "Sanctus" to 6, "Magnificat" to 13, "Agnus Dei" to 7,
            "Panis angelicus" to 16, "Salve Regina" to 17, "Ubi caritas" to 19, "Zdrowaś Maryjo" to 11, "Ave Maria" to 0,
            "Ave verum corpus" to 18, "Gloria in excelsis" to 4, "Kyrie eleison" to 3,
        ).map { (title, line) ->
            Fixture.rows.first { it.title == title }.copy(snippet = Snippet(before = "$title ", hit = "lyrics", after = " line $line"))
        }

        fun search(query: String, rows: List<Row> = emptyList(), scope: SearchScope = SearchScope.EVERYTHING): Snapshot =
            Fixture.library().copy(screen = Screen.Search, search = SearchBox(query, scope), rows = rows, banner = Banner(null, false, false, null, false, false))
    }
}
