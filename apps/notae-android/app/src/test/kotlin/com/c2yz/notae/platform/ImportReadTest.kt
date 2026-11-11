package com.c2yz.notae.platform

import android.content.ContentProvider
import android.content.ContentValues
import android.content.Intent
import android.content.pm.ProviderInfo
import android.database.Cursor
import android.net.Uri
import android.os.Looper
import android.os.ParcelFileDescriptor
import androidx.test.core.app.ActivityScenario
import androidx.test.core.app.ApplicationProvider
import com.c2yz.notae.MainActivity
import com.c2yz.notae.core.assertOneSandbox
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.GraphicsMode
import org.robolectric.shadows.ShadowToast
import java.io.File
import java.io.ByteArrayInputStream
import java.io.InputStream

/**
 * The import file read: capped, so a huge file picked by mistake is a typed failure instead of an OutOfMemoryError; and
 * the file a received intent carries (Robolectric, for [Intent] and [Uri]). The toast test creates a core through
 * MainActivity, so the class runs in the one native sandbox (OneSandbox.kt).
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class ImportReadTest {
    @get:Rule
    val files = TemporaryFolder()

    /** [size] bytes of `x`, counting how many were read. */
    private class Sized(private val size: Long) : InputStream() {
        var read = 0L

        override fun read(): Int = if (read < size) 'x'.code.also { read++ } else -1
    }

    @Test
    fun a_file_within_the_cap_is_read() {
        val text = readImport({ 7L }, { ByteArrayInputStream("{\"a\":1}".toByteArray()) }, cap = 16)

        assertEquals("{\"a\":1}", text.getOrNull())
    }

    @Test
    fun a_file_declared_over_the_cap_fails_without_being_opened() {
        var opened = false
        val text = readImport({ 17L }, { opened = true; Sized(17) }, cap = 16)

        assertTrue(text.isFailure)
        assertEquals("the file is larger than 16 bytes", text.exceptionOrNull()?.message)
        assertFalse(opened)
    }

    @Test
    fun a_file_of_unknown_size_stops_one_byte_past_the_cap() {
        val stream = Sized(1_000_000)
        val text = readImport({ -1L }, { stream }, cap = 16)

        assertEquals("the file is larger than 16 bytes", text.exceptionOrNull()?.message)
        assertTrue("read ${stream.read} bytes", stream.read <= 17)
    }

    @Test
    fun a_read_error_is_a_failure_not_a_throw() {
        val text = readImport({ error("no descriptor") }, { error("no stream") }, cap = 16)

        assertTrue(text.isFailure)
    }

    @Test
    fun importUriFromViewAndSend() {
        val file = Uri.parse("content://com.example.files/shared/Mass.notae-set")

        assertEquals(file, importUri(Intent(Intent.ACTION_VIEW, file)))
        assertEquals(file, importUri(Intent(Intent.ACTION_SEND).putExtra(Intent.EXTRA_STREAM, file)))
        assertNull(importUri(Intent(Intent.ACTION_MAIN)))
        assertNull(importUri(Intent(Intent.ACTION_MAIN, file)))
        assertNull(importUri(Intent(Intent.ACTION_VIEW)))
        assertNull(importUri(Intent(Intent.ACTION_SEND)))
        assertNull(importUri(Intent(Intent.ACTION_VIEW, Uri.parse("file:///sdcard/Mass.notae-set"))))
        assertNull(importUri(Intent(Intent.ACTION_VIEW, Uri.parse("https://example.com/Mass.notae-set"))))
        // Relaunched from Recents: the file was read when the intent first came.
        assertNull(importUri(Intent(Intent.ACTION_VIEW, file).addFlags(Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY)))
        assertNull(
            importUri(
                Intent(Intent.ACTION_SEND).putExtra(Intent.EXTRA_STREAM, file).addFlags(Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY),
            ),
        )
    }

    @Test
    fun a_received_file_that_is_not_notae_shows_a_toast() {
        assertOneSandbox()
        OneFileProvider.file = files.newFile().apply { writeText("{\"a\":1}") }
        Robolectric.buildContentProvider(OneFileProvider::class.java).create(ProviderInfo().apply { authority = AUTHORITY })
        val view = Intent(Intent.ACTION_VIEW, Uri.parse("content://$AUTHORITY/other.json"))
            .setClass(ApplicationProvider.getApplicationContext(), MainActivity::class.java)

        val scenario = ActivityScenario.launch<MainActivity>(view)
        val deadline = System.nanoTime() + 30_000_000_000L
        while (ShadowToast.getTextOfLatestToast() == null) {
            check(System.nanoTime() < deadline) { "no toast" }
            shadowOf(Looper.getMainLooper()).idle()
            Thread.sleep(20)
        }

        assertEquals("Could not import sets: this is not a Notae sets file", ShadowToast.getTextOfLatestToast())
        scenario.close()
    }

    /** Serves [file] at any URI, as another app's provider would serve a file it shares. */
    class OneFileProvider : ContentProvider() {
        override fun onCreate() = true

        override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor =
            ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)

        override fun query(uri: Uri, projection: Array<out String>?, selection: String?, args: Array<out String>?, sortOrder: String?): Cursor? = null

        override fun getType(uri: Uri) = "application/json"

        override fun insert(uri: Uri, values: ContentValues?): Uri? = null

        override fun delete(uri: Uri, selection: String?, args: Array<out String>?) = 0

        override fun update(uri: Uri, values: ContentValues?, selection: String?, args: Array<out String>?) = 0

        companion object {
            lateinit var file: File
        }
    }

    private companion object {
        const val AUTHORITY = "com.c2yz.notae.test.received"
    }
}
