package com.c2yz.notae.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import com.c2yz.notae.ui.theme.TokenColors
import com.c2yz.notae.ui.theme.Tokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/** The Compose [Tokens] are a copy of the Slint `Theme`; this reads the Slint source and compares. */
class TokensMatchSlintTest {
    private val colours: Map<String, (TokenColors) -> Color> = mapOf(
        "background" to { it.background },
        "card" to { it.card },
        "field" to { it.field },
        "foreground" to { it.foreground },
        "muted" to { it.muted },
        "divider" to { it.divider },
        "accent" to { it.accent },
        "accentText" to { it.accentText },
        "onAccent" to { it.onAccent },
        "accentSoft" to { it.accentSoft },
        "onAccentSoft" to { it.onAccentSoft },
        "error" to { it.error },
        "paper" to { it.paper },
        "paperEdge" to { it.paperEdge },
    )

    private fun slintSource(): String {
        var dir: File? = File(System.getProperty("user.dir")).absoluteFile
        while (dir != null) {
            val file = File(dir, "apps/notae-android/ui/components.slint")
            if (file.isFile) return file.readText()
            dir = dir.parentFile
        }
        error("components.slint not found above ${System.getProperty("user.dir")}")
    }

    private fun camel(kebab: String) =
        kebab.split('-').mapIndexed { i, w -> if (i == 0) w else w.replaceFirstChar { it.uppercase() } }.joinToString("")

    private fun argb(hex: String) = (0xFF000000L or hex.removePrefix("#").toLong(16)).toInt()

    @Test
    fun tokens_match_slint_theme() {
        val source = slintSource()
        val theme = source.substringAfter("export global Theme {").substringBefore("\n}")

        val parsed = Regex("""out property <color> ([a-z-]+): root\.dark \? (#[0-9A-Fa-f]{6}) : (#[0-9A-Fa-f]{6});""")
            .findAll(theme).associate { camel(it.groupValues[1]) to (it.groupValues[2] to it.groupValues[3]) }
        assertEquals("every Slint Theme colour has a Tokens twin", colours.keys, parsed.keys)
        for ((name, pair) in parsed) {
            val (dark, light) = pair
            assertEquals("$name light", argb(light), colours.getValue(name)(Tokens.light).toArgb())
            assertEquals("$name dark", argb(dark), colours.getValue(name)(Tokens.dark).toArgb())
        }

        // Durations: `root.animations-enabled ? Nms : 0ms`.
        val durations = Regex("""out property <duration> ([a-z-]+): root\.animations-enabled \? (\d+)ms : 0ms;""")
            .findAll(theme).associate { camel(it.groupValues[1]) to it.groupValues[2].toInt() }
        assertEquals(mapOf("motionShort" to Tokens.motionShortMillis, "motionMedium" to Tokens.motionMediumMillis), durations)

        // The `Motion` global's one length.
        val motion = source.substringAfter("export global Motion {").substringBefore("\n}")
        val shift = Regex("""out property <length> shift: (\d+)px;""").find(motion)?.groupValues?.get(1)?.toFloat()
        assertTrue("Motion.shift present", shift != null)
        assertEquals(shift, Tokens.motionShift.value)
    }
}
