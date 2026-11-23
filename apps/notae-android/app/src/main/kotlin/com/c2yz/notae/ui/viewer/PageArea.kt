package com.c2yz.notae.ui.viewer

import androidx.compose.foundation.Image
import androidx.compose.foundation.focusable
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.foundation.gestures.rememberTransformableState
import androidx.compose.foundation.gestures.transformable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import com.c2yz.notae.platform.PageSource
import com.c2yz.notae.ui.components.HeightClass
import com.c2yz.notae.ui.components.WidthClass
import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.min
import kotlin.math.roundToInt

/** How the viewer lays pages out. */
enum class PageMode {
    /** One page fit to the screen. */
    ONE,

    /** Two pages side by side, paired 1–2, 3–4, …: a tablet in landscape. */
    TWO,

    /** Pages at full width, scrolled vertically: a short window (a phone on its side). */
    FIT_WIDTH,
}

/** Two pages only in a landscape window of EXPANDED width and REGULAR height; a COMPACT height fits to width. */
fun pageModeFor(landscape: Boolean, width: WidthClass, height: HeightClass): PageMode = when {
    height == HeightClass.COMPACT -> PageMode.FIT_WIDTH
    landscape && width == WidthClass.EXPANDED -> PageMode.TWO
    else -> PageMode.ONE
}

/** The pages shown together, in order: pairs in [PageMode.TWO] (an odd last page alone), else one page each. */
fun spreadsFor(pageCount: Int, mode: PageMode): List<IntRange> {
    val step = if (mode == PageMode.TWO) 2 else 1
    return (0 until pageCount step step).map { first -> first..min(first + step - 1, pageCount - 1) }
}

/** The spread holding [page]; 0 when none does. */
fun spreadOf(page: Int, spreads: List<IntRange>): Int = spreads.indexOfFirst { page in it }.coerceAtLeast(0)

/** The zoom of a spread: 1× (fit) to 4×, and the pan while zoomed. */
class PageZoom {
    var scale by mutableFloatStateOf(1f)
    var offset by mutableStateOf(Offset.Zero)

    fun reset() {
        scale = 1f
        offset = Offset.Zero
    }
}

private const val MAX_ZOOM = 4f
private const val EDGE = 0.2f
private const val SWIPE_FRACTION = 0.15f
private val PAGE_GAP = 8.dp

/**
 * The score's pages in [mode]: spread [spread] fit to the space, or, in [PageMode.FIT_WIDTH], every page at full
 * width scrolled vertically. A tap on the outer fifth of either side, a swipe, an arrow key or Page Up/Down turns
 * (by a spread); a tap in the middle third calls [onToggleBars]. A pinch zooms 1×–4× and a zoomed page pans instead
 * of turning; a double tap returns to fit. [overlay] draws over each page, given its box in the page area.
 */
@Composable
fun PageArea(
    source: PageSource,
    mode: PageMode,
    spread: Int,
    onSpread: (Int) -> Unit,
    onToggleBars: () -> Unit,
    modifier: Modifier = Modifier,
    zoom: PageZoom = remember { PageZoom() },
    overlay: @Composable (pageIndex: Int, pageBox: Rect) -> Unit = { _, _ -> },
) {
    val spreads = remember(source, mode) { spreadsFor(source.pageCount, mode) }
    val current by rememberUpdatedState(spread.coerceIn(0, (spreads.size - 1).coerceAtLeast(0)))
    val turn = { by: Int ->
        val next = (current + by).coerceIn(0, (spreads.size - 1).coerceAtLeast(0))
        if (next != current) {
            zoom.reset()
            onSpread(next)
        }
    }
    val keys = remember { FocusRequester() }
    LaunchedEffect(Unit) { keys.requestFocus() }
    val keyed = modifier
        .focusRequester(keys)
        .focusable()
        .onKeyEvent { event ->
            if (event.type != KeyEventType.KeyDown) return@onKeyEvent false
            when (event.key) {
                Key.DirectionRight, Key.DirectionDown, Key.PageDown -> turn(1).let { true }
                Key.DirectionLeft, Key.DirectionUp, Key.PageUp -> turn(-1).let { true }
                else -> false
            }
        }
    if (mode == PageMode.FIT_WIDTH) {
        ScrolledPages(source, current, onSpread, onToggleBars, keyed, overlay)
    } else {
        SpreadPages(source, spreads.getOrElse(current) { IntRange.EMPTY }, turn, onToggleBars, keyed, zoom, overlay)
    }
}

@Composable
private fun SpreadPages(
    source: PageSource,
    pages: IntRange,
    turn: (Int) -> Unit,
    onToggleBars: () -> Unit,
    modifier: Modifier,
    zoom: PageZoom,
    overlay: @Composable (pageIndex: Int, pageBox: Rect) -> Unit,
) {
    BoxWithConstraints(modifier.fillMaxSize()) {
        val width = constraints.maxWidth.toFloat()
        val height = constraints.maxHeight.toFloat()
        var dragged by remember { mutableFloatStateOf(0f) }
        val transform = rememberTransformableState { zoomChange, panChange, _ ->
            zoom.scale = (zoom.scale * zoomChange).coerceIn(1f, MAX_ZOOM)
            zoom.offset = if (zoom.scale == 1f) Offset.Zero else clampPan(zoom.offset + panChange, zoom.scale, width, height)
        }
        val drag = rememberDraggableState { delta -> dragged += delta }
        val gap = with(LocalDensity.current) { PAGE_GAP.toPx() }
        val boxes = remember(pages, width, height, gap) { pageBoxes(source, pages, width, height, gap) }
        Box(
            Modifier
                .fillMaxSize()
                .pointerInput(pages) {
                    detectTapGestures(
                        onDoubleTap = { zoom.reset() },
                        onTap = { at ->
                            when {
                                at.x < size.width * EDGE -> turn(-1)
                                at.x > size.width * (1 - EDGE) -> turn(1)
                                at.x > size.width / 3f && at.x < size.width * 2f / 3f -> onToggleBars()
                            }
                        },
                    )
                }
                .transformable(transform, canPan = { zoom.scale > 1f })
                .draggable(
                    drag,
                    Orientation.Horizontal,
                    enabled = zoom.scale == 1f,
                    onDragStarted = { dragged = 0f },
                    onDragStopped = {
                        if (abs(dragged) > width * SWIPE_FRACTION) turn(if (dragged < 0) 1 else -1)
                        dragged = 0f
                    },
                )
                .graphicsLayer {
                    scaleX = zoom.scale
                    scaleY = zoom.scale
                    translationX = zoom.offset.x
                    translationY = zoom.offset.y
                },
        ) {
            val density = LocalDensity.current
            for ((index, box) in boxes) {
                val renderWidth = (box.width * min(2f, ceil(zoom.scale))).roundToInt().coerceAtLeast(1)
                Box(
                    Modifier
                        .offset { IntOffset(box.left.roundToInt(), box.top.roundToInt()) }
                        .size(with(density) { box.width.toDp() }, with(density) { box.height.toDp() }),
                ) {
                    PageImage(source, index, renderWidth)
                    overlay(index, Rect(0f, 0f, box.width, box.height))
                }
            }
        }
    }
}

@Composable
private fun ScrolledPages(
    source: PageSource,
    page: Int,
    onSpread: (Int) -> Unit,
    onToggleBars: () -> Unit,
    modifier: Modifier,
    overlay: @Composable (pageIndex: Int, pageBox: Rect) -> Unit,
) {
    val list = rememberLazyListState(page)
    LaunchedEffect(page) { if (list.firstVisibleItemIndex != page) list.animateScrollToItem(page) }
    LaunchedEffect(list) { snapshotFlow { list.firstVisibleItemIndex }.collect(onSpread) }
    BoxWithConstraints(modifier.fillMaxSize()) {
        val widthPx = constraints.maxWidth.coerceAtLeast(1)
        LazyColumn(
            Modifier.fillMaxSize().pointerInput(Unit) {
                detectTapGestures { at -> if (at.x > size.width / 3f && at.x < size.width * 2f / 3f) onToggleBars() }
            },
            state = list,
            verticalArrangement = Arrangement.spacedBy(PAGE_GAP),
        ) {
            items(source.pageCount) { index ->
                val heightPx = widthPx * source.pageAspect(index)
                Box(Modifier.fillMaxWidth().height(with(LocalDensity.current) { heightPx.toDp() })) {
                    PageImage(source, index, widthPx)
                    overlay(index, Rect(0f, 0f, widthPx.toFloat(), heightPx))
                }
            }
        }
    }
}

@Composable
private fun PageImage(source: PageSource, index: Int, widthPx: Int) {
    val bitmap by produceState<ImageBitmap?>(null, source, index, widthPx) {
        value = runCatching { source.render(index, widthPx) }.getOrNull()
    }
    bitmap?.let { Image(it, null, Modifier.fillMaxSize(), contentScale = ContentScale.FillBounds) }
}

/** Each page of [pages] fit together into [width] × [height], centred, [gap] apart, in the area's pixels. */
private fun pageBoxes(source: PageSource, pages: IntRange, width: Float, height: Float, gap: Float): List<Pair<Int, Rect>> {
    if (pages.isEmpty() || width <= 0f || height <= 0f) return emptyList()
    val aspects = pages.map { source.pageAspect(it).coerceAtLeast(0.01f) }
    val gaps = gap * (aspects.size - 1)
    // A shared page height: the full height, or less when the pages side by side would be wider than the area.
    val pageHeight = min(height, (width - gaps) / aspects.sumOf { 1.0 / it }.toFloat())
    val widths = aspects.map { pageHeight / it }
    var left = (width - widths.sum() - gaps) / 2f
    val top = (height - pageHeight) / 2f
    return pages.zip(widths).map { (index, pageWidth) ->
        (index to Rect(left, top, left + pageWidth, top + pageHeight)).also { left += pageWidth + gap }
    }
}

/** Keeps a page zoomed by [scale] covering the area: it pans at most to its own edges. */
private fun clampPan(offset: Offset, scale: Float, width: Float, height: Float): Offset {
    val x = width * (scale - 1f) / 2f
    val y = height * (scale - 1f) / 2f
    return Offset(offset.x.coerceIn(-x, x), offset.y.coerceIn(-y, y))
}
