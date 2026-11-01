package com.c2yz.notae.ui.details

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.unit.Density
import com.c2yz.notae.core.Action
import com.c2yz.notae.core.Failure
import com.c2yz.notae.core.MetadataDraft
import com.c2yz.notae.core.Snapshot
import com.c2yz.notae.nav.NotaeNavDisplay
import com.c2yz.notae.ui.assertAllClickablesLabelled
import com.c2yz.notae.ui.assertClickTargetsAtLeast48dp
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.theme.NotaeTheme
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** The metadata editor: fields kept in Compose until Save, the Action Save and Cancel send, and the core's error. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class MetadataEditorTest {
    @get:Rule
    val compose = createComposeRule()

    private val actions = mutableListOf<Action>()
    private var draft by mutableStateOf(aveMaria)

    private fun show() {
        var density = Density(1f)
        compose.setContent {
            density = LocalDensity.current
            NotaeTheme { MetadataEditor(draft, { action -> actions += action }) }
        }
        compose.assertAllClickablesLabelled()
        compose.assertClickTargetsAtLeast48dp(density)
    }

    @Test
    fun metadata_save_dispatches_fields() {
        show()

        compose.onNodeWithContentDescription("Score title").performTextReplacement("Ave Maria D. 839")
        compose.onNodeWithContentDescription("Composer or artist (optional)").performTextReplacement("Franz Schubert")
        compose.onNodeWithContentDescription("Score number (optional)").performTextReplacement("12")
        compose.onNodeWithContentDescription("Tags, separated by commas").performTextReplacement("Advent, Maryjne")
        compose.onNodeWithText("Save details").performClick()

        assertEquals(
            listOf<Action>(Action.SaveMetadata(aveMaria.noteKey, "12", "Ave Maria D. 839", "Franz Schubert", "Advent, Maryjne")),
            actions,
        )
    }

    @Test
    fun cancel_and_close_discard_the_edit() {
        show()

        compose.onNodeWithContentDescription("Score title").performTextReplacement("Something else")
        compose.onNodeWithText("Cancel").performClick()
        compose.onNodeWithContentDescription("Close").performClick()

        assertEquals(listOf<Action>(Action.CancelMetadata, Action.CancelMetadata), actions)
    }

    @Test
    fun fields_follow_the_loaded_draft() {
        // The core opens the editor empty and fills it when the worker has loaded the record.
        draft = MetadataDraft(aveMaria.noteKey, number = "", title = "", composer = "", tags = emptyList(), error = null)
        show()
        compose.onNodeWithContentDescription("Score title").assertEditable("")

        draft = aveMaria.copy(tags = listOf("Advent", "Maryjne"))
        compose.waitForIdle()

        compose.onNodeWithContentDescription("Score title").assertEditable("Ave Maria")
        compose.onNodeWithContentDescription("Tags, separated by commas").assertEditable("Advent, Maryjne")
    }

    @Test
    fun the_core_error_shows_and_keeps_the_edit() {
        show()
        compose.onNodeWithContentDescription("Score title").performTextReplacement("")
        compose.onNodeWithText("Save details").performClick()

        draft = aveMaria.copy(error = Failure.EmptyTitle)
        compose.waitForIdle()

        compose.onNodeWithText("title must not be empty").assertIsDisplayed()
        compose.onNodeWithContentDescription("Score title").assertEditable("")
    }

    // Screenshots: the editor over the library, as in metadata-editor and metadata-editor-error.

    private fun shot(state: String, snapshot: Snapshot, dark: Boolean) {
        compose.setContent {
            NotaeTheme(dark = dark) { NotaeNavDisplay(snapshot, onAction = {}, onBack = {}) }
        }
        compose.waitForIdle()
        compose.onRoot().captureRoboImage("src/test/screenshots/$state-landscape-${if (dark) "dark" else "light"}.png")
    }

    private val editing = Fixture.library().copy(metadataEditor = aveMaria)
    private val failed = Fixture.library().copy(metadataEditor = aveMaria.copy(error = Failure.EmptyTitle))

    @Test fun shot_editor_light() = shot("metadata-editor", editing, dark = false)

    @Test fun shot_editor_dark() = shot("metadata-editor", editing, dark = true)

    @Test fun shot_editor_error_light() = shot("metadata-editor-error", failed, dark = false)

    @Test fun shot_editor_error_dark() = shot("metadata-editor-error", failed, dark = true)

    private fun SemanticsNodeInteraction.assertEditable(text: String) =
        assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString(text)))

    private companion object {
        val aveMaria = MetadataDraft(
            noteKey = "key-2",
            number = "",
            title = "Ave Maria",
            composer = "Schubert",
            tags = emptyList(),
            error = null,
        )
    }
}
