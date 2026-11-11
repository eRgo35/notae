package com.c2yz.notae.ui.components

import androidx.compose.animation.EnterExitState
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.spring
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import com.c2yz.notae.ui.theme.NotaeMotion
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.material3.ripple
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * A One UI dialog: a card over a 40% black scrim, at most 560 wide, 26 radius and 20 spacing, ending in
 * [DialogButtons]. On a phone ([COMPACT_WIDTH]) it floats 12 dp above the bottom edge, within reach of the thumb; on a
 * wider window it is centred with 16 dp side margins. The scrim swallows input; it dismisses only if [onDismiss] is given.
 */
@Composable
fun NotaeDialog(
    dialogLabel: String,
    modifier: Modifier = Modifier,
    onDismiss: (() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) = ModalSurface(dialogLabel, onDismiss, bottomSheet = false, cardWidth = 560.dp, modifier = modifier, content = content)

/** Windows narrower than this are phones: dialogs float at the bottom, long forms take the whole screen. */
val COMPACT_WIDTH = 600.dp

/** From this width the library shows its side panel and the details sit beside the list (M3 expanded). */
val EXPANDED_WIDTH = 840.dp

/** The padding of a dialog that ends in [DialogButtons]: the buttons carry their own height below the content. */
val DIALOG_PADDING = PaddingValues(start = 24.dp, top = 26.dp, end = 24.dp, bottom = 8.dp)

/**
 * The scrim and card shared by [NotaeDialog] and [ActionSheet]. With [cardHeight] the card is that tall, or the window
 * less 16 dp above and below when that is shorter (the metadata sheet on a tablet). [fullScreen] fills the window, as
 * One UI's long forms do on a phone.
 *
 * Inside an [AnimatedOverlay] the scrim fades, a dialog grows in from 92% and a sheet or full-screen form rises from the
 * bottom; the back gesture shrinks the card as it goes ([LocalBackProgress]). A sheet that can be dismissed also follows
 * a drag down and closes when let go past [SHEET_DISMISS_FRACTION] of its height or flung.
 */
@Composable
internal fun ModalSurface(
    dialogLabel: String,
    onDismiss: (() -> Unit)?,
    bottomSheet: Boolean,
    cardWidth: Dp,
    modifier: Modifier = Modifier,
    padding: PaddingValues = DIALOG_PADDING,
    spacing: Dp = 20.dp,
    cardHeight: Dp? = null,
    fullScreen: Boolean = false,
    content: @Composable ColumnScope.() -> Unit,
) {
    val dismissLabel = stringResource(R.string.dismiss)
    BoxWithConstraints(modifier.fillMaxSize()) {
        // A phone's dialog floats at the bottom edge, as One UI's do, so its buttons are within reach.
        val floating = !bottomSheet && !fullScreen && cardHeight == null && maxWidth < COMPACT_WIDTH
        val overlay = LocalOverlayScope.current
        val backProgress = LocalBackProgress.current
        val density = LocalDensity.current
        var dragged by remember { mutableFloatStateOf(0f) }
        var cardPx by remember { mutableIntStateOf(0) }
        val backHold = remember { BackHold() }
        val draggable = bottomSheet && onDismiss != null
        val dragState = rememberDraggableState { delta -> dragged = (dragged + delta).coerceAtLeast(0f) }
        Box(
            Modifier
                .fillMaxSize()
                .then(overlay?.run { Modifier.animateEnterExit(fadeIn(NotaeMotion.medium()), fadeOut(NotaeMotion.short())) } ?: Modifier)
                .background(Color.Black.copy(alpha = 0.4f))
                .then(
                    if (onDismiss != null) {
                        Modifier
                            .semantics { contentDescription = dismissLabel }
                            .clickable(onClickLabel = dismissLabel, role = Role.Button, onClick = onDismiss)
                    } else {
                        Modifier.pointerInput(Unit) { detectTapGestures { } }
                    },
                ),
        )
        val shape = when {
            fullScreen -> RoundedCornerShape(0.dp)
            bottomSheet -> RoundedCornerShape(topStart = 26.dp, topEnd = 26.dp)
            else -> RoundedCornerShape(26.dp)
        }
        val alignment = when {
            bottomSheet || floating -> Alignment.BottomCenter
            else -> Alignment.Center
        }
        Column(
            Modifier
                .align(alignment)
                .then(
                    when {
                        fullScreen -> Modifier.fillMaxSize()
                        bottomSheet -> Modifier
                        floating -> Modifier.padding(start = 12.dp, end = 12.dp, bottom = 12.dp, top = 12.dp)
                        else -> Modifier.padding(horizontal = 16.dp)
                    },
                )
                .then(if (cardHeight != null && !fullScreen) Modifier.padding(vertical = 16.dp).heightIn(max = cardHeight).fillMaxHeight() else Modifier)
                .then(if (fullScreen) Modifier else Modifier.widthIn(max = cardWidth))
                .then(overlay?.run { Modifier.animateEnterExit(cardEnter(bottomSheet || fullScreen), cardExit(bottomSheet || fullScreen)) } ?: Modifier)
                .onSizeChanged { cardPx = it.height }
                .graphicsLayer {
                    val back = backHold.read(backProgress(), closing = overlay?.transition?.targetState == EnterExitState.PostExit)
                    val scale = 1f - BACK_SHRINK * back
                    scaleX = scale
                    scaleY = scale
                    translationY = dragged + if (bottomSheet) cardPx * BACK_DROP * back else 0f
                }
                .then(
                    if (draggable) {
                        Modifier.draggable(
                            dragState,
                            Orientation.Vertical,
                            onDragStopped = { velocity ->
                                if (dragged > cardPx * SHEET_DISMISS_FRACTION || velocity > with(density) { SHEET_FLING.toPx() }) {
                                    onDismiss?.invoke()
                                } else {
                                    animate(dragged, 0f, animationSpec = spring()) { value, _ -> dragged = value }
                                }
                            },
                        )
                    } else {
                        Modifier
                    },
                )
                .clip(shape)
                .background(NotaeTheme.colors.card)
                .pointerInput(Unit) { detectTapGestures { } }
                .semantics { paneTitle = dialogLabel }
                .padding(padding),
            verticalArrangement = Arrangement.spacedBy(spacing),
            content = content,
        )
    }
}

/** A sheet let go past this share of its height closes. */
private const val SHEET_DISMISS_FRACTION = 0.3f

/** A sheet flung down faster than this, per second, closes. */
private val SHEET_FLING = 1000.dp

/** How much the card shrinks, and a sheet drops by its height, at the end of the back gesture. */
private const val BACK_SHRINK = 0.05f
private const val BACK_DROP = 0.1f

/** A sheet or full-screen form rises from the bottom edge; a dialog grows in from 92% where it stands. */
private fun cardEnter(fromBottom: Boolean): EnterTransition =
    if (fromBottom) slideInVertically(NotaeMotion.medium()) { it } else fadeIn(NotaeMotion.medium()) + scaleIn(NotaeMotion.medium(), initialScale = 0.92f)

private fun cardExit(fromBottom: Boolean): ExitTransition =
    if (fromBottom) slideOutVertically(NotaeMotion.short()) { it } else fadeOut(NotaeMotion.short()) + scaleOut(NotaeMotion.short(), targetScale = 0.92f)

/**
 * One UI's dialog buttons: two text buttons sharing the card's width, split by a short divider, or three with a
 * [middle] one between them. On a phone three do not fit side by side, so they stack as full-width rows: [middle],
 * [confirm], then [dismiss]. [confirm] is in the accent colour, or the error colour when [destructive].
 */
@Composable
fun DialogButtons(
    dismiss: String,
    onDismiss: () -> Unit,
    confirm: String,
    onConfirm: () -> Unit,
    modifier: Modifier = Modifier,
    destructive: Boolean = false,
    confirmEnabled: Boolean = true,
    middle: String? = null,
    onMiddle: () -> Unit = {},
) {
    val c = NotaeTheme.colors
    val confirmColor = if (destructive) c.error else c.accentText
    if (middle != null && LocalConfiguration.current.screenWidthDp < COMPACT_WIDTH.value) {
        Column(modifier.fillMaxWidth()) {
            DialogTextButton(middle, onMiddle, c.accentText, Modifier.fillMaxWidth())
            DialogTextButton(confirm, onConfirm, confirmColor, Modifier.fillMaxWidth(), enabled = confirmEnabled)
            DialogTextButton(dismiss, onDismiss, c.accentText, Modifier.fillMaxWidth())
        }
        return
    }
    Row(modifier.fillMaxWidth().height(56.dp), verticalAlignment = Alignment.CenterVertically) {
        DialogTextButton(dismiss, onDismiss, c.accentText, Modifier.weight(1f))
        Box(Modifier.size(1.dp, 20.dp).background(c.divider))
        if (middle != null) {
            DialogTextButton(middle, onMiddle, c.accentText, Modifier.weight(1f))
            Box(Modifier.size(1.dp, 20.dp).background(c.divider))
        }
        DialogTextButton(confirm, onConfirm, confirmColor, Modifier.weight(1f), enabled = confirmEnabled)
    }
}

/** A 52 dp text button with a pill-shaped press, one half of [DialogButtons]. */
@Composable
private fun DialogTextButton(label: String, onClick: () -> Unit, color: Color, modifier: Modifier, enabled: Boolean = true) {
    val c = NotaeTheme.colors
    val shape = RoundedCornerShape(26.dp)
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    Box(
        modifier
            .height(52.dp)
            .clip(shape)
            .then(if (focused && enabled) Modifier.border(BorderStroke(2.dp, c.accent), shape) else Modifier)
            .clickable(source, ripple(color = color), enabled = enabled, role = Role.Button, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            label,
            Modifier.alpha(if (enabled) 1f else 0.45f).padding(horizontal = 8.dp),
            color = if (enabled) color else c.muted,
            fontSize = 16.sp,
            fontWeight = FontWeight.SemiBold,
            textAlign = TextAlign.Center,
            maxLines = 1,
        )
    }
}
