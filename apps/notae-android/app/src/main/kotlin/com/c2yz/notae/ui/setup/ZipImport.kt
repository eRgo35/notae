package com.c2yz.notae.ui.setup

import android.net.Uri
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.c2yz.notae.R
import com.c2yz.notae.platform.ZipImport
import com.c2yz.notae.platform.ZipImportException
import com.c2yz.notae.ui.components.DialogButtons
import com.c2yz.notae.ui.components.NotaeDialog
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.components.TextField
import com.c2yz.notae.ui.theme.NotaeTheme

/**
 * The ZIP import as the screens see it: its [state], owned by the view model, and what they can ask for. Notae has no
 * network access, so a link opens in the browser, which downloads the ZIP.
 */
class ZipImports(
    val state: ZipImport?,
    val choose: () -> Unit,
    val openLink: (url: String) -> Unit,
    val confirm: () -> Unit,
    val dismiss: () -> Unit,
) {
    companion object {
        /** No ZIP import: screenshots and screens shown on their own. */
        val None = ZipImports(null, {}, {}, {}, {})
    }
}

/** [text] as a web address to open, with `https://` added when it names no scheme; null unless it is http(s) with a host. */
fun browserAddress(text: String): String? {
    val trimmed = text.trim()
    if (trimmed.isEmpty()) return null
    val address = if ("://" in trimmed || trimmed.startsWith("javascript:", ignoreCase = true)) trimmed else "https://$trimmed"
    val uri = Uri.parse(address)
    val web = uri.scheme.equals("https", ignoreCase = true) || uri.scheme.equals("http", ignoreCase = true)
    return address.takeIf { web && !uri.host.isNullOrEmpty() }
}

/** "Import scores from a ZIP": a link to open in the browser, with what to tap there, or a ZIP file to choose. */
@Composable
fun ZipImportDialog(zip: ZipImports, onClose: () -> Unit) {
    val c = NotaeTheme.colors
    var link by remember { mutableStateOf("") }
    val address = browserAddress(link)
    val title = stringResource(R.string.zip_import)
    NotaeDialog(title, onDismiss = onClose) {
        Text(title, color = c.foreground, fontSize = 24.sp, fontWeight = FontWeight.SemiBold)
        Text(stringResource(R.string.zip_from_link), color = c.accentText, fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
        TextField(link, { link = it }, stringResource(R.string.zip_link_hint))
        PillButton(stringResource(R.string.zip_open_browser), { address?.let(zip.openLink) }, Modifier.fillMaxWidth(), tonal = true, enabled = address != null)
        Text(stringResource(R.string.zip_link_steps), color = c.muted, fontSize = 14.sp)
        DialogButtons(
            stringResource(R.string.set_cancel),
            onClose,
            stringResource(R.string.zip_choose),
            {
                onClose()
                zip.choose()
            },
        )
    }
}

/** Over every screen while a ZIP import needs the user: the replace question, the unpacking, or why it failed. */
@Composable
fun ZipImportStatus(zip: ZipImports) {
    val c = NotaeTheme.colors
    when (val state = zip.state) {
        null -> Unit
        is ZipImport.Confirm -> {
            val question = stringResource(R.string.zip_replace_question, state.name)
            NotaeDialog(question, onDismiss = zip.dismiss) {
                Text(question, color = c.foreground, fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
                Text(stringResource(R.string.zip_replace_detail), color = c.muted, fontSize = 15.sp)
                DialogButtons(stringResource(R.string.set_cancel), zip.dismiss, stringResource(R.string.zip_replace), zip.confirm)
            }
        }
        is ZipImport.Unpacking -> {
            val text = stringResource(R.string.zip_unpacking, state.name)
            NotaeDialog(text) {
                Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
                    CircularProgressIndicator(Modifier.size(28.dp), color = c.accent)
                    Text(text, color = c.foreground, fontSize = 17.sp)
                }
            }
        }
        is ZipImport.Failed -> {
            val text = when (state.reason) {
                ZipImportException.Reason.NOT_ZIP -> stringResource(R.string.zip_error_not_zip)
                ZipImportException.Reason.NO_PDFS -> stringResource(R.string.zip_error_no_pdfs)
                ZipImportException.Reason.NO_SPACE -> stringResource(R.string.zip_error_no_space)
                ZipImportException.Reason.UNREADABLE -> stringResource(R.string.zip_error_unreadable, state.detail.orEmpty())
            }
            NotaeDialog(text, onDismiss = zip.dismiss) {
                Text(text, color = c.foreground, fontSize = 18.sp)
                DialogButtons(
                    stringResource(R.string.dismiss),
                    zip.dismiss,
                    stringResource(R.string.zip_choose_another),
                    {
                        zip.dismiss()
                        zip.choose()
                    },
                )
            }
        }
    }
}
