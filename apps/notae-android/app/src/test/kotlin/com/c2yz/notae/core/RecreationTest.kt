package com.c2yz.notae.core

import android.Manifest
import android.content.pm.ProviderInfo
import android.os.Looper
import android.provider.DocumentsContract
import androidx.lifecycle.ViewModelProvider
import androidx.test.core.app.ActivityScenario
import com.c2yz.notae.MainActivity
import com.c2yz.notae.platform.FakeDocument
import com.c2yz.notae.platform.FakeDocumentsProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The open set and the details survive the activity's recreation and the process's death, and Back on the bare
 * library keeps the activity.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp")
class RecreationTest {
    @get:Rule
    val files = TemporaryFolder()

    private val tree = DocumentsContract.buildTreeDocumentUri(AUTHORITY, "root").toString()

    @Before
    fun registerLibrary() {
        // MainActivity creates the core (OneSandbox.kt).
        assertOneSandbox()
        val info = ProviderInfo().apply {
            authority = AUTHORITY
            exported = true
            grantUriPermissions = true
            readPermission = Manifest.permission.MANAGE_DOCUMENTS
            writePermission = Manifest.permission.MANAGE_DOCUMENTS
        }
        val provider = Robolectric.buildContentProvider(FakeDocumentsProvider::class.java).create(info).get()
        for (title in listOf("Ave Maria", "Barka")) {
            val pdf = files.newFile().apply { writeBytes("%PDF-1.4\n$title".toByteArray()) }
            provider.add("root", FakeDocument("$title.pdf", pdf))
        }
    }

    @Test
    fun process_recreation_restores_open_set() {
        var scenario = ActivityScenario.launch(MainActivity::class.java)
        val model = scenario.viewModel()
        model.libraryRootPicked("Scores", tree)
        model.await("the library") { it.screen == Screen.Library }
        model.dispatch(Action.Rescan)
        val library = model.await("the scanned rows") { it.banner.message is StatusMessage.Finished }
        model.dispatch(Action.NewSet)
        model.dispatch(Action.SaveSetDialog("Mass", null, null))
        model.dispatch(Action.OpenAddPicker)
        model.dispatch(Action.ToggleAddPickerPiece(library.rows.first().noteKey))
        model.dispatch(Action.CloseAddPicker)
        val piece = model.snapshot.value.rows.first().selectionId
        model.dispatch(Action.SelectScore(piece))
        val open = model.snapshot.value
        assertEquals(piece, open.selection?.row?.selectionId)

        // A configuration change keeps the view model.
        scenario.recreate()
        assertEquals(open.sectionKey, scenario.viewModel().snapshot.value.sectionKey)
        assertEquals(piece, scenario.viewModel().snapshot.value.selection?.row?.selectionId)

        // Process death: a new view model over the same data directory.
        scenario.close()
        scenario = ActivityScenario.launch(MainActivity::class.java)
        val restarted = scenario.viewModel()
        assertEquals("the first snapshot is in the set", open.sectionKey, restarted.snapshot.value.sectionKey)
        val restored = restarted.await("the restored rows") { it.screen == Screen.Library && it.rows.isNotEmpty() }
        assertEquals(open.sectionKey, restored.sectionKey)
        assertEquals(piece, restored.selection?.row?.selectionId)
        scenario.close()
    }

    @Test
    fun back_on_the_bare_library_keeps_the_activity() {
        val scenario = ActivityScenario.launch(MainActivity::class.java)
        val model = scenario.viewModel()
        model.libraryRootPicked("Scores", tree)
        model.await("the library") { it.screen == Screen.Library }

        // The data directory outlives a test, so close whatever an earlier one left open.
        while (model.back()) Unit
        var finishing = true
        scenario.onActivity { it.onBackPressedDispatcher.onBackPressed() }
        shadowOf(Looper.getMainLooper()).idle()
        scenario.onActivity { finishing = it.isFinishing }

        // Finishing would clear the view model and drop the core while a scan may still run.
        assertFalse("Back sends the app to the background instead of finishing", finishing)
        scenario.close()
    }

    private fun ActivityScenario<MainActivity>.viewModel(): NotaeViewModel {
        var model: NotaeViewModel? = null
        onActivity { model = ViewModelProvider(it)[NotaeViewModel::class.java] }
        return checkNotNull(model)
    }

    /** Waits for a snapshot satisfying [done]; the core pushes them from its own threads. */
    private fun NotaeViewModel.await(what: String, done: (Snapshot) -> Boolean): Snapshot {
        val deadline = System.nanoTime() + 30_000_000_000L
        while (true) {
            shadowOf(Looper.getMainLooper()).idle()
            val latest = snapshot.value
            if (done(latest)) return latest
            check(System.nanoTime() < deadline) { "timed out waiting for $what; last snapshot: $latest" }
            Thread.sleep(20)
        }
    }

    private companion object {
        const val AUTHORITY = "com.c2yz.notae.test.recreation"
    }
}
