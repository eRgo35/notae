package com.c2yz.notae.nav

import androidx.activity.BackEventCompat
import androidx.activity.ComponentActivity
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.graphics.Color
import androidx.core.view.drawToBitmap
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import com.c2yz.notae.core.PositionOf
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.SectionTitle
import com.c2yz.notae.core.Selection
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.ui.details.DETAILS_BODY_TAG
import com.c2yz.notae.ui.library.CHIPS_TAG
import com.c2yz.notae.ui.library.CHIPS_FADE_MILLIS
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.library.GRID_TAG
import com.c2yz.notae.ui.library.SIDE_PANEL_TAG
import com.c2yz.notae.ui.theme.NotaeTheme
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Only content moves when the screen changes: halfway through a change (the clock paused), the rail or the side panel,
 * the bottom bar and the top bars sit where they sit at rest, while the content is off its place. Tablet landscape (two
 * panes), tablet portrait and phone; the core stands in as the snapshot.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class ScreenMotionTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private var snapshot by mutableStateOf(Fixture.library())

    /** What the core shows once Back reaches it; nothing changes when `null`. */
    private var afterBack: Snapshot? = null
    private var backed = false

    private fun show(first: Snapshot) {
        snapshot = first
        compose.setContent { NotaeTheme { NotaeNavDisplay(snapshot, onAction = {}, onBack = { backed = true }) } }
        compose.waitForIdle()
    }

    /**
     * The Back key as 3-button navigation on Android 16 sends it: a back gesture that starts and ends at once, through the
     * activity's dispatcher. The core answers on its own thread, a frame later. Then [steps] steps of [STEP_MILLIS].
     */
    private fun pressBack(steps: Int) {
        compose.mainClock.autoAdvance = false
        val dispatcher = compose.activity.onBackPressedDispatcher
        fun frame() {
            compose.mainClock.advanceTimeByFrame()
            compose.waitForIdle()
        }
        compose.runOnUiThread { dispatcher.dispatchOnBackStarted(BackEventCompat(0f, 0f, 0f, BackEventCompat.EDGE_LEFT)) }
        frame()
        compose.runOnUiThread { dispatcher.dispatchOnBackProgressed(BackEventCompat(0f, 0f, 0f, BackEventCompat.EDGE_LEFT)) }
        frame()
        compose.runOnUiThread { dispatcher.onBackPressed() }
        frame()
        if (backed) afterBack?.let { snapshot = it }
        steps(steps)
    }

    /** How many sampled pixels of [area] differ between two frames. */
    private fun differing(a: android.graphics.Bitmap, b: android.graphics.Bitmap, area: Rect): Int {
        var count = 0
        for (x in area.left.toInt() until area.right.toInt() step 4) {
            for (y in area.top.toInt().coerceAtLeast(0) until area.bottom.toInt() step 4) {
                if (a.getPixel(x, y) != b.getPixel(x, y)) count++
            }
        }
        return count
    }

    /** Sends [next] and stops the clock early in the 300 ms change, while everything that moves is well off its place. */
    private fun halfway(next: Snapshot) {
        compose.mainClock.autoAdvance = false
        snapshot = next
        // In steps, each one composing and laying out: the change starts on the frame after the one that composes it.
        repeat(HALFWAY_STEPS) {
            compose.mainClock.advanceTimeBy(STEP_MILLIS)
            compose.waitForIdle()
        }
    }

    /** Sends [next] and lets the clock run [steps] steps of [STEP_MILLIS], each composing and laying out. */
    private fun steps(steps: Int, next: Snapshot? = null) {
        compose.mainClock.autoAdvance = false
        if (next != null) snapshot = next
        repeat(steps) {
            compose.mainClock.advanceTimeBy(STEP_MILLIS)
            compose.waitForIdle()
        }
    }

    private fun finish() {
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()
    }

    /**
     * Where each [matcher] node shows. Moving content is clipped to its own place, so content leaving toward the start
     * shows as its end edge moving, and content toward the end as its start edge.
     */
    private fun bounds(matcher: SemanticsMatcher): List<Rect> =
        compose.onAllNodes(matcher, useUnmergedTree = true).fetchSemanticsNodes().map { it.boundsInRoot }

    /** The one [matcher] node's laid-out size, however much of it shows. */
    private fun size(matcher: SemanticsMatcher): IntSize = compose.onAllNodes(matcher, useUnmergedTree = true).fetchSemanticsNodes().single().size

    /** Every [matcher] node in the middle of the change sits where one sits at rest, before or after it. */
    private fun assertStill(matcher: SemanticsMatcher, rest: List<Rect>, middle: List<Rect>) {
        assertTrue("$matcher shows in the middle of the change", middle.isNotEmpty())
        middle.forEach { assertTrue("$matcher at $it, at rest only at $rest", it in rest) }
    }

    private val rows = Fixture.rows
    private val ave = rows.indexOfFirst { it.title == "Ave Maria" }

    private fun selecting(index: Int, from: Snapshot = Fixture.library()): Snapshot = from.copy(
        selection = Selection(
            row = rows[index],
            previous = rows.getOrNull(index - 1)?.selectionId,
            next = rows.getOrNull(index + 1)?.selectionId,
            position = PositionOf((index + 1).toUInt(), rows.size.toUInt()),
        ),
    )

    private val search = hasContentDescription("Search")
    private val favorites = hasText("Favorites")
    private val sidePanel = hasTestTag(SIDE_PANEL_TAG)
    private val chips = hasTestTag(CHIPS_TAG)
    private val sets = hasText("Sets")
    private val grid = hasTestTag(GRID_TAG)
    private val palmCard = hasContentDescription("More actions for Niedziela Palmowa")
    private val detailsBack = hasContentDescription("Back to library")
    private val settingsTitle = hasText("Settings")
    private val libraryFolder = hasText("Library folder")

    @Test
    @Config(qualifiers = "w1280dp-h800dp-xhdpi")
    fun landscape_section_switch_moves_only_the_content() {
        show(Fixture.library())
        val searchAtRest = bounds(search)
        val panelAtRest = bounds(favorites)
        val gridAtRest = bounds(grid).single()

        halfway(Fixture.sets())
        val middle = Triple(bounds(search), bounds(favorites), bounds(grid).single())
        val cardMiddle = bounds(palmCard).single()
        compose.onRoot().captureRoboImage("src/test/screenshots/motion-section-landscape-light.png")
        finish()
        val cardAtRest = bounds(palmCard).single()

        assertStill(search, searchAtRest + bounds(search), middle.first)
        assertStill(favorites, panelAtRest + bounds(favorites), middle.second)
        // All scores leaves toward the start and Sets, later in the rail, comes in from the end.
        assertTrue("the grid leaves to the start: $gridAtRest -> ${middle.third}", middle.third.right < gridAtRest.right - 100f)
        assertTrue("the sets come from the end: $cardAtRest <- $cardMiddle", cardMiddle.left > cardAtRest.left + 100f)
    }

    @Test
    @Config(qualifiers = "w1280dp-h800dp-xhdpi")
    fun landscape_details_pane_slides_in_and_out_beside_a_still_list() {
        show(Fixture.library())

        halfway(selecting(ave))
        val gridMiddle = bounds(grid).single()
        val gridWidthMiddle = size(grid).width
        val backMiddle = bounds(detailsBack).single()
        finish()
        val gridOpen = bounds(grid).single()
        val backOpen = bounds(detailsBack).single()
        // The list takes its narrow width as the pane starts to open, and moves to the start with the side panel hiding.
        assertEquals("the list is at its new width while the pane opens", size(grid).width, gridWidthMiddle)
        assertTrue("the list moves to the start with the side panel: $gridOpen <- $gridMiddle", gridMiddle.left > gridOpen.left + 100f)
        assertEquals("the list stays at its height", gridOpen.top, gridMiddle.top)
        assertTrue("the pane comes in from the end: $backOpen <- $backMiddle", backMiddle.left > backOpen.left + 100f)

        halfway(Fixture.library())
        val gridClosing = bounds(grid).single()
        val gridWidthClosing = size(grid).width
        val backClosing = bounds(detailsBack).single()
        finish()
        // The list takes its full width as the pane starts to close, under the pane sliding away, and moves to the end
        // with the side panel coming back.
        assertEquals("the list is at its new width while the pane closes", size(grid).width, gridWidthClosing)
        assertTrue("the list moves to the end with the side panel: ${bounds(grid)} <- $gridClosing", gridClosing.left < bounds(grid).single().left - 100f)
        assertTrue("the pane leaves to the end: $backOpen -> $backClosing", backClosing.left > backOpen.left + 100f)
        assertTrue("the pane is gone", bounds(detailsBack).isEmpty())
    }

    @Test
    @Config(qualifiers = "w800dp-h1280dp-xhdpi")
    fun portrait_settings_slide_in_and_back_beside_a_still_rail_and_top_bar() {
        show(Fixture.library())
        val railAtRest = bounds(favorites)
        val gridAtRest = bounds(grid).single()

        halfway(Fixture.library().copy(screen = Screen.Settings))
        val railMiddle = bounds(favorites)
        val titleMiddle = bounds(settingsTitle)
        val folderMiddle = bounds(libraryFolder).single()
        compose.onRoot().captureRoboImage("src/test/screenshots/motion-settings-portrait-light.png")
        finish()
        val titleAtRest = bounds(settingsTitle)
        val folderAtRest = bounds(libraryFolder).single()

        // The rail stays put under the Settings content that slides over it; Settings' own top bar never moves.
        assertStill(favorites, railAtRest, railMiddle)
        assertStill(settingsTitle, titleAtRest, titleMiddle)
        assertTrue("Settings' content comes in from the end: $folderAtRest <- $folderMiddle", folderMiddle.left > folderAtRest.left + 100f)

        // Back runs it in reverse: Settings' content leaves to the end and the library's comes back from the start, and
        // the top bar stays.
        halfway(Fixture.library())
        val titleBack = bounds(settingsTitle)
        val folderBack = bounds(libraryFolder).single()
        val railBack = bounds(favorites)
        val gridBack = bounds(grid).single()
        finish()
        assertTrue("the library comes back from the start: $gridAtRest <- $gridBack", gridBack.right < gridAtRest.right - 100f)
        assertStill(settingsTitle, titleAtRest, titleBack)
        assertStill(favorites, railAtRest, railBack)
        assertTrue("Settings' content leaves to the end: $folderAtRest -> $folderBack", folderBack.left > folderAtRest.left + 100f)
        assertTrue("Settings are gone", bounds(libraryFolder).isEmpty())
    }

    @Test
    @Config(qualifiers = "w1280dp-h800dp-xhdpi")
    fun back_from_a_set_runs_the_push_in_reverse() {
        show(Fixture.palmSunday())
        val piecesAtRest = bounds(grid).single()

        halfway(Fixture.sets())
        // The set's pieces leave to the end while the sets come back from the start, both moving.
        val leaving = bounds(grid).filter { it.left > piecesAtRest.left + 100f }
        val cardMiddle = bounds(palmCard).single()
        finish()
        val cardAtRest = bounds(palmCard).single()
        assertTrue("the set's pieces leave to the end: $piecesAtRest -> ${bounds(grid)}", leaving.isNotEmpty())
        assertTrue("the sets come back from the start: $cardAtRest <- $cardMiddle", cardMiddle.left < cardAtRest.left - 100f)
    }

    @Test
    @Config(qualifiers = "w1280dp-h800dp-xhdpi")
    fun search_drops_in_and_rises_out_without_sliding() {
        show(Fixture.library())
        val field = hasSetTextAction()
        val search = Fixture.library().copy(screen = Screen.Search)

        steps(DROP_STEPS, search)
        val dropping = bounds(field).single()
        finish()
        val atRest = bounds(field).single()
        assertEquals("Search does not slide sideways", atRest.left, dropping.left)
        assertTrue("Search drops in from above: $atRest <- $dropping", dropping.top < atRest.top - 1f && dropping.top > atRest.top - 41f)

        steps(DROP_STEPS, Fixture.library())
        val rising = bounds(field).single()
        finish()
        assertEquals("Search does not slide sideways", atRest.left, rising.left)
        assertTrue("Search rises out: $atRest -> $rising", rising.top < atRest.top - 1f)
        assertTrue("Search is gone", bounds(field).isEmpty())
    }

    @Test
    @Config(qualifiers = "w1280dp-h800dp-xhdpi")
    fun the_back_key_closes_search_rising_over_the_library_as_its_arrow_does() {
        show(Fixture.library())
        snapshot = Fixture.library().copy(screen = Screen.Search)
        compose.waitForIdle()
        val field = hasSetTextAction()
        val atRest = bounds(field).single()
        afterBack = Fixture.library()

        pressBack(DROP_STEPS)
        val rising = bounds(field)
        val risingFrame = compose.activity.window.decorView.drawToBitmap()
        finish()
        val library = compose.activity.window.decorView.drawToBitmap()
        assertTrue("Search still shows partway: $rising", rising.size == 1)
        assertTrue("Search rises out: $atRest -> $rising", rising.single().top < atRest.top - 1f && rising.single().top > atRest.top - 41f)
        val drawn = differing(risingFrame, library, rising.single())
        assertTrue("Search draws over the library as it rises: $drawn", drawn > 100)
        assertTrue("Search is gone", bounds(field).isEmpty())
    }

    @Test
    @Config(qualifiers = "w1280dp-h800dp-xhdpi")
    fun the_back_key_closes_a_set_in_reverse() {
        show(Fixture.palmSunday())
        val piecesAtRest = bounds(grid).single()
        afterBack = Fixture.sets()

        pressBack(HALFWAY_STEPS)
        val leaving = bounds(grid).filter { it.left > piecesAtRest.left + 100f }
        val cardMiddle = bounds(palmCard).single()
        finish()
        val cardAtRest = bounds(palmCard).single()
        assertTrue("the set's pieces leave to the end: $piecesAtRest -> ${bounds(grid)}", leaving.isNotEmpty())
        assertTrue("the sets come back from the start: $cardAtRest <- $cardMiddle", cardMiddle.left < cardAtRest.left - 100f)
    }

    @Test
    @Config(qualifiers = "w1280dp-h800dp-xhdpi")
    fun the_side_panel_slides_and_the_grid_changes_its_columns_once() {
        val shown = Fixture.library()
        val hidden = shown.copy(layout = shown.layout.copy(sidebarCollapsedLandscape = true))
        show(shown)
        for (to in listOf(hidden, shown)) {
            val panelFrom = bounds(sidePanel)
            val before = size(grid).width
            compose.mainClock.autoAdvance = false
            snapshot = to
            val sizes = mutableListOf<Int>()
            val panel = mutableListOf<Rect>()
            repeat(PANEL_FRAMES) {
                compose.mainClock.advanceTimeByFrame()
                compose.waitForIdle()
                sizes += size(grid).width
                panel += bounds(sidePanel)
            }
            finish()
            val panelTo = bounds(sidePanel)
            // The grid takes its new width, and so its new columns, on the first frame of the change, and keeps it while it
            // moves.
            assertEquals("the grid's width changes once: $sizes", listOf(size(grid).width), sizes.dropWhile { it == before }.distinct())
            // The panel is on its way between showing and gone, at the start of the line.
            val restEnd = (panelFrom + panelTo).single().right
            val middle = panel.filter { it.width > 0f }.map { it.right }
            assertTrue("the panel slides at the start: $restEnd, $middle", middle.any { it < restEnd - 50f && it > 50f })
        }
    }

    @Test
    @Config(qualifiers = "w1280dp-h800dp-xhdpi")
    fun the_chips_fade_through_as_the_side_panel_hides_and_shows() {
        val shown = Fixture.library()
        val hidden = shown.copy(layout = shown.layout.copy(sidebarCollapsedLandscape = true))
        show(hidden)
        val area = bounds(chips).single()
        // The start of the row as drawn, wherever it is (it moves with the panel, and narrows): see-through while it fades.
        fun row(): android.graphics.Bitmap {
            val at = bounds(chips).single()
            val window = compose.activity.window.decorView.drawToBitmap()
            return android.graphics.Bitmap.createBitmap(window, at.left.toInt(), at.top.toInt(), ROW_START_PIXELS, area.height.toInt())
        }
        val atRest = row()
        // A frame at a time, each composing and drawing.
        fun frameAt(millis: Long) = repeat((millis / FRAME_MILLIS).toInt()) {
            compose.mainClock.advanceTimeByFrame()
            compose.waitForIdle()
        }
        fun drawn() = differing(row(), atRest, Rect(0f, 0f, atRest.width.toFloat(), atRest.height.toFloat()))

        // Showing the panel: the chips fade out over the first half, then their row goes.
        compose.mainClock.autoAdvance = false
        snapshot = shown
        frameAt(FADE_QUARTER_MILLIS)
        val fading = bounds(chips)
        val fadingDiffers = drawn()
        frameAt(2 * FADE_QUARTER_MILLIS)
        val gone = bounds(chips)
        finish()
        assertTrue("the chips are still there early on: $fading", fading.size == 1)
        assertTrue("the chips are fading out: $fadingDiffers", fadingDiffers > 20)
        assertTrue("the chips are gone in the second half: $gone", gone.isEmpty())

        // Hiding it: the chips' room comes at once, empty over the first half, then the chips fade in.
        compose.mainClock.autoAdvance = false
        snapshot = hidden
        frameAt(FADE_QUARTER_MILLIS)
        val early = row().let { image ->
            val blank = android.graphics.Bitmap.createBitmap(image.width, image.height, image.config!!).apply { eraseColor(image.getPixel(0, 0)) }
            differing(image, blank, Rect(0f, 0f, image.width.toFloat(), image.height.toFloat()))
        }
        frameAt(2 * FADE_QUARTER_MILLIS)
        val coming = drawn()
        finish()
        val shownAgain = drawn()
        assertEquals("the room is empty over the first half", 0, early)
        assertTrue("the chips are fading in: $coming", coming > 20)
        assertEquals("the chips end as they were", 0, shownAgain)
        assertEquals("the chips end where they were", area, bounds(chips).single())
    }

    @Test
    @Config(qualifiers = "w800dp-h1280dp-xhdpi")
    fun portrait_rail_stays_while_a_set_opens() {
        show(Fixture.sets())
        val railAtRest = bounds(favorites) + bounds(sets)

        halfway(Fixture.palmSunday())
        val railMiddle = bounds(favorites) + bounds(sets)
        val piecesMiddle = bounds(grid).single()
        finish()
        val piecesAtRest = bounds(grid).single()

        assertTrue("the rail shows in the middle of the change", railMiddle.isNotEmpty())
        railMiddle.forEach { assertTrue("rail at $it, at rest at $railAtRest", it in railAtRest) }
        assertTrue("the set's pieces come in from the end: $piecesAtRest <- $piecesMiddle", piecesMiddle.left > piecesAtRest.left + 100f)
    }

    @Test
    @Config(qualifiers = "w384dp-h832dp-xhdpi")
    fun phone_bars_stay_while_sections_and_details_change() {
        show(Fixture.library())
        val barAtRest = bounds(favorites)
        val gridAtRest = bounds(grid).single()

        halfway(Fixture.sets())
        val barMiddle = bounds(favorites)
        val gridMiddle = bounds(grid).single()
        finish()
        assertStill(favorites, barAtRest, barMiddle)
        assertTrue("the grid leaves to the start: $gridAtRest -> $gridMiddle", gridMiddle.right < gridAtRest.right - 100f)

        snapshot = Fixture.library()
        finish()
        halfway(selecting(ave))
        val backMiddle = bounds(detailsBack)
        val openMiddle = bounds(hasText("Open score"))
        val bodyMiddle = bounds(hasTestTag(DETAILS_BODY_TAG)).single()
        compose.onRoot().captureRoboImage("src/test/screenshots/motion-details-phone-light.png")
        finish()
        // The details' own top and bottom bars appear in place; only the body slides in.
        assertStill(detailsBack, bounds(detailsBack), backMiddle)
        assertStill(hasText("Open score"), bounds(hasText("Open score")), openMiddle)
        val bodyAtRest = bounds(hasTestTag(DETAILS_BODY_TAG)).single()
        assertTrue("the body comes in from the end: $bodyAtRest <- $bodyMiddle", bodyMiddle.left > bodyAtRest.left + 100f)
    }

    @Test
    @Config(qualifiers = "w1280dp-h800dp-xhdpi")
    fun a_bar_only_the_coming_screen_has_shows_only_in_the_second_half() {
        show(Fixture.library())
        val newSet = hasText("New set")

        // About 70 ms in: the going screen's bars show, and Sets' New set pill not yet.
        steps(2, Fixture.sets())
        val pill = bounds(newSet).single()
        val early = blueness(pill)
        // About 250 ms in: the pill fading in, part of the way.
        steps(5)
        val late = blueness(pill)
        finish()
        val atRest = blueness(pill)
        assertTrue("New set does not show in the first half of the change: $early", early < FAINT)
        assertTrue("New set fades in over the second half: $early, $late, $atRest", late > FAINT && late < atRest - FAINT)
    }

    /** How blue the bluest pixel inside [area] is (the New set pill's accent, opaque or fading): blue less red, 0 to 1. */
    private fun blueness(area: Rect): Float {
        // Drawn on the spot: a capture that waits for the next frame would wait for the paused clock.
        val pixels = compose.activity.window.decorView.drawToBitmap()
        var bluest = 0f
        for (x in area.left.toInt() until area.right.toInt()) {
            for (y in area.top.toInt() until area.bottom.toInt()) {
                val c = Color(pixels.getPixel(x, y))
                bluest = maxOf(bluest, c.blue - c.red)
            }
        }
        return bluest
    }

    @Test
    @Config(qualifiers = "w1280dp-h800dp-xhdpi")
    fun a_section_switch_interrupted_midway_carries_on_from_where_it_was() {
        show(Fixture.library())
        val gridAtRest = bounds(grid).single()
        val recent = Fixture.library().copy(sectionKey = "recent", title = SectionTitle.Recent, rows = emptyList())

        steps(3, Fixture.sets())
        val gridBefore = bounds(grid).maxOf { it.right }
        val cardBefore = bounds(palmCard).single()
        // Recent, earlier in the rail than Sets, comes in from the start, and Sets turns back toward the end.
        snapshot = recent
        val cards = mutableListOf(cardBefore.left)
        repeat(TURN_FRAMES) {
            compose.mainClock.advanceTimeByFrame()
            compose.waitForIdle()
            bounds(palmCard).singleOrNull()?.takeIf { it.width > 0f }?.let { cards += it.left }
        }
        val emptyMiddle = bounds(hasText("No recent scores")).single()
        finish()
        val emptyAtRest = bounds(hasText("No recent scores")).single()

        assertTrue("All scores was leaving to the start: $gridAtRest -> $gridBefore", gridBefore < gridAtRest.right - 100f)
        // Sets comes on in until the new change starts, then turns where it is, without a jump, and only goes on to the end.
        val turn = cards.indexOf(cards.min())
        assertTrue("Sets comes on in until it turns: $cards", cards.take(turn + 1).zipWithNext().all { (a, b) -> b <= a })
        assertTrue("Sets then goes only toward the end: $cards", cards.drop(turn).zipWithNext().all { (a, b) -> b >= a })
        assertTrue("Sets turns where it is: $cards", cards[turn + 1] - cards[turn] < TURN_JUMP)
        assertTrue("Sets goes on to the end: $cards", cards.last() > cardBefore.left + 100f)
        assertTrue("Recent comes in from the start: $emptyAtRest <- $emptyMiddle", emptyMiddle.left < emptyAtRest.left - 100f)
    }

    @Test
    fun the_going_and_coming_screens_bars_never_show_together() {
        listOf(Change.Push, Change.Pop, Change.PredictivePop, Change.Forward, Change.Backward).forEach { change ->
            var progress = 0f
            val bars = SharedBars()
            val going = ScreenPart({ 1f - progress }, { 1f - progress }, end = 1f, bars)
            val coming = ScreenPart({ progress }, { progress }, end = 1f, bars)
            going.start(change, entering = false)
            coming.start(change, entering = true)
            for (step in 0..PART_STEPS) {
                progress = step / PART_STEPS.toFloat()
                assertTrue("$change at $progress: exactly one screen's bars show", going.barsShown != coming.barsShown)
                assertTrue("$change at $progress: one fade at a time", going.fadeAlpha == 0f || coming.fadeAlpha == 0f)
                if (progress < 0.5f) assertEquals("$change at $progress: the going screen's", true, going.barsShown)
            }
            assertEquals("$change: the coming screen's bars end opaque", 1f, coming.fadeAlpha)
            assertEquals("$change: the going screen's bars end gone", 0f, going.fadeAlpha)
        }
    }

    @Test
    fun a_screen_turned_back_midway_slides_on_from_where_it_was() {
        var slide = 0f
        val sets = ScreenPart({ slide }, { slide }, end = 1f, SharedBars())
        sets.start(Change.Forward, entering = true)
        slide = 0.4f
        val turned = sets.away
        assertTrue("Sets is on its way in from the end: $turned", turned in 0.01f..0.99f)

        sets.start(Change.Backward, entering = false)
        assertEquals("Sets carries on from where it was", turned, sets.away, 0.0001f)
        var last = turned
        while (slide > 0f) {
            slide = maxOf(0f, slide - 0.05f)
            assertTrue("Sets moves only toward the end: $last -> ${sets.away}", sets.away >= last)
            last = sets.away
        }
        assertEquals("Sets ends past the end", 1f, sets.away, 0.0001f)
    }

    private companion object {
        /** Blueness the bars' grey ink and background stay under, and a fade part of the way stays clear of. */
        const val FAINT = 0.15f
        const val PART_STEPS = 60
        const val TURN_FRAMES = 10

        /** About 80 ms into Search's 200 ms. */
        const val DROP_STEPS = 2

        /** Most of the side panel's 250 ms, a frame at a time. */
        const val PANEL_FRAMES = 12

        /** Pixels: what a 16 ms frame moves the content at the start of a slide, well below a jump to where it began. */
        const val TURN_JUMP = 150f
        /** About 100 ms into the 300 ms change, before the emphasized curve has settled the content. */
        const val HALFWAY_STEPS = 3
        const val FRAME_MILLIS = 16L

        /** How much of the chips' row the fade test compares: the chips themselves, short of where a narrower row ends. */
        const val ROW_START_PIXELS = 1200

        /** A quarter of the chips' fade through: halfway into its first half. */
        const val FADE_QUARTER_MILLIS = CHIPS_FADE_MILLIS / 2L

        const val STEP_MILLIS = 40L
    }
}
