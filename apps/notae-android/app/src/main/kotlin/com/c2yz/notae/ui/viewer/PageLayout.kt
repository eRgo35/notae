package com.c2yz.notae.ui.viewer

import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import com.c2yz.notae.core.ViewerLayout
import com.c2yz.notae.core.ViewerPages
import com.c2yz.notae.core.ViewerScale
import com.c2yz.notae.ui.components.HeightClass
import com.c2yz.notae.ui.components.WidthClass
import kotlin.math.min

/** How the viewer shows a score: [scroll]ed or turned, [twoPages] side by side, at [scale] (never `Auto`). */
data class ViewLook(val scroll: Boolean, val twoPages: Boolean, val scale: ViewerScale)

/**
 * The look from the stored choices; each `Auto` is what 0.8.0 did: two pages in a landscape window of EXPANDED width
 * and REGULAR height, else one; on a COMPACT height a scroll at fit width, else turned pages at fit page.
 */
fun resolveLook(
    layout: ViewerLayout,
    pages: ViewerPages,
    scale: ViewerScale,
    landscape: Boolean,
    width: WidthClass,
    height: HeightClass,
): ViewLook {
    val short = height == HeightClass.COMPACT
    return ViewLook(
        scroll = when (layout) {
            ViewerLayout.AUTO -> short
            ViewerLayout.PAGES -> false
            ViewerLayout.SCROLL -> true
        },
        twoPages = when (pages) {
            ViewerPages.AUTO -> landscape && width == WidthClass.EXPANDED && !short
            ViewerPages.ONE -> false
            ViewerPages.TWO -> true
        },
        scale = if (scale == ViewerScale.Auto) (if (short) ViewerScale.FitWidth else ViewerScale.FitPage) else scale,
    )
}

/** The pages shown side by side, in order: pairs 1–2, 3–4, … when [twoPages] (an odd last page alone), else one each. */
fun rowsFor(pageCount: Int, twoPages: Boolean): List<IntRange> {
    val step = if (twoPages) 2 else 1
    return (0 until pageCount step step).map { first -> first..min(first + step - 1, pageCount - 1) }
}

/** The row holding [page]; 0 when none does. */
fun rowOf(page: Int, rows: List<IntRange>): Int = rows.indexOfFirst { page in it }.coerceAtLeast(0)

private const val POINTS_PER_INCH = 72f

/**
 * The size in pixels of a row of pages (height ÷ width [aspects], printed widths [pointWidths]) sharing one height,
 * [gap] apart, at [scale] in [area]; [dpi] is the screen's, for a percentage of the printed size.
 */
fun rowSize(aspects: List<Float>, pointWidths: List<Float>, area: Size, gap: Float, scale: ViewerScale, dpi: Float): Size {
    if (aspects.isEmpty()) return Size.Zero
    val safe = aspects.map { it.coerceAtLeast(0.01f) }
    val gaps = gap * (safe.size - 1)
    // The row's width per pixel of shared height.
    val widthPerHeight = safe.sumOf { 1.0 / it }.toFloat()
    val fitWidthHeight = (area.width - gaps).coerceAtLeast(1f) / widthPerHeight
    val height = when (scale) {
        ViewerScale.FitWidth -> fitWidthHeight
        ViewerScale.FitHeight -> area.height
        is ViewerScale.Percent -> (pointWidths.firstOrNull() ?: 595f) / POINTS_PER_INCH * dpi * scale.value.toFloat() / 100f * safe[0]
        ViewerScale.FitPage, ViewerScale.Auto -> min(area.height, fitWidthHeight)
    }
    return Size(height * widthPerHeight + gaps, height)
}

/** Each page's box inside a row of [rowSize], left to right from (0, 0), [gap] apart. */
fun pageBoxes(aspects: List<Float>, rowSize: Size, gap: Float): List<Rect> {
    var left = 0f
    return aspects.map { aspect ->
        val width = rowSize.height / aspect.coerceAtLeast(0.01f)
        Rect(left, 0f, left + width, rowSize.height).also { left += width + gap }
    }
}
