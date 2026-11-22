package com.c2yz.notae.ui

import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.junit4.ComposeContentTestRule
import androidx.compose.ui.test.onRoot

/** The layout size matrix (dp): name → Robolectric qualifiers. */
object Windows {
    val MATRIX: Map<String, String> = listOf(
        360 to 640, 384 to 832, 412 to 915,
        640 to 360, 832 to 384, 915 to 412,
        600 to 960, 700 to 500, 840 to 600,
        800 to 1280, 1280 to 800, 1440 to 900,
    ).associate { (w, h) -> "w${w}dp-h${h}dp" to "+w${w}dp-h${h}dp-xhdpi" }
}

/** The node's bounds lie inside the window (the compose root), so it is visible without scrolling. */
fun SemanticsNodeInteraction.assertInsideWindow(compose: ComposeContentTestRule): SemanticsNodeInteraction {
    val root = compose.onRoot().fetchSemanticsNode().boundsInRoot
    val node = fetchSemanticsNode().boundsInRoot
    check(node.left >= root.left - 0.5f && node.top >= root.top - 0.5f && node.right <= root.right + 0.5f &&
        node.bottom <= root.bottom + 0.5f) { "Node $node is outside the window $root" }
    return this
}
