package com.c2yz.notae.platform

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.File
import java.nio.charset.Charset
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

/** Unpacking a score ZIP into Notae's own storage. */
@RunWith(RobolectricTestRunner::class)
class ZipLibraryTest {
    @get:Rule
    val temp = TemporaryFolder()

    private fun zip(vararg entries: Pair<String, String>, charset: Charset = Charsets.UTF_8): ByteArray {
        val bytes = ByteArrayOutputStream()
        ZipOutputStream(bytes, charset).use { out ->
            for ((name, content) in entries) {
                out.putNextEntry(ZipEntry(name).apply { time = ENTRY_TIME })
                out.write(content.toByteArray())
                out.closeEntry()
            }
        }
        return bytes.toByteArray()
    }

    private fun unpack(bytes: ByteArray, free: Long = Long.MAX_VALUE): File =
        unpackZipLibrary({ ByteArrayInputStream(bytes) }, temp.root) { free }

    private fun reason(block: () -> Unit): ZipImportException.Reason = try {
        block()
        fail("expected a ZipImportException")
        throw IllegalStateException()
    } catch (error: ZipImportException) {
        error.reason
    }

    @Test
    fun keeps_pdfs_and_the_sidecar_and_skips_the_rest() {
        val dir = unpack(
            zip(
                "Nutki/001 - Ave.pdf" to "a",
                "Nutki/sub/002 - Credo.PDF" to "b",
                ".notae/library.json" to "{}",
                "__MACOSX/Nutki/._001 - Ave.pdf" to "x",
                "Nutki/readme.txt" to "t",
                "Nutki/.hidden.pdf" to "h",
            ),
        )
        assertEquals(File(temp.root, ZIP_LIBRARY_DIR), dir)
        assertEquals("a", File(dir, "Nutki/001 - Ave.pdf").readText())
        assertTrue(File(dir, "Nutki/sub/002 - Credo.PDF").exists())
        assertTrue(File(dir, ".notae/library.json").exists())
        assertFalse(File(dir, "__MACOSX").exists())
        assertFalse(File(dir, "Nutki/readme.txt").exists())
        assertFalse(File(dir, "Nutki/.hidden.pdf").exists())
    }

    @Test
    fun skipped_root_folders_are_not_unpacked() {
        val dir = unpack(
            zip(
                "001 - Ave.pdf" to "a",
                "skany/001 - Ave.pdf" to "s",
                "inne/Ćwiczenia.pdf" to "i",
                "sources/Indeks.pdf" to "x",
                "Nutki/skany/002 - Credo.pdf" to "c",
            ),
        )
        assertTrue(File(dir, "001 - Ave.pdf").exists())
        assertFalse(File(dir, "skany").exists())
        assertFalse(File(dir, "inne").exists())
        assertFalse(File(dir, "sources").exists())
        // Only the library root's folders are skipped, as when scanning.
        assertTrue(File(dir, "Nutki/skany/002 - Credo.pdf").exists())
    }

    @Test
    fun a_zip_of_only_skipped_folders_has_no_pdfs() {
        assertEquals(ZipImportException.Reason.NO_PDFS, reason { unpack(zip("skany/001 - Ave.pdf" to "s")) })
    }

    @Test
    fun zip_slip_is_rejected_and_the_old_library_kept() {
        unpack(zip("Ave.pdf" to "old"))
        assertEquals(ZipImportException.Reason.UNREADABLE, reason { unpack(zip("../evil.pdf" to "x", "Credo.pdf" to "c")) })
        assertEquals("old", File(temp.root, "$ZIP_LIBRARY_DIR/Ave.pdf").readText())
        assertFalse(File(temp.root, "evil.pdf").exists())
        assertFalse(File(temp.root, "$ZIP_LIBRARY_DIR.new").exists())
    }

    @Test
    fun polish_windows_names_come_out_right() {
        val dir = unpack(zip("Błogosławiona Ząb.pdf" to "p", charset = Charset.forName("IBM852")))
        assertTrue(dir.list()!!.joinToString(), File(dir, "Błogosławiona Ząb.pdf").exists())
    }

    @Test
    fun backslashes_between_folders_make_folders() {
        val dir = unpack(zip("Msze\\Ave.pdf" to "a"))
        assertEquals("a", File(dir, "Msze/Ave.pdf").readText())
    }

    @Test
    fun a_zip_without_pdfs_is_reported() {
        assertEquals(ZipImportException.Reason.NO_PDFS, reason { unpack(zip("notes.txt" to "t")) })
    }

    @Test
    fun a_file_that_is_not_a_zip_is_reported() {
        assertEquals(ZipImportException.Reason.NOT_ZIP, reason { unpack("{\"notae\":1}".toByteArray()) })
    }

    @Test
    fun too_little_space_is_reported() {
        assertEquals(ZipImportException.Reason.NO_SPACE, reason { unpack(zip("Ave.pdf" to "a"), free = 10L * 1024 * 1024) })
    }

    @Test
    fun a_new_zip_replaces_the_files_at_the_same_place() {
        unpack(zip("Ave.pdf" to "old", "Gone.pdf" to "g"))
        val dir = unpack(zip("Ave.pdf" to "new", "Credo.pdf" to "c"))
        assertEquals("new", File(dir, "Ave.pdf").readText())
        assertTrue(File(dir, "Credo.pdf").exists())
        assertFalse(File(dir, "Gone.pdf").exists())
        assertTrue(zipLibraryHandle(temp.root).startsWith("file:"))
        assertTrue(zipLibraryHandle(temp.root).endsWith("/$ZIP_LIBRARY_DIR"))
    }

    @Test
    fun files_keep_the_time_the_zip_gives_them_so_a_reimport_changes_nothing() {
        val dir = unpack(zip("Ave.pdf" to "a"))
        // ZIP times have a two-second resolution.
        assertEquals(ENTRY_TIME.toDouble(), File(dir, "Ave.pdf").lastModified().toDouble(), 2_000.0)
    }

    @Test
    fun choosing_a_folder_removes_the_unpacked_copy() {
        unpack(zip("Ave.pdf" to "a"))
        removeZipLibrary(temp.root)
        assertFalse(File(temp.root, ZIP_LIBRARY_DIR).exists())
        removeZipLibrary(temp.root)
    }

    private companion object {
        const val ENTRY_TIME = 1_700_000_000_000L
    }
}
