package com.c2yz.notae.ui.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

/** The colours of the Slint `Theme` for one scheme, under the same names in camelCase. */
data class TokenColors(
    val background: Color,
    val card: Color,
    val field: Color,
    val foreground: Color,
    val muted: Color,
    val divider: Color,
    val accent: Color,
    /** Accent for text on neutral backgrounds; plain `accent` is 4.1:1 on the light background. */
    val accentText: Color,
    val onAccent: Color,
    val accentSoft: Color,
    val onAccentSoft: Color,
    val error: Color,
    val paper: Color,
    val paperEdge: Color,
)

/** The Slint `Theme` and `Motion` tokens (apps/notae-android/ui/components.slint); `tokens_match_slint_theme` checks them. */
object Tokens {
    val light = TokenColors(
        background = Color(0xFFF3F3F5),
        card = Color(0xFFFFFFFF),
        field = Color(0xFFEBEBEF),
        foreground = Color(0xFF111114),
        muted = Color(0xFF6E6E73),
        divider = Color(0xFFE6E6EA),
        accent = Color(0xFF2F6FEB),
        accentText = Color(0xFF1A4FB8),
        onAccent = Color(0xFFFFFFFF),
        accentSoft = Color(0xFFE6EEFE),
        onAccentSoft = Color(0xFF1A4FB8),
        error = Color(0xFFC62828),
        paper = Color(0xFFFFFFFF),
        paperEdge = Color(0xFFE3E3E8),
    )

    val dark = TokenColors(
        background = Color(0xFF000000),
        card = Color(0xFF17171A),
        field = Color(0xFF26262B),
        foreground = Color(0xFFF2F2F4),
        muted = Color(0xFFA1A1A8),
        divider = Color(0xFF2A2A2E),
        accent = Color(0xFF5B9BFF),
        accentText = Color(0xFF5B9BFF),
        onAccent = Color(0xFF06142E),
        accentSoft = Color(0xFF1C2C4A),
        onAccentSoft = Color(0xFFB9D2FF),
        error = Color(0xFFFF8A80),
        paper = Color(0xFFE4E4E8),
        paperEdge = Color(0xFF2A2A2E),
    )

    /** Slint `Theme.motion-short` / `motion-medium` with animations enabled. */
    const val motionShortMillis = 120
    const val motionMediumMillis = 220

    /** Slint `Motion.shift`: how far a view slides while it enters or leaves. */
    val motionShift = 24.dp
}
