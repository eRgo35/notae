package com.c2yz.notae

import com.c2yz.notae.core.DocumentEntry
import com.c2yz.notae.core.PlatformLibrary
import com.c2yz.notae.core.RecordingListener
import com.c2yz.notae.core.Screen
import com.c2yz.notae.core.SetupScreen
import com.c2yz.notae.core.testCore
import com.c2yz.notae.ui.library.Fixture
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.GraphicsMode

/**
 * Loads the host build of the Rust core through JNA and checks a fresh core's first screen. Like every test that
 * creates a core, it runs in the one sandbox (NATIVE graphics, default SDK; core/OneSandbox.kt explains).
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class CoreSmokeTest {
    @get:Rule
    val dataDir = TemporaryFolder()

    @Test
    fun aFreshCoreAsksForALibraryFolder() {
        val listener = RecordingListener()
        val core = testCore(dataDir.root.path, EmptyPlatform(), listener)
        try {
            assertEquals(Screen.Setup(SetupScreen.Unconfigured), core.snapshot().screen)
        } finally {
            core.close()
        }
    }

    @Test
    fun fresh_core_snapshot_matches_fixture_base() {
        val core = testCore(dataDir.root.path, EmptyPlatform(), RecordingListener())
        try {
            assertEquals(Fixture.base, core.snapshot())
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
