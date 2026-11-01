package com.c2yz.notae.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import com.c2yz.notae.ui.theme.TokenColors
import com.c2yz.notae.ui.theme.Tokens
import org.junit.Assert.assertEquals
import org.junit.Test

/** The [Tokens] keep 0.3.0's `Theme` and `Motion` values, written out here as that theme defined them. */
class TokensTest {
    /** Each colour's (light, dark) value in 0.3.0. */
    private val colours: Map<String, Triple<(TokenColors) -> Color, Long, Long>> = mapOf(
        "background" to Triple({ it.background }, 0xF3F3F5, 0x000000),
        "card" to Triple({ it.card }, 0xFFFFFF, 0x17171A),
        "field" to Triple({ it.field }, 0xEBEBEF, 0x26262B),
        "foreground" to Triple({ it.foreground }, 0x111114, 0xF2F2F4),
        "muted" to Triple({ it.muted }, 0x6E6E73, 0xA1A1A8),
        "divider" to Triple({ it.divider }, 0xE6E6EA, 0x2A2A2E),
        "accent" to Triple({ it.accent }, 0x2F6FEB, 0x5B9BFF),
        "accentText" to Triple({ it.accentText }, 0x1A4FB8, 0x5B9BFF),
        "onAccent" to Triple({ it.onAccent }, 0xFFFFFF, 0x06142E),
        "accentSoft" to Triple({ it.accentSoft }, 0xE6EEFE, 0x1C2C4A),
        "onAccentSoft" to Triple({ it.onAccentSoft }, 0x1A4FB8, 0xB9D2FF),
        "error" to Triple({ it.error }, 0xC62828, 0xFF8A80),
        "paper" to Triple({ it.paper }, 0xFFFFFF, 0xE4E4E8),
        "paperEdge" to Triple({ it.paperEdge }, 0xE3E3E8, 0x2A2A2E),
    )

    private fun argb(rgb: Long) = (0xFF000000L or rgb).toInt()

    @Test
    fun tokens_keep_the_0_3_0_theme() {
        for ((name, entry) in colours) {
            val (colour, light, dark) = entry
            assertEquals("$name light", argb(light), colour(Tokens.light).toArgb())
            assertEquals("$name dark", argb(dark), colour(Tokens.dark).toArgb())
        }
        assertEquals(120, Tokens.motionShortMillis)
        assertEquals(220, Tokens.motionMediumMillis)
        assertEquals(24f, Tokens.motionShift.value)
    }
}
