package com.c2yz.notae.ui.library

import android.graphics.Bitmap
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.test.junit4.v2.createComposeRule
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.GraphicsMode
import java.io.File

/** The thumbnail shown follows the row's path, including to none. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class ThumbnailsTest {
    @get:Rule
    val compose = createComposeRule()

    @get:Rule
    val dir = TemporaryFolder()

    private var path by mutableStateOf<String?>(null)

    private fun png(name: String, width: Int): String {
        val file = File(dir.root, name)
        val bitmap = Bitmap.createBitmap(width, width, Bitmap.Config.ARGB_8888)
        file.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        return file.path
    }

    /** Waits for the decode on the IO dispatcher, then lets the assertion report what is shown. */
    private fun settle(done: () -> Boolean) {
        compose.waitForIdle()
        runCatching { compose.waitUntil(5_000, done) }
    }

    @Test
    fun thumbnail_follows_its_path() {
        val a = png("a.png", 10)
        val b = png("b.png", 20)
        path = a
        var shown: ImageBitmap? = null
        compose.setContent { shown = rememberThumbnail(path).value }

        settle { shown?.width == 10 }
        assertEquals("first path", 10, shown?.width)
        path = b
        settle { shown?.width == 20 }
        assertEquals("after the path changed", 20, shown?.width)
        path = null
        compose.waitForIdle()

        assertNull(shown)
    }
}
