package com.c2yz.notae.platform

import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ResolveInfo
import android.net.Uri
import android.os.Build
import androidx.core.net.toUri
import com.c2yz.notae.R
import com.c2yz.notae.core.PdfOpenError

/**
 * Opens a library PDF in the user's viewer, as 0.3.0 did (`v0.3.0:apps/notae-android/src/android/pdf_intent.rs`):
 * Samsung Notes when it handles the PDF, otherwise Android's chooser, with read-only access to the document.
 */
class PdfOpener(private val activity: Activity) {
    /**
     * The document at [relativePath] below the library tree [treeUri]. It walks the tree's folders,
     * so call it off the main thread; it throws what the provider throws.
     */
    fun find(treeUri: String, relativePath: String): Uri =
        documentUri(activity.contentResolver, treeUri.toUri(), relativePath)

    /** Opens [document] in a viewer, on the main thread; the error to report, or null once a viewer was launched. */
    fun open(document: Uri): PdfOpenError? = try {
        if (isContentUri(document)) launch(document) else PdfOpenError.InvalidContentUri
    } catch (error: Exception) {
        pdfOpenFailure(error)
    }

    private fun launch(document: Uri): PdfOpenError? {
        val view = pdfViewIntent(document)
        val samsungNotes = Intent(view).setPackage(SAMSUNG_NOTES_PACKAGE)
        if (samsungNotes.resolveActivity(activity.packageManager) != null) {
            activity.startActivity(samsungNotes)
            return null
        }
        if (viewers(view).isEmpty()) return PdfOpenError.NoCompatibleViewer
        val chooser = Intent.createChooser(view, activity.getString(R.string.open_pdf_with))
            .setFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        activity.startActivity(chooser)
        return null
    }

    private fun viewers(intent: Intent): List<ResolveInfo> {
        val packageManager = activity.packageManager
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            packageManager.queryIntentActivities(
                intent,
                PackageManager.ResolveInfoFlags.of(PackageManager.MATCH_DEFAULT_ONLY.toLong()),
            )
        } else {
            @Suppress("DEPRECATION")
            packageManager.queryIntentActivities(intent, PackageManager.MATCH_DEFAULT_ONLY)
        }
    }

    private companion object {
        const val SAMSUNG_NOTES_PACKAGE = "com.samsung.android.app.notes"
    }
}

/** A failure to find or open a PDF, with the platform's message. */
fun pdfOpenFailure(error: Throwable): PdfOpenError = PdfOpenError.Platform(error.message ?: error.toString())

/** `ACTION_VIEW` of [document] as `application/pdf`, granting the viewer read access only. */
fun pdfViewIntent(document: Uri): Intent =
    Intent(Intent.ACTION_VIEW)
        .setDataAndType(document, "application/pdf")
        .setFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)

/** Only a `content://` URI with an authority may leave the app (the core's `ContentUri`). */
private fun isContentUri(uri: Uri): Boolean {
    val text = uri.toString()
    return text.startsWith("content://") && !uri.authority.isNullOrEmpty() && text.none(Char::isWhitespace)
}
