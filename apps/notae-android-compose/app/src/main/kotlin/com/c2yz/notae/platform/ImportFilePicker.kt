package com.c2yz.notae.platform

import android.content.ActivityNotFoundException
import android.net.Uri
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.io.ByteArrayOutputStream
import java.io.FileNotFoundException
import java.io.IOException
import java.io.InputStream

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
            val resolver = activity.contentResolver
            readImport(
                declaredLength = { resolver.openAssetFileDescriptor(uri, "r")?.use { it.length } ?: UNKNOWN_LENGTH },
                open = { resolver.openInputStream(uri) ?: throw FileNotFoundException("cannot read $uri") },
            ).fold(onRead) { error -> onFailed(error.message ?: error.toString()) }
        }
    }

    private companion object {
        const val JSON = "application/json"
    }
}

/** An export file is a few kilobytes; anything past this is not one, and reading it all could run Notae out of memory. */
internal const val MAX_IMPORT_BYTES = 16 * 1024 * 1024

/** What [android.content.res.AssetFileDescriptor.getLength] reports when the provider does not know the size. */
private const val UNKNOWN_LENGTH = -1L

/**
 * The text of the file [open] reads, or why it cannot be imported, without throwing: a file over [cap] bytes, by its
 * [declaredLength] when the provider knows it and by reading at most [cap] + 1 bytes either way, or any read error.
 */
internal fun readImport(declaredLength: () -> Long, open: () -> InputStream, cap: Int = MAX_IMPORT_BYTES): Result<String> =
    runCatching {
        val tooLarge = IOException("the file is larger than $cap bytes")
        if (declaredLength() > cap) throw tooLarge
        val bytes = ByteArrayOutputStream()
        open().use { stream ->
            val buffer = ByteArray(8192)
            while (true) {
                val read = stream.read(buffer, 0, minOf(buffer.size, cap + 1 - bytes.size()))
                if (read < 0) break
                bytes.write(buffer, 0, read)
                if (bytes.size() > cap) throw tooLarge
            }
        }
        bytes.toString(Charsets.UTF_8.name())
    }
