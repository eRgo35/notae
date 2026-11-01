package com.c2yz.notae.platform

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts

/**
 * The system folder picker for the library root. A picked folder's read grant is made persistent
 * (read only) and reported with its display name and tree URI, the core's handle; a cancelled
 * picker reports nothing. Create it before [activity] is started.
 */
class FolderPicker(
    private val activity: ComponentActivity,
    onPicked: (displayName: String, handle: String) -> Unit,
    /** The picker could not run or its folder could not be kept, as 0.3.0's `PickerOutcome::Platform`. */
    private val onFailed: (detail: String) -> Unit,
) {
    private val launcher = activity.registerForActivityResult(ActivityResultContracts.OpenDocumentTree()) { tree ->
        if (tree == null) return@registerForActivityResult
        val displayName = try {
            activity.contentResolver.takePersistableUriPermission(tree, Intent.FLAG_GRANT_READ_URI_PERMISSION)
            displayName(tree)
        } catch (error: Exception) {
            onFailed(error.message ?: error.toString())
            return@registerForActivityResult
        }
        onPicked(displayName, tree.toString())
    }

    /** Opens the picker, starting at [initialUri] (`EXTRA_INITIAL_URI`) when one is known. */
    fun launch(initialUri: String?) {
        try {
            launcher.launch(initialUri?.let(Uri::parse))
        } catch (error: ActivityNotFoundException) {
            onFailed(error.message ?: error.toString())
        }
    }

    private fun displayName(tree: Uri): String {
        val root = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
        val name = activity.contentResolver.query(root, arrayOf(Document.COLUMN_DISPLAY_NAME), null, null)?.use {
            if (it.moveToFirst()) it.getString(0) else null
        }
        check(!name.isNullOrEmpty()) { "the picked folder has no display name" }
        return name
    }
}
