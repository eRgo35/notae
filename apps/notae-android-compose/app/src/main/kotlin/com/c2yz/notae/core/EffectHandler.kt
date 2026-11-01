package com.c2yz.notae.core

import android.net.Uri
import android.util.Log
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import com.c2yz.notae.platform.FolderPicker
import com.c2yz.notae.platform.PdfOpener
import com.c2yz.notae.platform.VoiceSearch
import com.c2yz.notae.platform.pdfOpenFailure
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.FileNotFoundException

/**
 * Carries out the core's [Effect]s for [activity] and reports each outcome back through [model].
 * Pickers and the viewer start on the main thread; file and folder walks run on [Dispatchers.IO].
 * Create it in `onCreate`: its activity-result launchers must be registered before the activity
 * is started. It handles effects while the activity is started; the rest wait in the view model.
 */
class EffectHandler(private val activity: AppCompatActivity, private val model: NotaeViewModel) {
    private val folderPicker = FolderPicker(
        activity,
        onPicked = model::libraryRootPicked,
        onFailed = { detail ->
            Log.w(TAG, "folder picker failed: $detail")
            model.libraryRootPickFailed(detail)
        },
    )
    private val voiceSearch = VoiceSearch(activity) { phrase -> model.dispatch(Action.SearchChanged(phrase)) }
    private val pdfOpener = PdfOpener(activity)
    private val exportFile = activity.registerForActivityResult(
        ActivityResultContracts.CreateDocument(JSON),
    ) { uri -> uri?.let(::writeExport) }
    private val importFile = activity.registerForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri -> uri?.let(::readImport) }

    init {
        activity.lifecycleScope.launch {
            activity.repeatOnLifecycle(Lifecycle.State.STARTED) {
                model.effects.collect(::handle)
            }
        }
    }

    private fun handle(effect: Effect) {
        when (effect) {
            is Effect.PickLibraryRoot -> folderPicker.launch(effect.initialUri)
            is Effect.OpenPdf -> openPdf(effect.uri, effect.relativePath)
            Effect.StartVoiceSearch -> if (!voiceSearch.start()) Log.w(TAG, "no speech recognizer could start")
            is Effect.WriteExport -> {
                model.pendingExport = effect.json
                exportFile.launch(effect.suggestedName)
            }
            Effect.PickImportFile -> importFile.launch(arrayOf(JSON, "text/plain", "application/octet-stream"))
        }
    }

    private fun openPdf(treeUri: String, relativePath: String) {
        activity.lifecycleScope.launch {
            val error = try {
                val document = withContext(Dispatchers.IO) { pdfOpener.find(treeUri, relativePath) }
                pdfOpener.open(document)
            } catch (error: CancellationException) {
                throw error
            } catch (error: Exception) {
                pdfOpenFailure(error)
            }
            model.pdfOpened(error)
        }
    }

    private fun writeExport(uri: Uri) {
        val json = model.pendingExport
        model.pendingExport = null
        activity.lifecycleScope.launch(Dispatchers.IO) {
            val error = try {
                checkNotNull(json) { "the export was lost while the file picker was open" }
                val stream = activity.contentResolver.openOutputStream(uri, "wt")
                    ?: throw FileNotFoundException("cannot write $uri")
                stream.use { it.write(json.toByteArray()) }
                null
            } catch (error: CancellationException) {
                throw error
            } catch (error: Exception) {
                error.message ?: error.toString()
            }
            model.fileWritten(error)
        }
    }

    private fun readImport(uri: Uri) {
        activity.lifecycleScope.launch(Dispatchers.IO) {
            try {
                val stream = activity.contentResolver.openInputStream(uri) ?: throw FileNotFoundException("cannot read $uri")
                model.importFileRead(stream.use { it.readBytes().decodeToString() })
            } catch (error: CancellationException) {
                throw error
            } catch (error: Exception) {
                // The core has no report for an unreadable import yet (it arrives with the import flow).
                Log.w(TAG, "cannot read the import file", error)
            }
        }
    }

    private companion object {
        const val TAG = "Notae"
        const val JSON = "application/json"
    }
}
