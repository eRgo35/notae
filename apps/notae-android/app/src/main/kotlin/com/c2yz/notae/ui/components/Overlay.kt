package com.c2yz.notae.ui.components

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.navigationevent.NavigationEventTransitionState
import androidx.navigationevent.compose.NavigationEventState

/** The visibility of the overlay being drawn, which [ModalSurface] animates its scrim and card by; none outside one. */
internal val LocalOverlayScope = staticCompositionLocalOf<AnimatedVisibilityScope?> { null }

/** How far the back gesture has gone, 0 to 1, for the dialog or sheet it would close; read while drawing. */
val LocalBackProgress = compositionLocalOf<() -> Float> { { 0f } }

/** The back gesture's progress on [state], 0 when no gesture is under way. */
fun backProgress(state: NavigationEventState<*>): () -> Float = {
    (state.transitionState as? NavigationEventTransitionState.InProgress)?.latestEvent?.progress ?: 0f
}

/**
 * Draws [content] for [value] while it is set, animating in when it arrives and out when it goes. While it goes, the
 * last value stays on screen, but it is gone for TalkBack at once and takes no touches, so nothing acts on it twice.
 * An overlay already open when this first composes shows without animating.
 */
@Composable
fun <T : Any> AnimatedOverlay(value: T?, content: @Composable (T) -> Unit) {
    val last = remember { Last<T>() }
    if (value != null) last.value = value
    val shown = last.value ?: return
    val closing = value == null
    AnimatedVisibility(visible = !closing, enter = EnterTransition.None, exit = ExitTransition.None) {
        CompositionLocalProvider(LocalOverlayScope provides this) {
            Box(if (closing) Modifier.clearAndSetSemantics { }.pointerInput(Unit) { swallowTouches() } else Modifier) {
                content(shown)
            }
        }
    }
}

/** The value last shown; not state, since a new value always comes with a recomposition. */
private class Last<T : Any> {
    var value: T? = null
}

private suspend fun androidx.compose.ui.input.pointer.PointerInputScope.swallowTouches() {
    awaitPointerEventScope {
        while (true) awaitPointerEvent(PointerEventPass.Initial).changes.forEach { it.consume() }
    }
}
