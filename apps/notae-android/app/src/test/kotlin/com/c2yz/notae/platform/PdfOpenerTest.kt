package com.c2yz.notae.platform

import android.content.Intent
import android.net.Uri
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class PdfOpenerTest {
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
}
