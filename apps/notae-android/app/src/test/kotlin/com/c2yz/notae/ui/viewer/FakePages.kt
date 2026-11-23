package com.c2yz.notae.ui.viewer

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.geometry.Rect
import com.c2yz.notae.platform.PageSource
import kotlin.math.roundToInt

/** [count] blank A4 pages, rendered white, recording which pages were drawn. */
class FakePages(override val pageCount: Int, private val aspect: Float = 1.414f, private val widthPoints: Float = 595f) : PageSource {
    val rendered = mutableListOf<Int>()
    var closed = false

    /** How often the pages' aspect was read: once per composition of a page. */
    var aspectReads = 0

    override fun pageAspect(index: Int): Float = aspect.also { aspectReads++ }

    override fun pageWidthPoints(index: Int): Float = widthPoints

    override suspend fun render(index: Int, widthPx: Int): ImageBitmap {
        rendered += index
        val bitmap = ImageBitmap(widthPx, (widthPx * aspect).roundToInt().coerceAtLeast(1))
        Canvas(bitmap).drawRect(Rect(0f, 0f, bitmap.width.toFloat(), bitmap.height.toFloat()), Paint().apply { color = Color.White })
        return bitmap
    }

    override fun close() {
        closed = true
    }
}
