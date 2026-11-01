package com.c2yz.notae.ui

import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsNodeInteractionsProvider
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp

/** Every node with a click action has a non-empty content description or text (merged semantics). */
fun SemanticsNodeInteractionsProvider.assertAllClickablesLabelled() {
    val nodes = onAllNodes(hasClickAction()).fetchSemanticsNodes()
    val unlabelled = nodes.filter { node ->
        val description = node.config.getOrNull(SemanticsProperties.ContentDescription).orEmpty()
        val text = node.config.getOrNull(SemanticsProperties.Text).orEmpty()
        description.none { it.isNotBlank() } && text.none { it.text.isNotBlank() }
    }
    check(unlabelled.isEmpty()) { "Clickable nodes without a label: ${unlabelled.map { it.id to it.boundsInRoot }}" }
}

/** Every node with a click action is at least 48 dp square. */
fun SemanticsNodeInteractionsProvider.assertClickTargetsAtLeast48dp(density: Density) {
    val min = with(density) { 48.dp.toPx() }
    val small = onAllNodes(hasClickAction()).fetchSemanticsNodes().filter {
        it.boundsInRoot.width < min - 0.5f || it.boundsInRoot.height < min - 0.5f
    }
    check(small.isEmpty()) { "Click targets under 48 dp: ${small.map { it.id to it.boundsInRoot }}" }
}
