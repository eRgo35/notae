package com.c2yz.notae.ui.setup

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
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
import com.c2yz.notae.core.ImportStatus
import com.c2yz.notae.core.SetupError
import com.c2yz.notae.core.SetupScreen
import com.c2yz.notae.ui.components.HeightClass
import com.c2yz.notae.ui.components.NotaeCard
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.components.windowHeightClass
import com.c2yz.notae.ui.settings.importStatusText
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * 0.3.0's `WelcomeScreen`: first run (unconfigured), the recovery screen (permission lost) and a storage
 * error above the button. The intro scrolls above the fixed button on short windows. "Import from a previous
 * Notae" shows only when [importAvailable] and runs the same import as Settings; [importStatus] is its outcome.
 */
@Composable
fun WelcomeScreen(
    setup: SetupScreen,
    importAvailable: Boolean,
    onAction: (Action) -> Unit,
    modifier: Modifier = Modifier,
    importStatus: ImportStatus? = null,
    zip: ZipImports = ZipImports.None,
) {
    var zipDialog by remember { mutableStateOf(false) }
    val c = NotaeTheme.colors
    val recovery = setup as? SetupScreen.PermissionLost
    BoxWithConstraints(modifier.fillMaxSize().background(c.background)) {
        val column = (maxWidth - 32.dp).coerceAtLeast(1.dp).coerceAtMost(560.dp)
        val narrow = maxWidth < 420.dp
        val short = windowHeightClass() == HeightClass.COMPACT
        val top = ((maxHeight - 640.dp) / 6).coerceAtLeast(16.dp).coerceAtMost(64.dp)
        val title: @Composable () -> Unit = {
            Text(
                stringResource(if (recovery != null) R.string.setup_reconnect_title else R.string.setup_welcome_title),
                Modifier.padding(horizontal = 16.dp).semantics { heading() },
                color = c.foreground,
                fontSize = if (narrow || short) 32.sp else 40.sp,
                fontWeight = FontWeight.SemiBold,
                textAlign = TextAlign.Center,
            )
        }
        val body: @Composable () -> Unit = {
            Text(
                if (recovery != null) stringResource(R.string.setup_reconnect_body, recovery.folder) else stringResource(R.string.setup_intro),
                Modifier.fillMaxWidth(),
                color = c.muted,
                fontSize = 17.sp,
                textAlign = TextAlign.Center,
            )
        }
        val features: @Composable () -> Unit = {
            NotaeCard(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(horizontal = 24.dp, vertical = 8.dp)) {
                    FeatureRow(R.drawable.ic_folder, R.string.setup_feature_files, R.string.setup_feature_files_detail)
                    Box(Modifier.fillMaxWidth().height(1.dp).background(c.divider))
                    FeatureRow(R.drawable.ic_search, R.string.setup_feature_search, R.string.setup_feature_search_detail)
                }
            }
        }
        val actions: @Composable () -> Unit = {
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
                // A recovering library has its folder to reconnect; a ZIP would replace it.
                if (recovery == null) {
                    PillButton(
                        stringResource(R.string.zip_import),
                        { zipDialog = true },
                        Modifier.widthIn(max = 360.dp).fillMaxWidth(),
                        tonal = true,
                    )
                }
                if (importAvailable) {
                    PillButton(
                        stringResource(R.string.setup_import),
                        { onAction(Action.ImportRequested) },
                        Modifier.widthIn(max = 360.dp).fillMaxWidth(),
                        tonal = true,
                    )
                }
                importStatus?.let { Text(importStatusText(it), color = c.muted, fontSize = 14.sp, textAlign = TextAlign.Center) }
                Text(stringResource(R.string.setup_pick_hint), color = c.muted, fontSize = 14.sp, textAlign = TextAlign.Center)
            }
        }
        if (short) {
            // A short window (a phone on its side): the intro on the start half, the features and the buttons on the
            // end half, each scrolling on its own, so the text and the main button are both in view.
            Row(Modifier.fillMaxSize().padding(vertical = 16.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                Column(
                    Modifier.weight(1f).fillMaxHeight().verticalScroll(rememberScrollState()),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterVertically),
                ) {
                    Image(painterResource(R.drawable.notae), null, Modifier.size(56.dp), contentScale = ContentScale.Fit)
                    title()
                    Box(Modifier.padding(horizontal = 16.dp)) { body() }
                }
                Column(
                    Modifier.weight(1f).fillMaxHeight(),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterVertically),
                ) {
                    Box(Modifier.weight(1f, fill = false).verticalScroll(rememberScrollState()).padding(end = 16.dp)) { features() }
                    actions()
                }
            }
        } else {
            // Centred in the window: on a tall screen the welcome no longer leaves its bottom half empty.
            Column(
                Modifier.fillMaxSize().padding(bottom = 24.dp),
                verticalArrangement = Arrangement.spacedBy(20.dp, Alignment.CenterVertically),
            ) {
                Column(
                    Modifier.weight(1f, fill = false).fillMaxWidth().verticalScroll(rememberScrollState()).padding(top = top),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(20.dp),
                ) {
                    Image(painterResource(R.drawable.notae), null, Modifier.size(96.dp), contentScale = ContentScale.Fit)
                    title()
                    Column(Modifier.widthIn(max = column).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        body()
                        features()
                    }
                }
                actions()
            }
        }
        if (zipDialog) ZipImportDialog(zip) { zipDialog = false }
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
