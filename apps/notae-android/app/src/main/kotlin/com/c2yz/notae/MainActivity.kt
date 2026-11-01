package com.c2yz.notae

import android.os.Bundle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.appcompat.app.AppCompatActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import com.c2yz.notae.core.EffectHandler
import com.c2yz.notae.core.NotaeViewModel
import com.c2yz.notae.nav.NotaeNavDisplay
import com.c2yz.notae.ui.theme.NotaeTheme

/** The app's only activity: draws the core's snapshots and carries out its effects. */
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
    }

    /**
     * Nothing left for the core to close: the app goes to the background (R19). Finishing would clear the view
     * model, and a relaunch would build a second core over the same index while the first one shuts down.
     */
    private fun back() {
        if (!model.back()) moveTaskToBack(true)
    }
}
