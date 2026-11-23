package com.c2yz.notae.platform

import android.content.Intent
import android.net.Uri
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.robolectric.Robolectric
import androidx.activity.ComponentActivity
import java.io.File
import org.junit.After
import org.junit.Before
import org.junit.Test
import androidx.core.content.FileProvider
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class PdfOpenerTest {
    /** FileProvider keeps each authority's paths in a static cache, which would outlive this test's files directory. */
    @Before
    @After
    fun forgetFileProviderPaths() {
        val cache = FileProvider::class.java.getDeclaredField("sCache").apply { isAccessible = true }.get(null) as MutableMap<*, *>
        synchronized(cache) { cache.clear() }
    }

    @Test
    fun pdf_intent_grants_read_and_targets_pdf_mime() {
        val document = Uri.parse("content://com.example.documents/tree/root/document/score.pdf")

        val intent = pdfViewIntent(document)

        assertEquals(Intent.ACTION_VIEW, intent.action)
        assertEquals(document, intent.data)
        assertEquals("application/pdf", intent.type)
        // Read only: no write or persistable grant crosses to the viewer.
        assertEquals(Intent.FLAG_GRANT_READ_URI_PERMISSION, intent.flags)
    }

    @Test
    fun a_zip_library_pdf_is_found_as_a_shareable_content_uri() {
        val activity = Robolectric.buildActivity(ComponentActivity::class.java).setup().get()
        val pdf = File(activity.filesDir, "$ZIP_LIBRARY_DIR/Nutki/001 - Ave.pdf")
        pdf.parentFile!!.mkdirs()
        pdf.writeText("%PDF-1.4")

        val uri = PdfOpener(activity).find(zipLibraryHandle(activity.filesDir), "Nutki/001 - Ave.pdf")

        assertEquals("content", uri.scheme)
        assertEquals("${activity.packageName}.files", uri.authority)
        assertTrue(activity.contentResolver.openInputStream(uri)!!.use { it.readBytes() }.isNotEmpty())
    }
}
