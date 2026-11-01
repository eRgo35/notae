package com.c2yz.notae.ui.sets

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import com.c2yz.notae.core.SetDialog
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Landscape with the soft keyboard up leaves about 300 dp above it (ruling R47). The fields scroll and Cancel and Save
 * stay in view.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w1280dp-h300dp-xhdpi")
class SetDialogShortWindowTest {
    @get:Rule
    val compose = createComposeRule()

    @Test
    fun cancel_and_save_stay_in_view_in_a_short_window() {
        val dialog = SetDialog(null, "", null, "Msza 10:00, kantor: Anna", null)
        compose.setContent { NotaeTheme { SetOverlays(Fixture.sets().copy(setDialog = dialog), {}) } }

        compose.onNodeWithText("Cancel").assertIsDisplayed()
        compose.onNodeWithText("Save set").assertIsDisplayed()
    }
}
