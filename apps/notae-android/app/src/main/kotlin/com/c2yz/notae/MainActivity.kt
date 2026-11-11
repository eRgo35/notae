package com.c2yz.notae

import android.content.Intent
import android.os.Bundle
import android.util.Log
import android.widget.Toast
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.appcompat.app.AppCompatActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.lifecycle.lifecycleScope
import com.c2yz.notae.core.EffectHandler
import com.c2yz.notae.core.ImportStatus
import com.c2yz.notae.core.NotaeViewModel
import com.c2yz.notae.nav.NotaeNavDisplay
import com.c2yz.notae.platform.importUri
import com.c2yz.notae.platform.readImport
import com.c2yz.notae.ui.settings.importStatusText
import com.c2yz.notae.ui.theme.NotaeTheme
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * The app's only activity: draws the core's snapshots and carries out its effects, and reads a file opened with or shared
 * to Notae (a shared set, or a backup) into the core, which checks what it is.
 */
class MainActivity : AppCompatActivity() {
    private val model: NotaeViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        val effects = EffectHandler(this, model)
        setContent {
            val snapshot by model.snapshot.collectAsState()
            NotaeTheme {
                NotaeNavDisplay(snapshot, onAction = model::dispatch, onBack = ::back, voiceAvailable = effects.voiceAvailable)
            }
        }
        // A recreated activity was handed the same intent again: its file was already read.
        if (savedInstanceState == null) receive(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        receive(intent)
    }

    /**
     * Reads the file [intent] carries, if any, on [Dispatchers.IO], as the Import sets picker does. The core reports the
     * outcome synchronously; a failure (unreadable, or not a Notae file) is also a toast, since Settings, which shows
     * it, may not be open.
     */
    private fun receive(intent: Intent) {
        val uri = importUri(intent) ?: return
        lifecycleScope.launch(Dispatchers.IO) {
            readImport(contentResolver, uri).fold(model::importFileRead) { error ->
                val detail = error.message ?: error.toString()
                Log.w(TAG, "received file read failed: $detail")
                model.importFileFailed(detail)
            }
            val failed = model.snapshot.value.settings.importStatus as? ImportStatus.Failed ?: return@launch
            withContext(Dispatchers.Main) {
                Toast.makeText(this@MainActivity, importStatusText(resources, failed), Toast.LENGTH_LONG).show()
            }
        }
    }

    /**
     * Nothing left for the core to close: the app goes to the background (R19). Finishing would clear the view
     * model, and a relaunch would build a second core over the same index while the first one shuts down.
     */
    private fun back() {
        if (!model.back()) moveTaskToBack(true)
    }

    private companion object {
        const val TAG = "Notae"
    }
}
