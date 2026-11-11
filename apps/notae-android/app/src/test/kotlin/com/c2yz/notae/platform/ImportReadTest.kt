package com.c2yz.notae.platform

import android.content.Intent
import android.net.Uri
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.ByteArrayInputStream
import java.io.InputStream

/**
 * The import file read: capped, so a huge file picked by mistake is a typed failure instead of an OutOfMemoryError; and
 * the file a received intent carries (Robolectric, for [Intent] and [Uri]).
 */
@RunWith(RobolectricTestRunner::class)
class ImportReadTest {
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
    }
}
