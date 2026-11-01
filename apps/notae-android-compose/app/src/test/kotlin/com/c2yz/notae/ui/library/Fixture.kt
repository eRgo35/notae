package com.c2yz.notae.ui.library

import com.c2yz.notae.core.Banner
import com.c2yz.notae.core.ComposerEntry
import com.c2yz.notae.core.Edited
import com.c2yz.notae.core.LayoutPrefs
import com.c2yz.notae.core.LibraryView
import com.c2yz.notae.core.Origin
import com.c2yz.notae.core.Row
import com.c2yz.notae.core.RowStatus
import com.c2yz.notae.core.ScanSummary
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.SearchBox
import com.c2yz.notae.core.SearchScope
import com.c2yz.notae.core.SectionEntry
import com.c2yz.notae.core.SectionTitle
import com.c2yz.notae.core.SetCard
import com.c2yz.notae.core.SetEntry
import com.c2yz.notae.core.SettingsInfo
import com.c2yz.notae.core.SetupScreen
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.core.StatusMessage
import com.c2yz.notae.core.Subtitle

/**
 * The parity fixture library (docs/compose-parity/README.md, "Fixture library"): 20 scores sorted by
 * title, Ave Maria by Schubert, Bach and Palestrina on two of rows 13 to 20, three favorites, two
 * recent scores and three sets. Panis angelicus, Salve Regina, Ubi caritas and Zdrowaś Maryjo show
 * only in search-results-portrait; Pange lingua is free, as no Slint capture shows the 20th title.
 */
object Fixture {
    private const val LONG_TITLE =
        "Bardzo długi tytuł utworu liturgicznego na wejście procesji z towarzyszeniem chóru i organów w uroczystość"

    private val titles = listOf(
        "Agnus Dei", "Alleluja", "Ave Maria", "Ave verum corpus", LONG_TITLE, "Barka", "Bogurodzica",
        "Ciebie Boga wysławiamy", "Gloria in excelsis", "Jezu ufam Tobie", "Kyrie eleison", "Lacrimosa dies illa",
        "Magnificat", "Pange lingua", "Panis angelicus", "Salve Regina", "Sanctus", "Te Deum laudamus", "Ubi caritas",
        "Zdrowaś Maryjo",
    )
    private val composers = mapOf("Ave Maria" to "Schubert", "Pange lingua" to "Bach", "Ubi caritas" to "Palestrina")
    private val favorites = listOf("Te Deum laudamus", "Magnificat", "Ave Maria")

    val rows: List<Row> = titles.mapIndexed { index, title ->
        row(index, title, composers[title], favorite = title in favorites)
    }

    val running = Banner(StatusMessage.ScanningLibrary, busy = true, failed = false, fraction = null, cancelEnabled = true, acknowledgeEnabled = false)

    val done = Banner(
        StatusMessage.Finished(ScanSummary(added = 20u, updated = 0u, moved = 0u, deleted = 0u, conflicts = 0u, failures = 0u)),
        busy = false,
        failed = false,
        fraction = null,
        cancelEnabled = false,
        acknowledgeEnabled = true,
    )

    fun row(index: Int, title: String, composer: String? = null, favorite: Boolean = false) = Row(
        selectionId = "key-$index",
        noteKey = "key-$index",
        number = null,
        title = title,
        composer = composer,
        tags = emptyList(),
        status = RowStatus.READY,
        textPending = false,
        relativePath = "$title.pdf",
        titleOrigin = Origin.FILENAME_DERIVED,
        composerOrigin = Origin.FILENAME_DERIVED,
        snippet = null,
        itemId = null,
        justOpened = false,
        position = null,
        missing = false,
        favorite = favorite,
        thumbnail = null,
    )

    /** All scores, in the grid, with the side panel open. */
    fun library(): Snapshot = base.copy(
        screen = Screen.Library,
        sectionKey = "all",
        title = SectionTitle.All,
        subtitle = Subtitle.Scores(rows.size.toUInt()),
        sections = listOf(
            SectionEntry("all", SectionTitle.All, 20u),
            SectionEntry("recent", SectionTitle.Recent, 2u),
            SectionEntry("favorites", SectionTitle.Favorites, 3u),
            SectionEntry("sets", SectionTitle.Sets, 3u),
        ),
        composers = listOf(
            ComposerEntry("composer:bach", "Bach", 1u),
            ComposerEntry("composer:palestrina", "Palestrina", 1u),
            ComposerEntry("composer:schubert", "Schubert", 1u),
        ),
        rows = rows,
        rescanEnabled = true,
    )

    const val FRIDAY_NAME = "Wielki Piatek - liturgia Meki Panskiej z adoracja krzyza i procesja do Grobu Panskiego"

    /** The three sets, most recently edited first. */
    val setCards = listOf(
        SetCard("palm", "Niedziela Palmowa", "2026-03-29", 9u, Edited.Today),
        SetCard("empty", "Pusty zestaw", null, 0u, Edited.Today),
        SetCard("friday", FRIDAY_NAME, "2026-04-03", 0u, Edited.Today),
    )

    /** Niedziela Palmowa's nine items: two repeats and a piece missing from the library. */
    val palmRows: List<Row> = listOf(
        "Ave Maria", "Kyrie eleison", "Magnificat", "Ave Maria", "Alleluja", "Sanctus", LONG_TITLE, "Pan Jezus już się zbliża", "Alleluja",
    ).mapIndexed { index, title ->
        val position = index + 1
        val source = rows.firstOrNull { it.title == title }
        (source ?: row(99, title)).copy(
            selectionId = "item-$position",
            itemId = "item-$position",
            position = position.toUInt(),
            missing = source == null,
        )
    }

    /** Inside Niedziela Palmowa after Kyrie eleison (item 2) was opened. */
    fun justOpened(): Snapshot = palmSunday().let { palm ->
        val rows = palm.rows.map { it.copy(justOpened = it.itemId == "item-2") }
        palm.copy(rows = rows, setEntries = rows.map { SetEntry.Piece(it) })
    }

    /** The Sets section, in the grid. */
    fun sets(): Snapshot = library().copy(
        sectionKey = "sets",
        title = SectionTitle.Sets,
        subtitle = Subtitle.Sets(setCards.size.toUInt()),
        rows = emptyList(),
        setCards = setCards,
    )

    /** Inside Niedziela Palmowa. */
    fun palmSunday(): Snapshot = library().copy(
        sectionKey = "set:palm",
        title = SectionTitle.Set("Niedziela Palmowa"),
        subtitle = Subtitle.Set("2026-03-29", 9u, 0u),
        setNotes = "Msza 10:00, kantor: Anna",
        rows = palmRows,
        setEntries = palmRows.map { SetEntry.Piece(it) },
    )

    const val PSALM = "Psalm responsoryjny: Wszystkie krańce ziemi ujrzały zbawienie Boga naszego"
    /** The psalm as its controls name it: the first 60 characters and an ellipsis. */
    const val PSALM_NAME = "Psalm responsoryjny: Wszystkie krańce ziemi ujrzały zbawieni…"
    const val HOMILY = "Homilia"

    /** Niedziela Palmowa with two text notes: the psalm after Kyrie and the homily after the second Ave Maria. */
    val notedEntries: List<SetEntry> = buildList<Any> {
        palmRows.forEachIndexed { index, row ->
            if (index == 2) add("note-1" to PSALM)
            if (index == 4) add("note-2" to HOMILY)
            add(row)
        }
    }.mapIndexed { index, item ->
        val position = (index + 1).toUInt()
        when (item) {
            is Row -> SetEntry.Piece(item.copy(position = position))
            else -> (item as Pair<*, *>).let { SetEntry.Note(position, it.first as String, it.second as String) }
        }
    }

    /** Inside Niedziela Palmowa with its two notes. */
    fun palmWithNotes(): Snapshot = palmSunday().copy(
        subtitle = Subtitle.Set("2026-03-29", 9u, 2u),
        rows = notedEntries.filterIsInstance<SetEntry.Piece>().map { it.row },
        setEntries = notedEntries,
    )

    /**
     * A fresh core's snapshot, for the fields the library screen does not read. It is written out rather than asked of a
     * core, so tests that only render never load the native core in their Robolectric sandbox (see
     * core/OneSandbox.kt). `CoreSmokeTest.fresh_core_snapshot_matches_fixture_base` keeps it equal to the core's.
     */
    val base: Snapshot = Snapshot(
        revision = 0u,
        screen = Screen.Setup(SetupScreen.Unconfigured),
        sectionKey = "all",
        title = SectionTitle.All,
        subtitle = Subtitle.Scores(0u),
        setNotes = null,
        sections = listOf(
            SectionEntry("all", SectionTitle.All, 0u),
            SectionEntry("recent", SectionTitle.Recent, 0u),
            SectionEntry("favorites", SectionTitle.Favorites, 0u),
            SectionEntry("sets", SectionTitle.Sets, 0u),
        ),
        composers = emptyList(),
        rows = emptyList(),
        setEntries = emptyList(),
        setCards = emptyList(),
        selection = null,
        search = SearchBox("", SearchScope.EVERYTHING),
        banner = Banner(message = null, busy = false, failed = false, fraction = null, cancelEnabled = false, acknowledgeEnabled = false),
        rescanEnabled = true,
        settings = SettingsInfo(libraryName = null, indexedCount = 0u, noTextCount = 0u, latestScan = null, export = null, importStatus = null),
        layout = LayoutPrefs(LibraryView.GRID, sidebarCollapsedPortrait = true, sidebarCollapsedLandscape = false),
        reordering = false,
        setDialog = null,
        deleteSet = null,
        metadataEditor = null,
        addToSet = null,
        addPicker = null,
        pdfOpenError = null,
        importAvailable = true,
        importConfirmation = null,
    )
}
