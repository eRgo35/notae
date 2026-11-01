package com.c2yz.notae.core

/**
 * A [CoreListener] for tests that keeps every snapshot and effect. The core calls it from its own threads as well as
 * the caller's, so the lists are guarded. Like `NotaeViewModel`'s listener it never lets an exception reach Rust (one
 * thrown from `onSnapshot` would end `dispatch` before its effects are sent); it keeps the first one for [rethrow].
 */
class RecordingListener : CoreListener {
    private val lock = Any()
    private val snapshotLog = mutableListOf<Snapshot>()
    private val effectLog = mutableListOf<Effect>()
    private var failure: Throwable? = null

    val snapshots: List<Snapshot> get() = synchronized(lock) { snapshotLog.toList() }

    val effects: List<Effect> get() = synchronized(lock) { effectLog.toList() }

    override fun onSnapshot(snapshot: Snapshot) = contained { snapshotLog += snapshot }

    override fun onEffect(effect: Effect) = contained { effectLog += effect }

    /** Fails the test with the exception a callback caught, if there was one. */
    fun rethrow() {
        synchronized(lock) { failure }?.let { throw AssertionError("a core callback threw", it) }
    }

    private inline fun contained(block: () -> Unit) {
        try {
            synchronized(lock) { block() }
        } catch (error: Throwable) {
            synchronized(lock) { if (failure == null) failure = error }
        }
    }
}
