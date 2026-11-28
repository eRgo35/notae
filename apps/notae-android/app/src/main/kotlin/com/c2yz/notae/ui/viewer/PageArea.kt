package com.c2yz.notae.ui.viewer

import androidx.compose.foundation.Image
import androidx.compose.foundation.focusable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.rememberScrollableState
import androidx.compose.foundation.gestures.scrollable
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.ui.Alignment
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.PagerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
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
import androidx.compose.ui.layout.layout
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import com.c2yz.notae.core.ViewerScale
import com.c2yz.notae.platform.PageSource
import kotlin.math.roundToInt
import kotlinx.coroutines.launch

private const val EDGE = 0.2f
private val PAGE_GAP = 8.dp

/**
 * The score's pages in [look]: turned a row at a time (one page or a pair) or, when [ViewLook.scroll], one row under
 * another. [page] is the page being read and [onPage] reports a new one: the first of a row turned to, while the row
 * on screen holding [page] keeps it. A tap on the outer fifth of either side, an arrow key or Page Up/Down turns,
 * animated; a tap in the middle third calls [onToggleBars]. Two fingers zoom anywhere ([pinchZoom]) on top of the
 * chosen scale, from half the fit to four times the printed size, and a double tap returns to it. [overlay] draws over
 * each page, sized to it. A turn past the last page, or before the first, calls [onBeyond] with 1 or -1 instead.
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
    onBeyond: (Int) -> Unit = {},
    overlay: @Composable (pageIndex: Int) -> Unit = {},
) {
    val rows = remember(source, look.twoPages) { rowsFor(source.pageCount, look.twoPages) }
    val keys = remember { FocusRequester() }
    LaunchedEffect(Unit) { keys.requestFocus() }
    val focused = modifier.focusRequester(keys).focusable()
    val reading by rememberUpdatedState(page)
    val report by rememberUpdatedState(onPage)
    val shown = remember(rows) { { first: Int -> if (reading !in rows[rowOf(first, rows)]) report(first) } }
    val beyond by rememberUpdatedState(onBeyond)
    // A new layout starts at the chosen scale from its first frame, on the row holding the page.
    val fresh = remember(look) {
        zoom.reset()
        look
    }
    key(fresh) {
        if (look.scroll) {
            ScrolledRows(source, rows, look, rowOf(page, rows), shown, onToggleBars, { beyond(it) }, focused, zoom, overlay)
        } else {
            PagedRows(source, rows, look, rowOf(page, rows), shown, onToggleBars, { beyond(it) }, focused, zoom, overlay)
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

/** How a look's rows are sized in an [area]: at the chosen scale, and the fit and printed sizes that bound the zoom. */
private class RowSizes(source: PageSource, rows: List<IntRange>, look: ViewLook, val area: Size, gap: Float, dpi: Float) {
    private val capacity = if (look.twoPages) 2 else 1
    private val sizeAt = { scale: ViewerScale ->
        rows.map { row -> rowSize(row.map(source::pageAspect), row.map(source::pageWidthPoints), area, gap, scale, dpi, capacity) }
    }
    val base = sizeAt(look.scale)
    private val fit = sizeAt(ViewerScale.FitPage)
    private val printed = sizeAt(ViewerScale.Percent(100.toUShort()))

    fun of(row: Int): Size = base.getOrNull(row) ?: Size.Zero

    /** Row [row] pinched by [factor] as a percentage of its printed size. */
    fun percent(row: Int, factor: Float): Int {
        val print = printed.getOrNull(row)?.height ?: 0f
        return if (print > 0f) (of(row).height * factor / print * 100f).roundToInt() else 100
    }

    /** The pinch factors row [row] may take. */
    fun bounds(row: Int): ClosedFloatingPointRange<Float> =
        zoomBounds(of(row).height, fit.getOrNull(row)?.height ?: 0f, printed.getOrNull(row)?.height ?: 0f)
}

@Composable
private fun PagedRows(
    source: PageSource,
    rows: List<IntRange>,
    look: ViewLook,
    initialRow: Int,
    onPage: (Int) -> Unit,
    onToggleBars: () -> Unit,
    onBeyond: (Int) -> Unit,
    modifier: Modifier,
    zoom: PageZoom,
    overlay: @Composable (pageIndex: Int) -> Unit,
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
    val turn: (Int) -> Unit = { by ->
        val next = (pager.targetPage + by).coerceIn(0, (rows.size - 1).coerceAtLeast(0))
        if (next == pager.targetPage) {
            onBeyond(by)
        } else {
            zoom.reset()
            scope.launch { pager.animateScrollToPage(next) }
        }
    }
    val dpi = screenDpi()
    val gap = with(LocalDensity.current) { PAGE_GAP.toPx() }
    BoxWithConstraints(modifier.fillMaxSize().turnKeys(turn)) {
        val area = Size(constraints.maxWidth.toFloat(), constraints.maxHeight.toFloat())
        val sizes = remember(rows, look, area, gap, dpi) { RowSizes(source, rows, look, area, gap, dpi) }
        // Sizes during a pinch are read while laying out, so a pinch moves the pages without composing them again.
        val shown = { sizes.of(pager.currentPage) * zoom.factor }
        val overflowX by remember(sizes) { derivedStateOf { shown().width > area.width + 0.5f } }
        val overflowY by remember(sizes) { derivedStateOf { shown().height > area.height + 0.5f } }
        LaunchedEffect(sizes) { snapshotFlow { sizes.percent(pager.currentPage, zoom.factor) }.collect { zoom.percent = it } }
        val onPinch = remember(sizes) {
            { centroid: Offset, change: Float, pan: Offset ->
                val base = sizes.of(pager.currentPage)
                val old = zoom.factor
                val new = (old * change).coerceIn(sizes.bounds(pager.currentPage))
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
                        Modifier.pointerInput(sizes) {
                            detectDragGestures { change, drag ->
                                change.consume()
                                val size = shown()
                                val pan = zoom.offset - drag
                                zoom.offset = Offset(clampPan(pan.x, size.width, area.width), clampPan(pan.y, size.height, area.height))
                            }
                        }
                    } else {
                        Modifier
                    },
                ),
        ) {
            HorizontalPager(pager, Modifier.fillMaxSize(), userScrollEnabled = !overflowX, key = { rows[it].first }) { index ->
                val current = index == pager.currentPage
                val zoomed by remember(current) { derivedStateOf { current && zoom.factor > 1f } }
                val row = rows[index]
                val aspects = row.map(source::pageAspect)
                val base = sizes.of(index)
                val fitBoxes = pageBoxes(aspects, base, gap)
                Box(Modifier.fillMaxSize().clipToBounds()) {
                    aspects.indices.forEach { at ->
                        PageBox(source, row.first + at, renderWidth(fitBoxes[at].width, zoomed, aspects[at], area.width), overlay) {
                            val factor = if (current) zoom.factor else 1f
                            val pan = if (current) zoom.offset else Offset.Zero
                            val size = base * factor
                            val origin = Offset(contentStart(size.width, area.width, pan.x), contentStart(size.height, area.height, pan.y))
                            pageBoxes(aspects, size, gap * factor)[at].translate(origin)
                        }
                    }
                }
            }
        }
    }
}

/** Page [index], rendered [renderWidth] wide, with its [overlay], in the [box] its parent's pixels give while laying out. */
@Composable
private fun PageBox(
    source: PageSource,
    index: Int,
    renderWidth: Int,
    overlay: @Composable (pageIndex: Int) -> Unit,
    box: () -> Rect,
) {
    Box(
        Modifier.layout { measurable, constraints ->
            val at = box()
            val placeable = measurable.measure(Constraints.fixed(at.width.roundToInt().coerceAtLeast(0), at.height.roundToInt().coerceAtLeast(0)))
            layout(constraints.maxWidth, constraints.maxHeight) { placeable.place(at.left.roundToInt(), at.top.roundToInt()) }
        },
    ) {
        PageImage(source, index, renderWidth.coerceAtLeast(1))
        overlay(index)
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
    onBeyond: (Int) -> Unit,
    modifier: Modifier,
    zoom: PageZoom,
    overlay: @Composable (pageIndex: Int) -> Unit,
) {
    val list = remember(rows) { LazyListState(initialRow.coerceIn(0, (rows.size - 1).coerceAtLeast(0))) }
    val scope = rememberCoroutineScope()
    val report by rememberUpdatedState(onPage)
    // At the end of the score the last row may be too short to reach the top: then it is the row being read.
    val shownRow = { if (!list.canScrollForward && list.canScrollBackward) rows.lastIndex else list.firstVisibleItemIndex }
    LaunchedEffect(list) { snapshotFlow { shownRow() }.collect { row -> rows.getOrNull(row)?.let { report(it.first) } } }
    val turn: (Int) -> Unit = { by ->
        if (if (by > 0) !list.canScrollForward else !list.canScrollBackward) onBeyond(by)
        val first = shownRow()
        // Back from part way down a row goes to that row's top first.
        val from = if (by < 0 && first == list.firstVisibleItemIndex && list.firstVisibleItemScrollOffset > 0) first + 1 else first
        val next = (from + by).coerceIn(0, (rows.size - 1).coerceAtLeast(0))
        scope.launch { list.animateScrollToItem(next) }
    }
    val dpi = screenDpi()
    val gap = with(LocalDensity.current) { PAGE_GAP.toPx() }
    BoxWithConstraints(modifier.fillMaxSize().turnKeys(turn)) {
        val area = Size(constraints.maxWidth.toFloat(), constraints.maxHeight.toFloat())
        val sizes = remember(rows, look, area, gap, dpi) { RowSizes(source, rows, look, area, gap, dpi) }
        val widest = sizes.base.maxOfOrNull { it.width } ?: 0f
        // Rows are centred in the content, as wide as the widest row or the area. Across, it is panned by zoom.offset.x.
        val contentWidth = { factor: Float -> maxOf(area.width, widest * factor) }
        val across = { clampPan(zoom.offset.x, contentWidth(zoom.factor), area.width) }
        LaunchedEffect(sizes) { snapshotFlow { sizes.percent(list.firstVisibleItemIndex, zoom.factor) }.collect { zoom.percent = it } }
        val onPinch = remember(list, sizes) {
            { centroid: Offset, change: Float, pan: Offset ->
                val old = zoom.factor
                val new = (old * change).coerceIn(sizes.bounds(list.firstVisibleItemIndex))
                val k = new / old
                // The point under the fingers before the move, from the content's centre line, grows by k and stays put.
                val fromCentre = centroid.x - pan.x - (contentWidth(old) / 2f - across())
                zoom.factor = new
                zoom.offset = Offset(clampPan(contentWidth(new) / 2f + fromCentre * k - centroid.x, contentWidth(new), area.width), 0f)
                // Rows grow from the point under the fingers: scroll by what grows above it.
                list.dispatchRawDelta((list.firstVisibleItemScrollOffset + centroid.y) * (k - 1f) - pan.y)
                Unit
            }
        }
        val panAcross = rememberScrollableState { delta ->
            val old = across()
            val new = clampPan(old - delta, contentWidth(zoom.factor), area.width)
            zoom.offset = Offset(new, 0f)
            old - new
        }
        val zoomed by remember { derivedStateOf { zoom.factor > 1f } }
        Box(
            Modifier
                .fillMaxSize()
                .clipToBounds()
                .pinchZoom(onPinch)
                .pageTaps(turn, onToggleBars) { zoom.reset() }
                .scrollable(panAcross, Orientation.Horizontal),
        ) {
            LazyColumn(
                Modifier
                    .layout { measurable, constraints ->
                        val width = contentWidth(zoom.factor).roundToInt()
                        val placeable = measurable.measure(constraints.copy(minWidth = width, maxWidth = width))
                        layout(constraints.maxWidth, placeable.height) { placeable.place(-across().roundToInt(), 0) }
                    }
                    .fillMaxHeight(),
                state = list,
                verticalArrangement = Arrangement.spacedBy(PAGE_GAP),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                items(rows.size, key = { rows[it].first }) { index ->
                    val row = rows[index]
                    val aspects = row.map(source::pageAspect)
                    val base = sizes.of(index)
                    val fitBoxes = pageBoxes(aspects, base, gap)
                    Box(
                        Modifier.layout { measurable, _ ->
                            val size = base * zoom.factor
                            val width = size.width.roundToInt()
                            val height = size.height.roundToInt()
                            val placeable = measurable.measure(Constraints.fixed(width, height))
                            layout(width, height) { placeable.place(0, 0) }
                        },
                    ) {
                        aspects.indices.forEach { at ->
                            PageBox(source, row.first + at, renderWidth(fitBoxes[at].width, zoomed, aspects[at], area.width), overlay) {
                                pageBoxes(aspects, base * zoom.factor, gap * zoom.factor)[at]
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
