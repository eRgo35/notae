package com.c2yz.notae.nav

import androidx.compose.animation.EnterExitState
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.Transition
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.ClipOp
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInWindow
import androidx.compose.ui.unit.toSize
import androidx.navigation3.ui.LocalNavAnimatedContentScope
import com.c2yz.notae.ui.theme.NotaeMotion
import com.c2yz.notae.ui.theme.Tokens

/**
 * Which way a screen change goes. Navigation 3 keeps both screens still ([ScreenMotion]); each screen moves its own
 * parts by it: the content slides ([screenContent]) and the bars fade through in place ([screenBar], [screenFade]).
 */
internal enum class Change {
    /** A deeper screen comes in over the one below. */
    Push,

    /** Back: the screen on top goes, uncovering the one below. */
    Pop,

    /** The back gesture, following the finger: Back's slide, so a gesture started mid-change carries on from there. */
    PredictivePop,

    /** A section later in the rail's order. */
    Forward,

    /** A section earlier in the rail's order. */
    Backward,

    /** Setup and Loading: the whole screens fade through, and the parts do nothing. */
    Fade,
}

/** The bars of the screens on show, by key, so a screen knows which of its bars the other screen has too. */
internal class SharedBars {
    private val bars = mutableStateListOf<Pair<ScreenPart, String>>()

    fun add(bar: Pair<ScreenPart, String>) {
        bars += bar
    }

    fun remove(bar: Pair<ScreenPart, String>) {
        bars -= bar
    }

    fun elsewhere(part: ScreenPart, key: String): Boolean = bars.any { it.second == key && it.first !== part }
}

/**
 * One screen's part in the change it is in, recorded when its enter or exit starts ([start]), so a later change
 * starting on another screen never redirects it. Both progresses are 1 when the screen is shown and 0 when it is gone:
 * [slide] eased, for the content, and [time] linear in time (or in the back gesture), for the bars. A change interrupted
 * midway carries on from where the content and the bars were.
 */
internal class ScreenPart(
    private val slide: () -> Float,
    private val time: () -> Float,
    /** The end of the line: 1 to the right, -1 to the left. */
    val end: Float,
    val bars: SharedBars,
) {
    /** The content's bounds and the screen's origin, in the window. */
    var hole by mutableStateOf<Rect?>(null)
    var origin by mutableStateOf(Offset.Zero)

    /** The change this screen is in, its direction, and where its content was and is going (a new value redraws). */
    private var run by mutableStateOf<Run?>(null)

    /** Records the change now starting for this screen, coming in ([entering]) or going. */
    fun start(change: Change, entering: Boolean) {
        val from = if (run != null) away else when (change) {
            Change.Push, Change.Forward -> if (entering) 1f else 0f
            Change.Backward -> if (entering) -1f else 0f
            else -> 0f
        }
        val to = when {
            entering -> 0f
            change == Change.Pop || change == Change.PredictivePop || change == Change.Backward -> 1f
            change == Change.Forward -> -1f
            // The screen a deeper one covers stays where it is.
            else -> from
        }
        run = Run(change, entering, slide(), from, to)
    }

    private val change: Change get() = run?.change ?: Change.Fade

    /** How far this screen's own slide has gone, 0 to 1, from where it was when its change started. */
    private fun slid(run: Run): Float {
        val target = if (run.entering) 1f else 0f
        if (target == run.startSlide) return 1f
        return ((slide() - run.startSlide) / (target - run.startSlide)).coerceIn(0f, 1f)
    }

    /** The content's offset toward the end of the line, as a share of its width. */
    val away: Float get() = run?.let { it.startAway + (it.endAway - it.startAway) * slid(it) } ?: 0f

    /**
     * Whether this screen's bars show at all: in the later half of its time shown, which is the first half of the change
     * for the screen going and the second for the one coming, never both at once. By time alone, so an interrupted change
     * carries on.
     */
    val barsShown: Boolean
        get() = change == Change.Fade || if (run?.entering != false) time() >= HALF else time() > HALF

    /** A bar of this screen alone, or text that changes: fades out over the first half going, in over the second half coming. */
    val fadeAlpha: Float get() = if (change == Change.Fade) 1f else ((time() - HALF) / HALF).coerceIn(0f, 1f)
}

/** A screen's change: where its slide was when it started, and its content's offsets, as a share of its width, then and at its end. */
private data class Run(
    val change: Change,
    val entering: Boolean,
    val startSlide: Float,
    val startAway: Float,
    val endAway: Float,
)

private const val HALF = 0.5f

private val LocalScreenPart = staticCompositionLocalOf<ScreenPart?> { null }

/** Gives the screen drawn by [content], a Navigation 3 entry, its [ScreenPart] in the changes [motion] runs. */
@Composable
internal fun ScreenFrame(motion: ScreenMotion, content: @Composable () -> Unit) {
    val transition: Transition<EnterExitState> = LocalNavAnimatedContentScope.current.transition
    val shown = { state: EnterExitState -> if (state == EnterExitState.Visible) 1f else 0f }
    val slide = transition.animateFloat({ NotaeMotion.long() }, label = "screen slide") { shown(it) }
    val time = transition.animateFloat({ tween(Tokens.motionLongMillis, easing = LinearEasing) }, label = "screen time") { shown(it) }
    val part = remember(transition, slide, time, motion) { ScreenPart({ slide.value }, { time.value }, motion.end, motion.bars) }
    val entering = transition.targetState == EnterExitState.Visible
    // Applied before the change's first frame is drawn, after Navigation 3 has chosen how it goes.
    DisposableEffect(part, entering) {
        part.start(motion.change, entering)
        onDispose { }
    }
    CompositionLocalProvider(LocalScreenPart provides part, content = content)
}

/**
 * The screen's background, behind its bars and around its content: shown with the bars (the going screen's in the first
 * half of a change, the coming one's in the second) and never over the content's place, which the content fills with
 * its own background ([screenContent]). Outside Navigation 3 (a screen alone in a test) it is a plain background.
 */
internal fun Modifier.screenBackground(color: Color): Modifier = composed {
    val part = LocalScreenPart.current ?: return@composed background(color)
    onGloballyPositioned { part.origin = it.positionInWindow() }.drawBehind {
        if (!part.barsShown) return@drawBehind
        val hole = part.hole?.translate(-part.origin)
        if (hole == null) {
            drawRect(color)
        } else {
            clipRect(hole.left, hole.top, hole.right, hole.bottom, ClipOp.Difference) { drawRect(color) }
        }
    }
}

/**
 * A bar, or a control in one, by [key]: the rail, the bottom bar, the top bar's Search. When the other screen of a
 * change has a bar with the same [key], it does not move or fade: the going screen's shows for the first half of
 * the change and the coming screen's for the second. A bar only one screen has fades through ([screenFade]).
 */
internal fun Modifier.screenBar(key: String): Modifier = composed {
    val part = LocalScreenPart.current ?: return@composed this
    DisposableEffect(part, key) {
        val bar = part to key
        part.bars.add(bar)
        onDispose { part.bars.remove(bar) }
    }
    graphicsLayer { alpha = if (part.bars.elsewhere(part, key)) (if (part.barsShown) 1f else 0f) else part.fadeAlpha }
}

/**
 * A bar only this screen has, or text that changes with the screen (a section's title): the going screen's fades out
 * over the first half of a change and the coming screen's fades in over the second, so the two never show together.
 */
internal fun Modifier.screenFade(): Modifier = composed {
    val part = LocalScreenPart.current ?: return@composed this
    graphicsLayer { alpha = part.fadeAlpha }
}

/**
 * The screen's content, below its top bar: the part that slides, for the whole change, clipped to its own place and on
 * its own [color] so it covers the content it slides over.
 */
internal fun Modifier.screenContent(color: Color): Modifier = composed {
    val part = LocalScreenPart.current ?: return@composed this
    onGloballyPositioned { part.hole = Rect(it.positionInWindow(), it.size.toSize()) }
        .graphicsLayer { clip = part.away != 0f }
        .graphicsLayer { translationX = part.away * part.end * size.width }
        .background(color)
}
