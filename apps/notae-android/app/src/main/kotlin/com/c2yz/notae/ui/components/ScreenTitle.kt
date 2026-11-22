package com.c2yz.notae.ui.components

import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.sp

/**
 * The title size of Settings, Licenses and Details: 24 sp in a short window (a phone on its side), 26 sp when [narrow],
 * otherwise 32 sp. Callers keep the title to one line with an ellipsis (Details allows two).
 */
@Composable
fun screenTitleSize(narrow: Boolean): TextUnit = when {
    windowHeightClass() == HeightClass.COMPACT -> 24.sp
    narrow -> 26.sp
    else -> 32.sp
}
