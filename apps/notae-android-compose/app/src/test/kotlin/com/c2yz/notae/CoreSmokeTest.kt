package com.c2yz.notae

import com.c2yz.notae.core.CoreListener
import com.c2yz.notae.core.DocumentEntry
import com.c2yz.notae.core.Effect
import com.c2yz.notae.core.NotaeCore
import com.c2yz.notae.core.PlatformLibrary
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.SetupScreen
import com.c2yz.notae.core.Snapshot
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.util.Collections

/** Loads the host build of the Rust core through JNA and checks a fresh core's first screen. */
@RunWith(RobolectricTestRunner::class)
class CoreSmokeTest {
    @get:Rule
    val dataDir = TemporaryFolder()

    @Test
    fun aFreshCoreAsksForALibraryFolder() {
        val listener = RecordingListener()
        val core = NotaeCore(dataDir.root.path, EmptyPlatform(), listener)
        try {
            assertEquals(Screen.Setup(SetupScreen.Unconfigured), core.snapshot().screen)
        } finally {
            core.close()
        }
    }
}

/** A platform with no library folder: any call fails the test. */
private class EmptyPlatform : PlatformLibrary {
    override fun checkRoot(handle: String) = error("no library folder was picked")

    override fun enumerate(handle: String): List<DocumentEntry> = error("no library folder was picked")

    override fun extractTextLayer(handle: String, path: String): String? = error("no library folder was picked")

    override fun renderThumbnail(handle: String, path: String, outPng: String): Boolean =
        error("no library folder was picked")
}

private class RecordingListener : CoreListener {
    val snapshots: MutableList<Snapshot> = Collections.synchronizedList(mutableListOf())
    val effects: MutableList<Effect> = Collections.synchronizedList(mutableListOf())

    override fun onSnapshot(snapshot: Snapshot) {
        snapshots += snapshot
    }

    override fun onEffect(effect: Effect) {
        effects += effect
    }
}
