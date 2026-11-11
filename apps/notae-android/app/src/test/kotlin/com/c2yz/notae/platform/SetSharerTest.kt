package com.c2yz.notae.platform

import android.app.Activity
import android.content.Intent
import android.net.Uri
import androidx.core.content.IntentCompat
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import java.io.File

@RunWith(RobolectricTestRunner::class)
class SetSharerTest {
    @Test
    fun buildsSendIntent() {
        val activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        val shared = File(activity.cacheDir, "shared").apply { mkdirs() }
        File(shared, "Old.notae-set").writeText("old")
        File(shared, "Mass.notae-set").writeText("stale")

        runBlocking { SetSharer(activity).share("Mass.notae-set", """{"kind":"set"}""", "Mass\n1. 12 – Ave") }

        val chooser = shadowOf(activity).nextStartedActivity
        assertEquals(Intent.ACTION_CHOOSER, chooser.action)
        val send = checkNotNull(IntentCompat.getParcelableExtra(chooser, Intent.EXTRA_INTENT, Intent::class.java))
        assertEquals(Intent.ACTION_SEND, send.action)
        assertEquals("application/json", send.type)
        assertEquals("Mass\n1. 12 – Ave", send.getStringExtra(Intent.EXTRA_TEXT))
        assertEquals("Mass", send.getStringExtra(Intent.EXTRA_TITLE))
        assertEquals(Intent.FLAG_GRANT_READ_URI_PERMISSION, send.flags and Intent.FLAG_GRANT_READ_URI_PERMISSION)
        val uri = checkNotNull(IntentCompat.getParcelableExtra(send, Intent.EXTRA_STREAM, Uri::class.java))
        assertEquals("content", uri.scheme)
        assertEquals("${activity.packageName}.files", uri.authority)
        // The cache file holds the json, and an older shared file is gone.
        assertEquals("""{"kind":"set"}""", File(shared, "Mass.notae-set").readText())
        assertFalse(File(shared, "Old.notae-set").exists())
    }
}
