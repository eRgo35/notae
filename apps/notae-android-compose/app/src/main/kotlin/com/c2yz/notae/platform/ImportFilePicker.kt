package com.c2yz.notae.platform

import android.content.ActivityNotFoundException
import android.net.Uri
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.io.FileNotFoundException

/**
 * The system open-file picker for an export file ("Import sets"). The chosen file is read on [Dispatchers.IO] and its
 * text reported; a cancelled picker reports nothing. A picker that cannot run, or a file that cannot be read, reports
 * the platform's message, as [FolderPicker] does. Create it before [activity] is started.
 */
class ImportFilePicker(
    private val activity: ComponentActivity,
    private val onRead: (text: String) -> Unit,
    private val onFailed: (detail: String) -> Unit,
) {
    private val launcher = activity.registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri?.let(::read)
    }

    /** Opens the picker on JSON files; some providers label a `.json` file as plain text or bytes, so those show too. */
    fun launch() {
        try {
            launcher.launch(arrayOf(JSON, "text/plain", "application/octet-stream"))
        } catch (error: ActivityNotFoundException) {
            onFailed(error.message ?: error.toString())
        }
    }

    private fun read(uri: Uri) {
        activity.lifecycleScope.launch(Dispatchers.IO) {
            val text = try {
                val stream = activity.contentResolver.openInputStream(uri) ?: throw FileNotFoundException("cannot read $uri")
                stream.use { it.readBytes().decodeToString() }
            } catch (error: CancellationException) {
                throw error
            } catch (error: Exception) {
                onFailed(error.message ?: error.toString())
                return@launch
            }
            onRead(text)
        }
    }

    private companion object {
        const val JSON = "application/json"
    }
}
