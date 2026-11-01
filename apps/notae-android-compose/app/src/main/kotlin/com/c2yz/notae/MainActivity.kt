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
        EffectHandler(this, model)
        setContent {
            val snapshot by model.snapshot.collectAsState()
            NotaeTheme {
                NotaeNavDisplay(snapshot, onBack = ::back)
            }
        }
    }

    /** Nothing left for the core to close: leave, as Android would (R19). */
    private fun back() {
        if (!model.back()) finish()
    }
}
