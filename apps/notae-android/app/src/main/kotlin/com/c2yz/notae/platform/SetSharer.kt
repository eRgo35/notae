package com.c2yz.notae.platform

import android.app.Activity
import android.content.Intent
import androidx.core.content.FileProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

/**
 * Sends a set through the share sheet: one `ACTION_SEND` with the set file as a [FileProvider] stream and the plain list
 * as its text. The file goes to `cacheDir/shared/`, the only folder the provider exposes, after the older files there.
 */
class SetSharer(private val activity: Activity) {
    /** Writes [json] to `<fileName>` on [Dispatchers.IO], then starts the chooser from the caller's (main) thread. */
    suspend fun share(fileName: String, json: String, text: String) {
        val file = withContext(Dispatchers.IO) {
            val folder = File(activity.cacheDir, SHARED)
            folder.deleteRecursively()
            folder.mkdirs()
            File(folder, fileName).apply { writeText(json) }
        }
        val uri = FileProvider.getUriForFile(activity, "${activity.packageName}.files", file)
        val send = Intent(Intent.ACTION_SEND)
            .setType(JSON)
            .putExtra(Intent.EXTRA_STREAM, uri)
            .putExtra(Intent.EXTRA_TEXT, text)
            .putExtra(Intent.EXTRA_TITLE, fileName.removeSuffix(SET_SUFFIX))
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        activity.startActivity(Intent.createChooser(send, null))
    }

    private companion object {
        const val SHARED = "shared"
        const val JSON = "application/json"
        const val SET_SUFFIX = ".notae-set"
    }
}
