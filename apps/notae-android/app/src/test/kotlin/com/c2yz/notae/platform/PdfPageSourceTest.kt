package com.c2yz.notae.platform

import android.graphics.Bitmap
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.shadows.ShadowLooper
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/** The PDF source's renders and its close, which runs on the main thread. */
@RunWith(RobolectricTestRunner::class)
class PdfPageSourceTest {
    @Test
    fun closing_during_a_render_returns_at_once_and_closes_after_it() {
        val rendering = CountDownLatch(1)
        val release = CountDownLatch(1)
        val events = mutableListOf<String>()
        val source = PdfPageSource(
            listOf(1.414f),
            listOf(595f),
            { _, width ->
                rendering.countDown()
                release.await(5, TimeUnit.SECONDS)
                synchronized(events) { events += "rendered" }
                Bitmap.createBitmap(width, width, Bitmap.Config.ARGB_8888)
            },
            { synchronized(events) { events += "closed" } },
        )
        // A page renders from composition, on the main thread.
        CoroutineScope(Dispatchers.Main.immediate).launch { runCatching { source.render(0, 10) } }
        assertTrue("the render started", rendering.await(5, TimeUnit.SECONDS))

        val started = System.nanoTime()
        source.close()
        assertTrue("close does not wait on the render", TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - started) < 1_000)
        assertFalse("the renderer stays open under the page", synchronized(events) { "closed" in events })

        release.countDown()
        val deadline = System.currentTimeMillis() + 5_000
        while (synchronized(events) { "closed" !in events } && System.currentTimeMillis() < deadline) {
            ShadowLooper.idleMainLooper()
            Thread.sleep(10)
        }
        assertEquals(listOf("rendered", "closed"), synchronized(events) { events.toList() })
    }

    @Test
    fun reports_each_pages_printed_width_and_a4_past_the_end() {
        val source = PdfPageSource(listOf(1.414f, 0.7f), listOf(595f, 842f), { _, w -> Bitmap.createBitmap(w, w, Bitmap.Config.ARGB_8888) }, {})
        assertEquals(842f, source.pageWidthPoints(1))
        assertEquals(595f, source.pageWidthPoints(5))
    }

    @Test
    fun the_cache_is_bounded_by_bytes_not_by_pages() = kotlinx.coroutines.runBlocking {
        val renders = mutableListOf<Int>()
        val source = PdfPageSource(List(5) { 1f }, List(5) { 595f }, { index, w ->
            renders += index
            Bitmap.createBitmap(w, w, Bitmap.Config.ARGB_8888)
        }, {})
        // Five 16 MB pages: more than the cache keeps, though fewer pages than it used to count.
        for (index in 0 until 5) source.render(index, 2048)
        source.render(0, 2048)
        assertEquals(listOf(0, 1, 2, 3, 4, 0), renders)
        source.render(4, 2048)
        assertEquals("the newest page is still kept", 6, renders.size)
    }
}
