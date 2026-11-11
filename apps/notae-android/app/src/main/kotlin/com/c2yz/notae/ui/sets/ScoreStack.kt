package com.c2yz.notae.ui.sets

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.c2yz.notae.R
import com.c2yz.notae.ui.library.PagePreview
import com.c2yz.notae.ui.theme.NotaeTheme
import kotlin.math.sin

/** The stack's front page, for tests that measure it. */
internal const val STACK_FRONT_TAG = "stack-front"

/** A4: a page is 1.41 times as tall as wide. */
private const val PAGE_ASPECT = 1.41f

/** How far each page behind steps up and right, as a share of the page width. */
private const val STEP = 0.10f

/** How far the pages behind tilt, in degrees: the first one back leans left, the next right. */
private const val TILT = 5f

/** A mini stack's step and tilt: larger, so its second page still shows at 40 dp. */
private const val MINI_STEP = 0.15f
private const val MINI_TILT = 6f

/**
 * A set's artwork: the first pages of its first [pages] scores, the first score in front and the others stepping up
 * and right behind it by [step] of the page width, tilted by [tilt] degrees. A `null` preview is a blank page. With no
 * previews, a set that holds only notes ([empty] false) shows one note page, and an [empty] set a dashed empty page.
 * Pages keep the A4 shape and are sized for a full stack, so every stack in a grid has the same page size.
 */
@Composable
internal fun ScoreStack(
    previews: List<String?>,
    empty: Boolean,
    modifier: Modifier = Modifier,
    pages: Int = 3,
    step: Float = STEP,
    tilt: Float = TILT,
) {
    val c = NotaeTheme.colors
    BoxWithConstraints(modifier, contentAlignment = Alignment.Center) {
        val steps = pages - 1
        // Room for the steps behind and for a tilted page's corners.
        val lean = sin(Math.toRadians(tilt.toDouble())).toFloat()
        val pageWidth = minOf(
            maxWidth / (1f + step * steps + PAGE_ASPECT * lean),
            maxHeight / (PAGE_ASPECT + step * steps + lean),
        )
        val shown = previews.take(pages)
        val offset = pageWidth * step
        // The drawn stack is centred, whatever its depth.
        val shift = offset * (shown.size - 1).coerceAtLeast(0) / 2
        val radius = pageWidth * 0.06f
        val single = Modifier.testTag(STACK_FRONT_TAG).width(pageWidth).aspectRatio(1f / PAGE_ASPECT)
        when {
            shown.isNotEmpty() -> Unit
            empty -> EmptyPage(single, radius)
            else -> NotePage(single, radius)
        }
        for (index in shown.indices.reversed()) {
            val behind = index > 0
            val page = Modifier
                .offset(x = offset * index - shift, y = shift - offset * index)
                .then(if (behind) Modifier.rotate(if (index % 2 == 1) -tilt else tilt) else Modifier.testTag(STACK_FRONT_TAG))
                .width(pageWidth)
                .aspectRatio(1f / PAGE_ASPECT)
                // A visible edge, so a page behind reads as paper on the tile in light and in dark.
                .then(if (behind) Modifier.border(1.dp, c.onAccentSoft.copy(alpha = 0.3f), RoundedCornerShape(radius)) else Modifier)
            PagePreview(shown[index], page, radius)
        }
    }
}

/** A set's artwork on a small tile, as on set rows, the set sheet and the checklist: two pages on accent-soft. */
@Composable
internal fun MiniStack(previews: List<String?>, empty: Boolean, size: Dp) {
    Box(Modifier.size(size).clip(RoundedCornerShape(size * 0.28f)).background(NotaeTheme.colors.accentSoft)) {
        ScoreStack(previews, empty, Modifier.fillMaxSize().padding(size * 0.08f), pages = 2, step = MINI_STEP, tilt = MINI_TILT)
    }
}

/** A set of notes only: one page in the note tint with the note glyph, as a text note's tile. */
@Composable
private fun NotePage(modifier: Modifier, radius: Dp) {
    val c = NotaeTheme.colors
    BoxWithConstraints(modifier.clip(RoundedCornerShape(radius)).background(c.note), contentAlignment = Alignment.Center) {
        Icon(painterResource(R.drawable.ic_note), null, Modifier.size(minOf(40.dp, maxWidth / 2)), tint = c.onNote)
    }
}

/** An empty set's page: a dashed outline where its first score will lie. */
@Composable
private fun EmptyPage(modifier: Modifier, radius: Dp) {
    val color = NotaeTheme.colors.onAccentSoft.copy(alpha = 0.5f)
    Box(
        modifier.drawBehind {
            val width = maxOf(1.dp.toPx(), size.width * 0.015f)
            val dash = maxOf(2.dp.toPx(), size.width * 0.07f)
            drawRoundRect(
                color,
                topLeft = Offset(width / 2, width / 2),
                size = Size(size.width - width, size.height - width),
                cornerRadius = CornerRadius(radius.toPx()),
                style = Stroke(width, pathEffect = PathEffect.dashPathEffect(floatArrayOf(dash, dash * 0.7f))),
            )
        },
    )
}
