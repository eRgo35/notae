package com.c2yz.notae.ui.theme

import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.tween

/**
 * The app's motion, on 0.3.0's durations: [short] for state changes (a colour, a fade), [medium] for things that move
 * (a screen, a sheet, a pane). Compose scales both by the system's animator duration scale.
 */
object NotaeMotion {
    /** Material's emphasized decelerate: quick out of the gate, a long settle, as One UI's moving surfaces. */
    val emphasized: Easing = CubicBezierEasing(0.2f, 0f, 0f, 1f)

    fun <T> short(): FiniteAnimationSpec<T> = tween(Tokens.motionShortMillis, easing = FastOutSlowInEasing)

    fun <T> medium(): FiniteAnimationSpec<T> = tween(Tokens.motionMediumMillis, easing = emphasized)
}
