package com.c2yz.notae.ui.viewer

import androidx.compose.foundation.Image
import androidx.compose.foundation.focusable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.width
import androidx.compose.ui.Alignment
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.PagerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import com.c2yz.notae.platform.PageSource
import kotlin.math.roundToInt
import kotlinx.coroutines.launch

private const val EDGE = 0.2f
private val PAGE_GAP = 8.dp

/**
 * The score's pages in [look]: turned a row at a time (one page or a pair) or, when [ViewLook.scroll], one row under
 * another. [page] is the first page on screen and [onPage] reports a new one. A tap on the outer fifth of either side,
 * an arrow key or Page Up/Down turns, animated; a tap in the middle third calls [onToggleBars]. Two fingers zoom
 * anywhere ([pinchZoom]) on top of the chosen scale and a double tap returns to it. [overlay] draws over each page,
 * given its box in the page's own coordinates.
 */
@Composable
fun PageArea(
    source: PageSource,
    look: ViewLook,
    page: Int,
    onPage: (Int) -> Unit,
    onToggleBars: () -> Unit,
    modifier: Modifier = Modifier,
    zoom: PageZoom = remember { PageZoom() },
    overlay: @Composable (pageIndex: Int, pageBox: Rect) -> Unit = { _, _ -> },
) {
    val rows = remember(source, look.twoPages) { rowsFor(source.pageCount, look.twoPages) }
    val keys = remember { FocusRequester() }
    LaunchedEffect(Unit) { keys.requestFocus() }
    val focused = modifier.focusRequester(keys).focusable()
    // A new layout starts at the chosen scale, on the row holding the first page.
    key(look) {
        LaunchedEffect(Unit) { zoom.reset() }
        if (look.scroll) {
            ScrolledRows(source, rows, look, rowOf(page, rows), onPage, onToggleBars, focused, zoom, overlay)
        } else {
            PagedRows(source, rows, look, rowOf(page, rows), onPage, onToggleBars, focused, zoom, overlay)
        }
    }
}

/** Arrow keys and Page Up/Down call [turn] with -1 or 1. */
private fun Modifier.turnKeys(turn: (Int) -> Unit): Modifier = onKeyEvent { event ->
    if (event.type != KeyEventType.KeyDown) return@onKeyEvent false
    when (event.key) {
        Key.DirectionRight, Key.DirectionDown, Key.PageDown -> turn(1).let { true }
        Key.DirectionLeft, Key.DirectionUp, Key.PageUp -> turn(-1).let { true }
        else -> false
    }
}

/** Taps: the outer fifths call [turn], the middle third [onToggleBars], a double tap [onDoubleTap]. */
private fun Modifier.pageTaps(turn: (Int) -> Unit, onToggleBars: () -> Unit, onDoubleTap: () -> Unit): Modifier =
    pointerInput(Unit) {
        detectTapGestures(
            onDoubleTap = { onDoubleTap() },
            onTap = { at ->
                when {
                    at.x < size.width * EDGE -> turn(-1)
                    at.x > size.width * (1 - EDGE) -> turn(1)
                    at.x > size.width / 3f && at.x < size.width * 2f / 3f -> onToggleBars()
                }
            },
        )
    }

/** The screen's dots per inch, for a percentage of the printed size. */
@Composable
private fun screenDpi(): Float = LocalContext.current.resources.displayMetrics.xdpi.takeIf { it > 0f } ?: 160f

@Composable
private fun PagedRows(
    source: PageSource,
    rows: List<IntRange>,
    look: ViewLook,
    initialRow: Int,
    onPage: (Int) -> Unit,
    onToggleBars: () -> Unit,
    modifier: Modifier,
    zoom: PageZoom,
    overlay: @Composable (pageIndex: Int, pageBox: Rect) -> Unit,
) {
    val pager = remember(rows) { PagerState(initialRow.coerceIn(0, (rows.size - 1).coerceAtLeast(0))) { rows.size } }
    val scope = rememberCoroutineScope()
    val report by rememberUpdatedState(onPage)
    LaunchedEffect(pager) {
        var settled = pager.settledPage
        snapshotFlow { pager.settledPage }.collect { row ->
            if (row != settled) zoom.reset()
            settled = row
            rows.getOrNull(row)?.let { report(it.first) }
        }
    }
    val turn = { by: Int ->
        val next = (pager.targetPage + by).coerceIn(0, (rows.size - 1).coerceAtLeast(0))
        if (next != pager.targetPage) {
            zoom.reset()
            scope.launch { pager.animateScrollToPage(next) }
        }
    }
    val dpi = screenDpi()
    val gap = with(LocalDensity.current) { PAGE_GAP.toPx() }
    BoxWithConstraints(modifier.fillMaxSize().turnKeys(turn)) {
        val area = Size(constraints.maxWidth.toFloat(), constraints.maxHeight.toFloat())
        // The size of row [index] at the chosen scale, before the pinch.
        val baseSize = { index: Int ->
            rows.getOrNull(index)?.let { row ->
                rowSize(row.map(source::pageAspect), row.map(source::pageWidthPoints), area, gap, look.scale, dpi)
            } ?: Size.Zero
        }
        val shown = baseSize(pager.currentPage) * zoom.factor
        val first = rows.getOrNull(pager.currentPage)?.first
        SideEffect { if (first != null) zoom.percent = printedPercent(source, first, shown.height, dpi) }
        val overflowX = shown.width > area.width + 0.5f
        val overflowY = shown.height > area.height + 0.5f
        val onPinch = remember(area, rows, look) {
            { centroid: Offset, change: Float, pan: Offset ->
                val base = baseSize(pager.currentPage)
                val old = zoom.factor
                val new = (old * change).coerceIn(MIN_ZOOM, MAX_ZOOM)
                val before = base * old
                val after = base * new
                // The content point under the fingers before the move stays under them.
                val start = Offset(contentStart(before.width, area.width, zoom.offset.x), contentStart(before.height, area.height, zoom.offset.y))
                val point = centroid - pan - start
                val newStart = centroid - point * (new / old)
                zoom.factor = new
                zoom.offset = Offset(clampPan(-newStart.x, after.width, area.width), clampPan(-newStart.y, after.height, area.height))
            }
        }
        Box(
            Modifier
                .fillMaxSize()
                .pinchZoom(onPinch)
                .pageTaps(turn, onToggleBars) { zoom.reset() }
                .then(
                    if (overflowX || overflowY) {
                        Modifier.pointerInput(overflowX, overflowY, shown) {
                            detectDragGestures { change, drag ->
                                change.consume()
                                val pan = zoom.offset - drag
                                zoom.offset = Offset(clampPan(pan.x, shown.width, area.width), clampPan(pan.y, shown.height, area.height))
                            }
                        }
                    } else {
                        Modifier
                    },
                ),
        ) {
            HorizontalPager(pager, Modifier.fillMaxSize(), userScrollEnabled = !overflowX, key = { rows[it].first }) { index ->
                val current = index == pager.currentPage
                val factor = if (current) zoom.factor else 1f
                val pan = if (current) zoom.offset else Offset.Zero
                val base = baseSize(index)
                val size = base * factor
                val origin = Offset(contentStart(size.width, area.width, pan.x), contentStart(size.height, area.height, pan.y))
                val row = rows[index]
                val aspects = row.map(source::pageAspect)
                Box(Modifier.fillMaxSize().clipToBounds()) {
                    val fitBoxes = pageBoxes(aspects, base, gap)
                    pageBoxes(aspects, size, gap * factor).forEachIndexed { at, box ->
                        val renderWidth = (fitBoxes[at].width * if (factor > 1f) 2f else 1f).coerceAtMost(area.width * 2f)
                        PageBox(source, row.first + at, box.translate(origin), renderWidth.roundToInt(), overlay)
                    }
                }
            }
        }
    }
}

/** Page [index] drawn into [box] (area pixels), rendered [renderWidth] wide, with its [overlay]. */
@Composable
private fun PageBox(
    source: PageSource,
    index: Int,
    box: Rect,
    renderWidth: Int,
    overlay: @Composable (pageIndex: Int, pageBox: Rect) -> Unit,
) {
    val density = LocalDensity.current
    Box(
        Modifier
            .offset { IntOffset(box.left.roundToInt(), box.top.roundToInt()) }
            .size(with(density) { box.width.toDp() }, with(density) { box.height.toDp() }),
    ) {
        PageImage(source, index, renderWidth.coerceAtLeast(1))
        overlay(index, Rect(0f, 0f, box.width, box.height))
    }
}

@Composable
private fun ScrolledRows(
    source: PageSource,
    rows: List<IntRange>,
    look: ViewLook,
    initialRow: Int,
    onPage: (Int) -> Unit,
    onToggleBars: () -> Unit,
    modifier: Modifier,
    zoom: PageZoom,
    overlay: @Composable (pageIndex: Int, pageBox: Rect) -> Unit,
) {
    val list = remember(rows) { LazyListState(initialRow.coerceIn(0, (rows.size - 1).coerceAtLeast(0))) }
    val across = rememberScrollState()
    val scope = rememberCoroutineScope()
    val report by rememberUpdatedState(onPage)
    LaunchedEffect(list) { snapshotFlow { list.firstVisibleItemIndex }.collect { row -> rows.getOrNull(row)?.let { report(it.first) } } }
    val turn = { by: Int ->
        val first = list.firstVisibleItemIndex
        // Back from part way down a row goes to that row's top first.
        val from = if (by < 0 && list.firstVisibleItemScrollOffset > 0) first + 1 else first
        val next = (from + by).coerceIn(0, (rows.size - 1).coerceAtLeast(0))
        scope.launch { list.animateScrollToItem(next) }
        Unit
    }
    val dpi = screenDpi()
    val density = LocalDensity.current
    val gap = with(density) { PAGE_GAP.toPx() }
    BoxWithConstraints(modifier.fillMaxSize().turnKeys(turn)) {
        val area = Size(constraints.maxWidth.toFloat(), constraints.maxHeight.toFloat())
        val baseSizes = remember(rows, look, area, dpi) {
            rows.map { row -> rowSize(row.map(source::pageAspect), row.map(source::pageWidthPoints), area, gap, look.scale, dpi) }
        }
        val widest = (baseSizes.maxOfOrNull { it.width } ?: 0f) * zoom.factor
        val contentWidth = maxOf(area.width, widest)
        val firstRow = list.firstVisibleItemIndex
        val firstShown = (baseSizes.getOrNull(firstRow)?.height ?: 0f) * zoom.factor
        SideEffect { rows.getOrNull(firstRow)?.let { zoom.percent = printedPercent(source, it.first, firstShown, dpi) } }
        val onPinch = remember(list, across) {
            { centroid: Offset, change: Float, pan: Offset ->
                val old = zoom.factor
                val new = (old * change).coerceIn(MIN_ZOOM, MAX_ZOOM)
                val k = new / old
                zoom.factor = new
                // Rows grow from the point under the fingers: scroll by what grows above and left of it.
                val down = (list.firstVisibleItemScrollOffset + centroid.y) * (k - 1f) - pan.y
                val right = (across.value + centroid.x) * (k - 1f) - pan.x
                list.dispatchRawDelta(down)
                across.dispatchRawDelta(right)
                Unit
            }
        }
        Box(Modifier.fillMaxSize().pinchZoom(onPinch).pageTaps(turn, onToggleBars) { zoom.reset() }) {
            Box(Modifier.fillMaxSize().horizontalScroll(across, enabled = contentWidth > area.width + 0.5f)) {
                LazyColumn(
                    Modifier.width(with(density) { contentWidth.toDp() }).fillMaxHeight(),
                    state = list,
                    verticalArrangement = Arrangement.spacedBy(PAGE_GAP),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    items(rows.size, key = { rows[it].first }) { index ->
                        val row = rows[index]
                        val aspects = row.map(source::pageAspect)
                        val base = baseSizes[index]
                        val size = base * zoom.factor
                        val fitBoxes = pageBoxes(aspects, base, gap)
                        Box(Modifier.size(with(density) { size.width.toDp() }, with(density) { size.height.toDp() })) {
                            pageBoxes(aspects, size, gap * zoom.factor).forEachIndexed { at, box ->
                                val renderWidth = (fitBoxes[at].width * if (zoom.factor > 1f) 2f else 1f).coerceAtMost(area.width * 2f)
                                PageBox(source, row.first + at, box, renderWidth.roundToInt(), overlay)
                            }
                        }
                    }
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
