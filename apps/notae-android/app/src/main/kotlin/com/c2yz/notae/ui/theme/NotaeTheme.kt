package com.c2yz.notae.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily

private val LocalTokenColors = staticCompositionLocalOf { Tokens.light }

/** 0.3.0's tokens for the current scheme; components read their colours here. */
object NotaeTheme {
    val colors: TokenColors
        @Composable @ReadOnlyComposable get() = LocalTokenColors.current
}

/** One UI look: 0.3.0's tokens, the default font, and no dynamic colour. */
@Composable
fun NotaeTheme(dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    val t = if (dark) Tokens.dark else Tokens.light
    val scheme = (if (dark) darkColorScheme() else lightColorScheme()).copy(
        primary = t.accent,
        onPrimary = t.onAccent,
        primaryContainer = t.accentSoft,
        onPrimaryContainer = t.onAccentSoft,
        background = t.background,
        onBackground = t.foreground,
        surface = t.card,
        onSurface = t.foreground,
        surfaceVariant = t.field,
        onSurfaceVariant = t.muted,
        outline = t.divider,
        error = t.error,
    )
    CompositionLocalProvider(LocalTokenColors provides t) {
        MaterialTheme(colorScheme = scheme) {
            // Replaced, not merged: Material's bodyLarge would add 0.5 sp tracking and a 24 sp line height 0.3.0 lacked.
            CompositionLocalProvider(LocalTextStyle provides TextStyle(fontFamily = FontFamily.Default, color = t.foreground)) {
                content()
            }
        }
    }
}
