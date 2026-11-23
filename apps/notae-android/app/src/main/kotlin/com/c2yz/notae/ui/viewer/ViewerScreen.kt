package com.c2yz.notae.ui.viewer

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Viewer
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * Notae's viewer: a bar with Back and the score's title over its pages. It is drawn over the screen that opened it;
 * Back (the button, the gesture or Escape) closes it through the core.
 */
@Composable
fun ViewerScreen(viewer: Viewer, onAction: (Action) -> Unit) {
    val c = NotaeTheme.colors
    Column(Modifier.fillMaxSize().background(c.background)) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(stringResource(R.string.viewer_back), painterResource(R.drawable.ic_arrow_back), { onAction(Action.CloseViewer) })
            Text(
                viewer.row.title,
                Modifier.weight(1f),
                color = c.foreground,
                fontSize = 18.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Box(Modifier.weight(1f).fillMaxWidth())
    }
}
