package com.c2yz.notae.ui.setup

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.coerceAtLeast
import androidx.compose.ui.unit.coerceAtMost
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.SetupError
import com.c2yz.notae.core.SetupScreen
import com.c2yz.notae.ui.components.NotaeCard
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * Slint `WelcomeScreen`: first run (unconfigured), the recovery screen (permission lost) and a storage
 * error above the button. The intro scrolls above the fixed button on short windows. [onImport] is
 * called by "Import from a previous Notae", which shows only when [importAvailable].
 */
@Composable
fun WelcomeScreen(
    setup: SetupScreen,
    importAvailable: Boolean,
    onAction: (Action) -> Unit,
    onImport: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = NotaeTheme.colors
    val recovery = setup as? SetupScreen.PermissionLost
    BoxWithConstraints(modifier.fillMaxSize().background(c.background)) {
        val column = (maxWidth - 32.dp).coerceAtLeast(1.dp).coerceAtMost(560.dp)
        val narrow = maxWidth < 420.dp
        val top = ((maxHeight - 640.dp) / 6).coerceAtLeast(16.dp).coerceAtMost(64.dp)
        Column(
            Modifier.fillMaxSize().padding(bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            Column(
                Modifier.weight(1f, fill = false).fillMaxWidth().verticalScroll(rememberScrollState()).padding(top = top),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(20.dp),
            ) {
                Image(painterResource(R.drawable.notae), null, Modifier.size(96.dp), contentScale = ContentScale.Fit)
                Text(
                    stringResource(if (recovery != null) R.string.setup_reconnect_title else R.string.setup_welcome_title),
                    Modifier.padding(horizontal = 16.dp).semantics { heading() },
                    color = c.foreground,
                    fontSize = if (narrow) 32.sp else 40.sp,
                    fontWeight = FontWeight.SemiBold,
                    textAlign = TextAlign.Center,
                )
                Column(Modifier.widthIn(max = column).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(
                        if (recovery != null) stringResource(R.string.setup_reconnect_body, recovery.folder) else stringResource(R.string.setup_intro),
                        Modifier.fillMaxWidth(),
                        color = c.muted,
                        fontSize = 17.sp,
                        textAlign = TextAlign.Center,
                    )
                    NotaeCard(Modifier.fillMaxWidth()) {
                        Column(Modifier.padding(horizontal = 24.dp, vertical = 8.dp)) {
                            FeatureRow(R.drawable.ic_folder, R.string.setup_feature_files, R.string.setup_feature_files_detail)
                            Box(Modifier.fillMaxWidth().height(1.dp).background(c.divider))
                            FeatureRow(R.drawable.ic_search, R.string.setup_feature_search, R.string.setup_feature_search_detail)
                        }
                    }
                }
            }
            if (setup is SetupScreen.Error) {
                Text(
                    errorText(setup.error),
                    Modifier.padding(horizontal = 16.dp).fillMaxWidth(),
                    color = c.error,
                    fontSize = 15.sp,
                    textAlign = TextAlign.Center,
                )
            }
            Column(
                Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                PillButton(
                    stringResource(if (recovery != null) R.string.setup_reconnect_folder else R.string.setup_choose_folder),
                    { onAction(Action.PickLibraryRoot) },
                    Modifier.widthIn(max = 360.dp).fillMaxWidth(),
                    primary = true,
                )
                if (importAvailable) {
                    PillButton(stringResource(R.string.setup_import), onImport, Modifier.widthIn(max = 360.dp).fillMaxWidth(), tonal = true)
                }
                Text(stringResource(R.string.setup_pick_hint), color = c.muted, fontSize = 14.sp, textAlign = TextAlign.Center)
            }
        }
    }
}

@Composable
private fun errorText(error: SetupError): String = when (error) {
    SetupError.Cancelled -> stringResource(R.string.setup_error_cancelled)
    SetupError.NotADirectory -> stringResource(R.string.setup_error_not_a_directory)
    SetupError.PermissionLost -> stringResource(R.string.setup_error_permission_lost)
    SetupError.PermissionDenied -> stringResource(R.string.setup_error_permission_denied)
    is SetupError.Other -> stringResource(R.string.setup_error_other, error.detail)
}

@Composable
private fun FeatureRow(icon: Int, heading: Int, detail: Int) {
    val c = NotaeTheme.colors
    Row(Modifier.heightIn(min = 80.dp), horizontalArrangement = Arrangement.spacedBy(18.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(44.dp).clip(CircleShape).background(c.accentSoft), Alignment.Center) {
            Icon(painterResource(icon), null, Modifier.size(24.dp), tint = c.onAccentSoft)
        }
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(stringResource(heading), color = c.foreground, fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
            Text(stringResource(detail), color = c.muted, fontSize = 14.sp)
        }
    }
}
