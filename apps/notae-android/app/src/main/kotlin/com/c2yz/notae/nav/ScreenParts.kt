package com.c2yz.notae.nav

import androidx.compose.animation.EnterExitState
import androidx.compose.animation.core.Transition
import androidx.compose.animation.core.animateFloat
import androidx.compose.foundation.background
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
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
import androidx.compose.ui.graphics.GraphicsLayerScope
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInWindow
import androidx.compose.ui.unit.toSize
import androidx.navigation3.ui.LocalNavAnimatedContentScope
import com.c2yz.notae.ui.theme.NotaeMotion

/**
 * Which way the screen change now running goes. Navigation 3 keeps both screens still ([ScreenMotion]); each screen
 * moves its own parts by it: the content slides ([screenContent]) and the bars stay where they are ([screenChrome]).
 */
internal enum class Change {
    /** A deeper screen comes in over the one below. */
    Push,

    /** Back: the screen on top goes, uncovering the one below. */
    Pop,

    /** The back gesture in progress. */
    PredictivePop,

    /** A section later in the rail's order. */
    Forward,

    /** A section earlier in the rail's order. */
    Backward,

    /** Setup and Loading: the whole screens fade through, and the parts do nothing. */
    Fade,
}

/**
 * One screen's part in the change now running: how far it is in ([progress], 1 at rest), and where its content sits,
 * so its background leaves the content's place to whatever is below while the content moves.
 */
internal class ScreenPart(
    private val transition: Transition<EnterExitState>,
    private val progress: State<Float>,
    private val motion: ScreenMotion,
) {
    /** The content's bounds and the screen's origin, in the window. */
    var hole by mutableStateOf<Rect?>(null)
    var origin by mutableStateOf(Offset.Zero)

    private val entering: Boolean get() = transition.targetState == EnterExitState.Visible

    /** The screen drawn over the other: the one coming in, or the one Back takes away. */
    private val onTop: Boolean
        get() = when (motion.change) {
            Change.Push, Change.Forward, Change.Backward -> entering
            Change.Pop, Change.PredictivePop -> !entering
            Change.Fade -> false
        }

    /** The bars of the screen on top fade in or out over the same bars, or another screen's, below them. */
    val chromeAlpha: Float get() = if (onTop) progress.value else 1f

    /** How far the content is from its place, as a share of its width (0 at rest). */
    private val away: Float get() = 1f - progress.value

    val contentMoving: Boolean get() = away != 0f && motion.change != Change.Fade

    /** Slides the content toward the end of the line, or shrinks it under the back gesture, as [Change] says. */
    fun GraphicsLayerScope.moveContent() {
        val width = size.width * motion.end * away
        when (motion.change) {
            Change.Push -> if (entering) translationX = width
            Change.Pop -> if (!entering) translationX = width
            Change.Forward -> translationX = if (entering) width else -width
            Change.Backward -> translationX = if (entering) -width else width
            Change.PredictivePop -> if (!entering) {
                scaleX = 1f - PREDICTIVE_SHRINK * away
                scaleY = scaleX
            }
            Change.Fade -> Unit
        }
    }
}

/** The back gesture shrinks the content it leaves to 90%, as it shrank the whole screen before. */
private const val PREDICTIVE_SHRINK = 0.1f

private val LocalScreenPart = staticCompositionLocalOf<ScreenPart?> { null }

/** Gives the screen drawn by [content], a Navigation 3 entry, its [ScreenPart] in the changes [motion] runs. */
@Composable
internal fun ScreenFrame(motion: ScreenMotion, content: @Composable () -> Unit) {
    val transition = LocalNavAnimatedContentScope.current.transition
    val progress = transition.animateFloat({ NotaeMotion.long() }, label = "screen") { if (it == EnterExitState.Visible) 1f else 0f }
    val part = remember(transition, progress, motion) { ScreenPart(transition, progress, motion) }
    CompositionLocalProvider(LocalScreenPart provides part, content = content)
}

/**
 * The screen's background, behind its bars and around its content. In a screen change it fades with the bars and leaves
 * out the content's place, which the content fills with its own background ([screenContent]). Outside Navigation 3 (a
 * screen alone in a test) it is a plain background.
 */
internal fun Modifier.screenBackground(color: Color): Modifier = composed {
    val part = LocalScreenPart.current ?: return@composed background(color)
    onGloballyPositioned { part.origin = it.positionInWindow() }.drawBehind {
        val alpha = part.chromeAlpha
        val hole = part.hole?.translate(-part.origin)
        if (hole == null) {
            drawRect(color, alpha = alpha)
        } else {
            clipRect(hole.left, hole.top, hole.right, hole.bottom, ClipOp.Difference) { drawRect(color, alpha = alpha) }
        }
    }
}

/**
 * The screen's bars (a rail, a top bar, a bottom bar, the banner): they never move. In a screen change the incoming
 * screen's bars fade in over the outgoing one's, and Back fades them out again, so bars both screens share stay put.
 */
internal fun Modifier.screenChrome(): Modifier = composed {
    val part = LocalScreenPart.current ?: return@composed this
    graphicsLayer { alpha = part.chromeAlpha }
}

/**
 * The screen's content, below its top bar: the part that slides in a screen change, clipped to its own place and on
 * its own [color] so it covers the content it slides over.
 */
internal fun Modifier.screenContent(color: Color): Modifier = composed {
    val part = LocalScreenPart.current ?: return@composed this
    onGloballyPositioned { part.hole = Rect(it.positionInWindow(), it.size.toSize()) }
        .graphicsLayer { clip = part.contentMoving }
        .graphicsLayer { with(part) { moveContent() } }
        .background(color)
}
