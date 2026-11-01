package com.c2yz.notae.ui.library

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.onRoot
import com.c2yz.notae.core.Banner
import com.c2yz.notae.core.Failure
import com.c2yz.notae.core.PdfOpenError
import com.c2yz.notae.core.Row
import com.c2yz.notae.core.SectionTitle
import com.c2yz.notae.core.Subtitle
import com.github.takahirom.roborazzi.captureRoboImage
import org.robolectric.RuntimeEnvironment
import androidx.compose.ui.unit.Density
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.LibraryView
import com.c2yz.notae.core.ScanSummary
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.core.StatusMessage
import com.c2yz.notae.ui.assertAllClickablesLabelled
import com.c2yz.notae.ui.assertClickTargetsAtLeast48dp
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** The library screen: the Action each control sends, what a crafted snapshot shows, labels and 48 dp targets. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class LibraryScreenTest {
    @get:Rule
    val compose = createComposeRule()

    private val actions = mutableListOf<Action>()
    private var shown by mutableStateOf<Snapshot?>(null)

    private fun show(snapshot: Snapshot) {
        shown = snapshot
        var density = Density(1f)
        compose.setContent {
            density = LocalDensity.current
            NotaeTheme {
                shown?.let { LibraryScreen(it, { action -> actions += action }) }
            }
        }
        checkTargets(density)
    }

    private fun checkTargets(density: Density) {
        compose.assertAllClickablesLabelled()
        compose.assertClickTargetsAtLeast48dp(density)
    }

    @Test
    fun grid_and_list_follow_library_view() {
        show(Fixture.library().withView(LibraryView.GRID))

        compose.onNodeWithTag(GRID_TAG).assertIsDisplayed()
        compose.onNodeWithTag(LIST_TAG).assertDoesNotExist()
        compose.onNodeWithContentDescription("Switch to list view").performClick()
        assertEquals(listOf<Action>(Action.SetLibraryView(LibraryView.LIST)), actions)

        shown = Fixture.library().withView(LibraryView.LIST)
        compose.waitForIdle()

        compose.onNodeWithTag(LIST_TAG).assertIsDisplayed()
        compose.onNodeWithTag(GRID_TAG).assertDoesNotExist()
        compose.onNodeWithContentDescription("Switch to grid view").performClick()
        assertEquals(LibraryView.GRID, (actions.last() as Action.SetLibraryView).view)
        compose.assertAllClickablesLabelled()
    }

    @Test
    fun overflow_sheet_offers_favorite_and_add_to_set() {
        val library = Fixture.library()
        val aveMaria = library.rows.first { it.title == "Ave Maria" }
        val agnus = library.rows.first { it.title == "Agnus Dei" }
        show(library)

        compose.onNodeWithContentDescription("More actions for Ave Maria").performClick()
        compose.onNodeWithText("Remove from favorites").assertIsDisplayed()
        compose.onNodeWithText("Open score").assertIsDisplayed()
        compose.onNodeWithText("Score details").assertIsDisplayed()
        compose.onNodeWithText("Edit details").assertIsDisplayed()
        compose.onNodeWithText("Remove from set").assertDoesNotExist()
        compose.assertAllClickablesLabelled()
        compose.onNodeWithText("Add to set…").performClick()

        assertEquals(listOf<Action>(Action.AddToSet(aveMaria.noteKey)), actions)
        compose.onNodeWithText("Add to set…").assertDoesNotExist()

        compose.onNodeWithContentDescription("More actions for Agnus Dei").performClick()
        compose.onNodeWithText("Add to favorites").performClick()

        assertEquals(Action.ToggleFavorite(agnus.noteKey), actions.last())
        compose.onNodeWithText("Add to favorites").assertDoesNotExist()
    }

    @Test
    fun hidden_scrollbar_lets_the_top_row_menu_open() {
        // The idle scrollbar's track sits over the ⋮ of the first list row; hidden, it must not take the tap.
        show(Fixture.library().withView(LibraryView.LIST))

        compose.onNodeWithContentDescription("More actions for Agnus Dei").performClick()

        compose.onNodeWithText("Open score").assertIsDisplayed()
    }

    @Test
    fun favorite_badge_shows() {
        val rows = Fixture.library().rows.filter { it.title in setOf("Agnus Dei", "Alleluja", "Ave Maria") }
        val small = Fixture.library().copy(rows = rows)
        show(small.withView(LibraryView.GRID))
        compose.onAllNodesWithTag(FAVORITE_BADGE_TAG, useUnmergedTree = true).assertCountEquals(1)

        shown = small.withView(LibraryView.LIST)
        compose.waitForIdle()
        compose.onAllNodesWithTag(FAVORITE_BADGE_TAG, useUnmergedTree = true).assertCountEquals(1)

        shown = small.copy(rows = rows.map { it.copy(favorite = false) })
        compose.waitForIdle()
        compose.onAllNodesWithTag(FAVORITE_BADGE_TAG, useUnmergedTree = true).assertCountEquals(0)
    }

    @Test
    fun banner_cancel_dispatches_cancel() {
        show(Fixture.library().copy(banner = Fixture.running, rescanEnabled = false))

        compose.onNodeWithText("Scanning library…").assertIsDisplayed()
        compose.onNodeWithText("Dismiss").assertDoesNotExist()
        compose.onNodeWithText("Cancel").performClick()
        assertEquals(listOf<Action>(Action.CancelRescan), actions)

        shown = Fixture.library().copy(banner = Fixture.done)
        compose.waitForIdle()
        compose.onNodeWithText("Finished: 20 added").assertIsDisplayed()
        compose.onNodeWithText("Cancel").assertDoesNotExist()
        compose.onNodeWithText("Dismiss").performClick()
        assertEquals(Action.AcknowledgeOperation, actions.last())
    }

    @Test
    fun header_and_sidebar_dispatch_their_actions() {
        show(Fixture.library())

        compose.onNodeWithContentDescription("Search").performClick()
        compose.onNodeWithContentDescription("Scan library").performClick()
        compose.onNodeWithContentDescription("Hide sidebar").performClick()
        compose.onNodeWithText("Favorites").performClick()
        // The side-panel row, not Ave Maria's card, which reads "Schubert" too.
        compose.onNode(hasText("Schubert") and hasText("1")).performClick()
        compose.onNodeWithText("Settings").performClick()

        assertEquals(
            listOf(
                Action.OpenSearch,
                Action.Rescan,
                Action.SetSidebarCollapsed(portrait = false, collapsed = true),
                Action.SelectSection("favorites"),
                Action.SelectSection("composer:schubert"),
                Action.OpenSettings,
            ),
            actions,
        )
    }

    @Test
    fun tapping_a_card_opens_the_pdf() {
        val library = Fixture.library()
        val agnus = library.rows.first()
        show(library)

        compose.onNodeWithContentDescription("Agnus Dei").performClick()

        assertEquals(listOf<Action>(Action.OpenPdf(agnus.relativePath, agnus.noteKey, null)), actions)
    }

    @Test
    fun empty_all_offers_a_scan() {
        show(Fixture.library().copy(rows = emptyList()))

        compose.onNodeWithText("A little space for your scores").assertIsDisplayed()
        compose.onNodeWithText("Scan your library folder to add the PDFs it contains.").assertIsDisplayed()
        compose.onNodeWithText("Scan library").performClick()

        assertEquals(listOf<Action>(Action.Rescan), actions)
    }

    @Test
    fun a_missing_piece_offers_only_removal() {
        val piece = Fixture.library().rows.first().copy(itemId = "item-8", position = 8u, missing = true, selectionId = "item-8")
        show(Fixture.library().copy(sectionKey = "set:0123", rows = listOf(piece)))

        compose.onNodeWithText("Missing from library").assertIsDisplayed()
        compose.onNodeWithContentDescription("More actions for Agnus Dei").performClick()
        compose.onNodeWithText("Open score").assertDoesNotExist()
        compose.onNodeWithText("Remove from set").performClick()

        assertEquals(listOf<Action>(Action.RemoveSetItem("item-8")), actions)
    }

    @Test
    fun banner_text_follows_the_slint_wording() {
        // Indexing uses the singular only for one PDF, as the Slint text does.
        show(Fixture.library().copy(banner = Fixture.running.copy(message = StatusMessage.Indexing(1u))))
        compose.onNodeWithText("Indexing 1 PDF…").assertIsDisplayed()
        shown = Fixture.library().copy(banner = Fixture.running.copy(message = StatusMessage.Indexing(20u)))
        compose.waitForIdle()
        compose.onNodeWithText("Indexing 20 PDFs…").assertIsDisplayed()
        shown = Fixture.library().copy(banner = Fixture.done.copy(message = StatusMessage.Finished(ScanSummary(0u, 0u, 0u, 0u, 0u, 0u))))
        compose.waitForIdle()
        compose.onNodeWithText("Finished: No changes").assertIsDisplayed()
    }

    // Screenshots (app/src/test/screenshots), named after their docs/compose-parity/slint/ references.

    private fun shot(state: String, snapshot: Snapshot, landscape: Boolean, dark: Boolean, before: () -> Unit = {}) {
        RuntimeEnvironment.setQualifiers(if (landscape) "+w1280dp-h800dp-xhdpi" else "+w800dp-h1280dp-xhdpi")
        shown = snapshot
        compose.setContent {
            NotaeTheme(dark = dark) { shown?.let { LibraryScreen(it, { action -> actions += action }) } }
        }
        before()
        compose.waitForIdle()
        val orientation = if (landscape) "landscape" else "portrait"
        compose.onRoot().captureRoboImage("src/test/screenshots/$state-$orientation-${if (dark) "dark" else "light"}.png")
    }

    private fun section(key: String, title: SectionTitle, rows: List<Row>) =
        Fixture.library().copy(sectionKey = key, title = title, subtitle = Subtitle.Scores(rows.size.toUInt()), rows = rows)

    private fun titled(vararg titles: String) = titles.map { title -> Fixture.rows.first { it.title == title } }

    private val all = Fixture.library()
    private val allList = all.withView(LibraryView.LIST)
    private val collapsed = all.copy(layout = all.layout.copy(sidebarCollapsedLandscape = true))
    private val composer = section("composer:schubert", SectionTitle.Composer("Schubert"), titled("Ave Maria"))
    private val favorites = section("favorites", SectionTitle.Favorites, titled("Te Deum laudamus", "Magnificat", "Ave Maria"))
    private val recent = section("recent", SectionTitle.Recent, titled("Alleluja", "Ave Maria"))
    private val emptySets = Fixture.library().copy(sectionKey = "sets", title = SectionTitle.Sets, subtitle = Subtitle.Sets(0u), rows = emptyList(), setCards = emptyList())
    private val failed = all.copy(
        banner = Banner(
            StatusMessage.Failed(
                Failure.EnumerationFailed(
                    "cannot list SAF children of document primary:Documents/NotaeRef: Null pointer in ContentResolver.query children cursor",
                ),
            ),
            busy = false,
            failed = true,
            fraction = null,
            cancelEnabled = false,
            acknowledgeEnabled = true,
        ),
        rescanEnabled = false,
    )
    private val done = all.copy(banner = Fixture.done.copy(message = StatusMessage.Finished(ScanSummary(0u, 0u, 0u, 0u, 0u, 0u))), rescanEnabled = false)
    private val running = all.copy(banner = Fixture.running, rescanEnabled = false)
    private val pdfError = all.copy(pdfOpenError = PdfOpenError.NoCompatibleViewer)
    private val openSheet = { compose.onNodeWithContentDescription("More actions for Ave Maria").performClick(); Unit }

    @Test fun shot_all_grid_landscape_light() = shot("library-all-grid", all, landscape = true, dark = false)

    @Test fun shot_all_grid_landscape_dark() = shot("library-all-grid", all, landscape = true, dark = true)

    @Test fun shot_all_grid_portrait_light() = shot("library-all-grid", all, landscape = false, dark = false)

    @Test fun shot_all_grid_portrait_dark() = shot("library-all-grid", all, landscape = false, dark = true)

    @Test fun shot_all_list_landscape_light() = shot("library-all-list", allList, landscape = true, dark = false)

    @Test fun shot_all_list_landscape_dark() = shot("library-all-list", allList, landscape = true, dark = true)

    @Test fun shot_all_list_portrait_light() = shot("library-all-list", allList, landscape = false, dark = false)

    @Test fun shot_all_list_portrait_dark() = shot("library-all-list", allList, landscape = false, dark = true)

    @Test fun shot_sidebar_collapsed_light() = shot("library-all-sidebar-collapsed", collapsed, landscape = true, dark = false)

    @Test fun shot_sidebar_collapsed_dark() = shot("library-all-sidebar-collapsed", collapsed, landscape = true, dark = true)

    @Test fun shot_composer_light() = shot("library-composer", composer, landscape = true, dark = false)

    @Test fun shot_composer_dark() = shot("library-composer", composer, landscape = true, dark = true)

    @Test fun shot_favorites_grid_light() = shot("library-favorites-grid", favorites, landscape = true, dark = false)

    @Test fun shot_favorites_grid_dark() = shot("library-favorites-grid", favorites, landscape = true, dark = true)

    @Test fun shot_favorites_list_light() = shot("library-favorites-list", favorites.withView(LibraryView.LIST), landscape = true, dark = false)

    @Test fun shot_favorites_list_dark() = shot("library-favorites-list", favorites.withView(LibraryView.LIST), landscape = true, dark = true)

    @Test fun shot_recent_grid_light() = shot("library-recent-grid", recent, landscape = true, dark = false)

    @Test fun shot_recent_grid_dark() = shot("library-recent-grid", recent, landscape = true, dark = true)

    @Test fun shot_recent_list_light() = shot("library-recent-list", recent.withView(LibraryView.LIST), landscape = true, dark = false)

    @Test fun shot_recent_list_dark() = shot("library-recent-list", recent.withView(LibraryView.LIST), landscape = true, dark = true)

    @Test fun shot_empty_all_light() = shot("library-empty-all", section("all", SectionTitle.All, emptyList()), landscape = true, dark = false)

    @Test fun shot_empty_all_dark() = shot("library-empty-all", section("all", SectionTitle.All, emptyList()), landscape = true, dark = true)

    @Test fun shot_empty_recent_light() = shot("library-empty-recent", section("recent", SectionTitle.Recent, emptyList()), landscape = true, dark = false)

    @Test fun shot_empty_recent_dark() = shot("library-empty-recent", section("recent", SectionTitle.Recent, emptyList()), landscape = true, dark = true)

    @Test fun shot_empty_favorites_light() = shot("library-empty-favorites", section("favorites", SectionTitle.Favorites, emptyList()), landscape = true, dark = false)

    @Test fun shot_empty_favorites_dark() = shot("library-empty-favorites", section("favorites", SectionTitle.Favorites, emptyList()), landscape = true, dark = true)

    @Test fun shot_empty_sets_light() = shot("library-empty-sets", emptySets, landscape = true, dark = false)

    @Test fun shot_empty_sets_dark() = shot("library-empty-sets", emptySets, landscape = true, dark = true)

    @Test fun shot_score_menu_light() = shot("sheet-score-menu", all, landscape = true, dark = false, before = openSheet)

    @Test fun shot_score_menu_dark() = shot("sheet-score-menu", all, landscape = true, dark = true, before = openSheet)

    @Test fun shot_banner_running_light() = shot("banner-running", running, landscape = true, dark = false)

    @Test fun shot_banner_running_dark() = shot("banner-running", running, landscape = true, dark = true)

    @Test fun shot_banner_done_light() = shot("banner-done", done, landscape = true, dark = false)

    @Test fun shot_banner_done_dark() = shot("banner-done", done, landscape = true, dark = true)

    @Test fun shot_banner_failed_light() = shot("banner-failed", failed, landscape = true, dark = false)

    @Test fun shot_banner_failed_dark() = shot("banner-failed", failed, landscape = true, dark = true)

    @Test fun shot_pdf_launch_error_light() = shot("pdf-launch-error", pdfError, landscape = true, dark = false)

    @Test fun shot_pdf_launch_error_dark() = shot("pdf-launch-error", pdfError, landscape = true, dark = true)

    private fun Snapshot.withView(view: LibraryView) = copy(layout = layout.copy(libraryView = view))
}
