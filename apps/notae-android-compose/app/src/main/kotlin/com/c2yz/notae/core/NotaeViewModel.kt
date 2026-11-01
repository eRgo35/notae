package com.c2yz.notae.core

import android.app.Application
import android.util.Log
import androidx.lifecycle.AndroidViewModel
import com.c2yz.notae.platform.SafLibrary
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.update
import java.util.concurrent.atomic.AtomicReference
import kotlin.concurrent.thread

/**
 * Owns the Rust core, built over the app's files directory and the SAF library. The core pushes a
 * [Snapshot] after every change, from whichever thread made it; [snapshot] keeps the highest
 * revision, since deliveries from different threads can arrive out of order. Effects wait in
 * [effects] for the activity, so one asked for while the activity is being recreated is not lost.
 *
 * `dispatch`, `back` and `snapshot` never wait for platform work in the core, so the main thread
 * may call them.
 */
class NotaeViewModel(app: Application) : AndroidViewModel(app) {
    private val flow = AtomicReference<MutableStateFlow<Snapshot>?>(null)
    private val pendingEffects = Channel<Effect>(Channel.UNLIMITED)
    private val core = NotaeCore(app.filesDir.path, SafLibrary(app), Listener())

    /** The newest state. */
    val snapshot: StateFlow<Snapshot>

    /** Platform work the core asked for, in order, each delivered once. */
    val effects: Flow<Effect> = pendingEffects.receiveAsFlow()

    /** The export waiting for the save-file picker, kept here so it outlives the activity. */
    var pendingExport: String? = null

    init {
        // Snapshots pushed before the flow exists are older than this read; offering a second
        // read covers any pushed between the first and the flow's publication.
        val state = MutableStateFlow(core.snapshot())
        flow.set(state)
        offer(core.snapshot())
        snapshot = state.asStateFlow()
    }

    fun dispatch(action: Action) = core.dispatch(action)

    /** Closes the innermost thing open; false when nothing is, and the activity should finish. */
    fun back(): Boolean = core.back()

    fun libraryRootPicked(displayName: String, handle: String) = core.libraryRootPicked(displayName, handle)

    fun fileWritten(error: String?) = core.fileWritten(error)

    fun importFileRead(text: String) = core.importFileRead(text)

    fun pdfOpened(error: PdfOpenError?) = core.pdfOpened(error)

    override fun onCleared() {
        pendingEffects.close()
        // Dropping the core joins its thread, which may be waiting for a platform call.
        thread(name = "notae-core-close") { core.close() }
    }

    private fun offer(next: Snapshot) {
        flow.get()?.update { shown -> if (next.revision > shown.revision) next else shown }
    }

    /** Called on the core's threads. Nothing may escape into Rust, where it would panic. */
    private inner class Listener : CoreListener {
        override fun onSnapshot(snapshot: Snapshot) = contained("snapshot") { offer(snapshot) }

        override fun onEffect(effect: Effect) = contained("effect") { pendingEffects.trySend(effect) }

        private inline fun contained(what: String, block: () -> Unit) {
            try {
                block()
            } catch (error: Throwable) {
                Log.e(TAG, "dropped a $what from the core", error)
            }
        }
    }

    private companion object {
        const val TAG = "Notae"
    }
}
