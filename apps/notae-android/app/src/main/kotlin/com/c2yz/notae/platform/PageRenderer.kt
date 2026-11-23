package com.c2yz.notae.platform

import android.content.ContentResolver
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.pdf.PdfRenderer
import android.os.ParcelFileDescriptor
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.core.graphics.createBitmap
import androidx.core.net.toUri
import java.io.Closeable
import java.io.FileNotFoundException
import kotlin.math.max
import kotlin.math.roundToInt
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/** A score's pages for the viewer. */
interface PageSource : Closeable {
    val pageCount: Int

    /** Page [index]'s height divided by its width. */
    fun pageAspect(index: Int): Float

    /** Page [index], [widthPx] wide, on white. */
    suspend fun render(index: Int, widthPx: Int): ImageBitmap
}

/** Opens the PDF at `relativePath` below the library tree `treeUri`; throws what the provider or the PDF throws. */
typealias PageSourceFactory = suspend (treeUri: String, relativePath: String) -> PageSource

/**
 * Android's [PdfRenderer] over a library PDF, opened read-only. The renderer has one page open at a time, so a
 * [Mutex] serialises the renders, which run on [Dispatchers.IO]; the last [CACHE_SIZE] bitmaps are kept, which holds
 * the spread on screen and its neighbours.
 */
class PdfPageSource internal constructor(
    private val aspects: List<Float>,
    private val renderPage: (index: Int, widthPx: Int) -> Bitmap,
    private val closeRenderer: () -> Unit,
) : PageSource {
    private val lock = Mutex()
    private val cache = object : LinkedHashMap<Pair<Int, Int>, ImageBitmap>(CACHE_SIZE, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Pair<Int, Int>, ImageBitmap>?): Boolean = size > CACHE_SIZE
    }
    @Volatile private var closed = false

    override val pageCount: Int get() = aspects.size

    override fun pageAspect(index: Int): Float = aspects.getOrElse(index) { A4_ASPECT }

    // The lock is taken and released on IO, so a render never needs the main thread to finish.
    override suspend fun render(index: Int, widthPx: Int): ImageBitmap = withContext(Dispatchers.IO) {
        lock.withLock {
            cache[index to widthPx]?.let { return@withLock it }
            check(!closed) { "the score was closed" }
            renderPage(index, widthPx).asImageBitmap().also { cache[index to widthPx] = it }
        }
    }

    override fun close() {
        if (closed) return
        closed = true
        // Called on the main thread: never wait there. A render in progress keeps the lock, so the renderer closes
        // after it, on IO.
        CoroutineScope(Dispatchers.IO).launch {
            lock.withLock {
                cache.clear()
                closeRenderer()
            }
        }
    }

    companion object {
        private const val CACHE_SIZE = 6

        /** Page [index] of [renderer] at [widthPx] wide on white. */
        private fun renderPage(renderer: PdfRenderer, index: Int, widthPx: Int): Bitmap = renderer.openPage(index).use { page ->
            val height = max(1, (widthPx * page.height.toFloat() / max(1, page.width)).roundToInt())
            createBitmap(max(1, widthPx), height).also {
                it.eraseColor(Color.WHITE)
                page.render(it, null, null, PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY)
            }
        }
        private const val A4_ASPECT = 1.414f

        /** Opens the PDF at [relativePath] below [treeUri]; walks the tree on [Dispatchers.IO]. */
        suspend fun open(resolver: ContentResolver, treeUri: String, relativePath: String): PdfPageSource = withContext(Dispatchers.IO) {
            val document = documentUri(resolver, treeUri.toUri(), relativePath)
            val descriptor: ParcelFileDescriptor = resolver.openFileDescriptor(document, "r")
                ?: throw FileNotFoundException("no descriptor for $relativePath")
            // The renderer closes the descriptor, once it exists.
            val renderer = try {
                PdfRenderer(descriptor)
            } catch (error: Exception) {
                descriptor.close()
                throw error
            }
            try {
                check(renderer.pageCount > 0) { "PDF has no readable pages" }
                val aspects = (0 until renderer.pageCount).map { index ->
                    renderer.openPage(index).use { page -> page.height.toFloat() / max(1, page.width) }
                }
                PdfPageSource(aspects, { index, widthPx -> renderPage(renderer, index, widthPx) }, renderer::close)
            } catch (error: Exception) {
                renderer.close()
                throw error
            }
        }
    }
}
