package com.c2yz.notae.platform

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Intent
import android.speech.RecognizerIntent
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts

/**
 * Voice search through the system speech recognizer's activity, which records the audio itself
 * (no `RECORD_AUDIO`). The first nonblank phrase of a successful result goes to [onPhrase], as in
 * the Slint app (`apps/notae-android/src/voice_search.rs`). Create it before [activity] is started.
 */
class VoiceSearch(private val activity: ComponentActivity, onPhrase: (String) -> Unit) {
    private val launcher = activity.registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        if (result.resultCode != Activity.RESULT_OK) return@registerForActivityResult
        result.data?.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS)
            ?.firstOrNull { it.isNotBlank() }
            ?.let(onPhrase)
    }

    /** Whether a speech recognizer is installed. */
    fun isAvailable(): Boolean = recognizerIntent().resolveActivity(activity.packageManager) != null

    /** Starts listening in the app's language; false when no recognizer could start. */
    fun start(): Boolean = try {
        launcher.launch(recognizerIntent())
        true
    } catch (_: ActivityNotFoundException) {
        false
    }

    private fun recognizerIntent() = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH)
        .putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
        .putExtra(RecognizerIntent.EXTRA_LANGUAGE, activity.resources.configuration.locales[0].toLanguageTag())
}
