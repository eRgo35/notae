package com.c2yz.notae.platform

import android.app.Activity
import android.content.Intent
import android.net.Uri
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.shadows.ShadowToast

@RunWith(RobolectricTestRunner::class)
class UrlOpenerTest {
    private val url = "https://github.com/eRgo35/notae/blob/main/PRIVACY.md"

    @Test
    fun url_opens_with_action_view() {
        val activity = Robolectric.buildActivity(Activity::class.java).setup().get()

        openUrl(activity, url)

        val started = shadowOf(activity).nextStartedActivity
        assertEquals(Intent.ACTION_VIEW, started.action)
        assertEquals(Uri.parse(url), started.data)
        assertNull(ShadowToast.getTextOfLatestToast())
    }

    @Test
    fun no_browser_shows_a_toast_instead_of_crashing() {
        val activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        // Nothing resolves under Robolectric: startActivity throws ActivityNotFoundException, as on a device without a browser.
        shadowOf(activity.application).checkActivities(true)

        openUrl(activity, url)

        assertEquals("No app can open the link", ShadowToast.getTextOfLatestToast())
    }
}
