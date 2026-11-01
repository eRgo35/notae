package com.c2yz.notae.ui.library

import com.c2yz.notae.core.Banner
import com.c2yz.notae.core.ComposerEntry
import com.c2yz.notae.core.CoreListener
import com.c2yz.notae.core.DocumentEntry
import com.c2yz.notae.core.Effect
import com.c2yz.notae.core.NotaeCore
import com.c2yz.notae.core.Origin
import com.c2yz.notae.core.PlatformLibrary
import com.c2yz.notae.core.Row
import com.c2yz.notae.core.RowStatus
import com.c2yz.notae.core.ScanSummary
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.SectionEntry
import com.c2yz.notae.core.SectionTitle
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.core.StatusMessage
import com.c2yz.notae.core.Subtitle
import java.nio.file.Files

/**
 * The parity fixture library (docs/compose-parity/README.md, "Fixture library"): 20 scores sorted by
 * title, Ave Maria by Schubert, Bach and Palestrina on two of rows 13 to 20, three favorites, two
 * recent scores and three sets. Rows 13 to 20 other than Magnificat, Sanctus and Te Deum laudamus
 * are never visible in the Slint captures, so their titles are free.
 */
object Fixture {
    private const val LONG_TITLE =
        "Bardzo długi tytuł utworu liturgicznego na wejście procesji z towarzyszeniem chóru i organów w uroczystość"

    private val titles = listOf(
        "Agnus Dei", "Alleluja", "Ave Maria", "Ave verum corpus", LONG_TITLE, "Barka", "Bogurodzica",
        "Ciebie Boga wysławiamy", "Gloria in excelsis", "Jezu ufam Tobie", "Kyrie eleison", "Lacrimosa dies illa",
        "Magnificat", "Pange lingua", "Panis angelicus", "Regina caeli", "Salve Regina", "Sanctus", "Sicut cervus",
        "Te Deum laudamus",
    )
    private val composers = mapOf("Ave Maria" to "Schubert", "Pange lingua" to "Bach", "Sicut cervus" to "Palestrina")
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

    /** A fresh core's snapshot, for the fields the library screen does not read. */
    private val base: Snapshot by lazy {
        val dir = Files.createTempDirectory("notae-fixture").toFile()
        val core = NotaeCore(dir.path, NoPlatform, NoListener)
        try {
            core.snapshot()
        } finally {
            core.close()
            dir.deleteRecursively()
        }
    }
}

private object NoPlatform : PlatformLibrary {
    override fun checkRoot(handle: String) = error("no library folder")

    override fun enumerate(handle: String): List<DocumentEntry> = error("no library folder")

    override fun extractTextLayer(handle: String, path: String): String? = error("no library folder")

    override fun renderThumbnail(handle: String, path: String, outPng: String): Boolean = error("no library folder")
}

private object NoListener : CoreListener {
    override fun onSnapshot(snapshot: Snapshot) = Unit

    override fun onEffect(effect: Effect) = Unit
}
