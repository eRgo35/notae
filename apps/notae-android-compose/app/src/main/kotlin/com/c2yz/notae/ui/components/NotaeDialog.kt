package com.c2yz.notae.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.c2yz.notae.R
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * Slint `SetModal` (dialog form): a card centred over a 40% black scrim, at most 560 wide with 16 dp side margins,
 * 26 radius, 28 padding and 20 spacing. The scrim swallows input; it dismisses only if [onDismiss] is given.
 */
@Composable
fun NotaeDialog(
    dialogLabel: String,
    modifier: Modifier = Modifier,
    onDismiss: (() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) = ModalSurface(dialogLabel, onDismiss, bottomSheet = false, cardWidth = 560.dp, modifier = modifier, content = content)

/** The scrim and card shared by [NotaeDialog] and [ActionSheet]. */
@Composable
internal fun ModalSurface(
    dialogLabel: String,
    onDismiss: (() -> Unit)?,
    bottomSheet: Boolean,
    cardWidth: Dp,
    modifier: Modifier = Modifier,
    padding: PaddingValues = PaddingValues(28.dp),
    spacing: Dp = 20.dp,
    content: @Composable ColumnScope.() -> Unit,
) {
    val dismissLabel = stringResource(R.string.dismiss)
    Box(modifier.fillMaxSize()) {
        Box(
            Modifier
                .fillMaxSize()
                .background(Color.Black.copy(alpha = 0.4f))
                .then(
                    if (onDismiss != null) {
                        Modifier
                            .semantics { contentDescription = dismissLabel }
                            .clickable(onClickLabel = dismissLabel, role = Role.Button, onClick = onDismiss)
                    } else {
                        Modifier.pointerInput(Unit) { detectTapGestures { } }
                    },
                ),
        )
        val shape = if (bottomSheet) RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp) else RoundedCornerShape(26.dp)
        Column(
            Modifier
                .align(if (bottomSheet) Alignment.BottomCenter else Alignment.Center)
                .then(if (bottomSheet) Modifier else Modifier.padding(horizontal = 16.dp))
                .widthIn(max = cardWidth)
                .clip(shape)
                .background(NotaeTheme.colors.card)
                .pointerInput(Unit) { detectTapGestures { } }
                .semantics { paneTitle = dialogLabel }
                .padding(padding),
            verticalArrangement = Arrangement.spacedBy(spacing),
            content = content,
        )
    }
}
