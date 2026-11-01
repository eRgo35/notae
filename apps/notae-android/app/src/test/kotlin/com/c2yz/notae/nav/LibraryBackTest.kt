package com.c2yz.notae.nav

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.c2yz.notae.ui.library.Fixture
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Back closes the library's ⋮ sheet, which only the screen knows about, before the core is asked. */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class LibraryBackTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun back_closes_the_score_sheet_first() {
        var backs = 0
        compose.setContent {
            NotaeTheme { NotaeNavDisplay(Fixture.library(), onAction = {}, onBack = { backs++ }) }
        }
        compose.onNodeWithContentDescription("More actions for Ave Maria").performClick()
        compose.onNodeWithText("Open score").assertExists()

        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()

        compose.onNodeWithText("Open score").assertDoesNotExist()
        assertEquals(0, backs)

        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
        assertEquals(1, backs)
    }
}
