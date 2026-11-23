package com.c2yz.notae.ui.setup

import android.net.Uri
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import com.c2yz.notae.platform.ZipImport
import com.c2yz.notae.platform.ZipImportException
import com.c2yz.notae.ui.theme.NotaeTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Importing scores from a ZIP: the dialog that starts it and the questions and progress over the app. */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w1280dp-h800dp-xhdpi")
class ZipImportTest {
    @get:Rule
    val compose = createComposeRule()

    private val calls = mutableListOf<String>()
    private fun imports(state: ZipImport? = null) = ZipImports(
        state,
        choose = { calls += "choose" },
        openLink = { calls += "open $it" },
        confirm = { calls += "confirm" },
        dismiss = { calls += "dismiss" },
    )

    @Test
    fun an_address_gets_https_and_only_web_addresses_open() {
        assertEquals("https://example.com/nuty.zip", browserAddress("  example.com/nuty.zip "))
        assertEquals("http://chor.pl/a.zip", browserAddress("http://chor.pl/a.zip"))
        assertNull(browserAddress(""))
        assertNull(browserAddress("ftp://chor.pl/a.zip"))
        assertNull(browserAddress("javascript:alert(1)"))
        assertNull(browserAddress("https://"))
    }

    @Test
    fun the_dialog_opens_the_link_in_the_browser() {
        compose.setContent { NotaeTheme { ZipImportDialog(imports(), onClose = { calls += "close" }) } }
        compose.onNodeWithText("Open in browser").assertIsNotEnabled()
        compose.onNodeWithText("Paste the link to the ZIP").performTextInput("drive.google.com/file/abc")
        compose.onNodeWithText("Open in browser").performClick()
        assertEquals(listOf("open https://drive.google.com/file/abc"), calls)
    }

    @Test
    fun the_dialog_chooses_a_zip_file() {
        compose.setContent { NotaeTheme { ZipImportDialog(imports(), onClose = { calls += "close" }) } }
        compose.onNodeWithText("Choose ZIP file").performClick()
        assertEquals(listOf("close", "choose"), calls)
    }

    @Test
    fun a_shared_zip_asks_before_replacing_the_library() {
        compose.setContent { NotaeTheme { ZipImportStatus(imports(ZipImport.Confirm("Nutki.zip", Uri.parse("content://x/1")))) } }
        compose.onNodeWithText("Replace your library with the scores in “Nutki.zip”?").assertExists()
        compose.onNodeWithText("Replace").performClick()
        assertEquals(listOf("confirm"), calls)
    }

    @Test
    fun unpacking_and_failures_are_shown() {
        var state by androidx.compose.runtime.mutableStateOf<ZipImport?>(ZipImport.Unpacking("Nutki.zip"))
        compose.setContent { NotaeTheme { ZipImportStatus(imports(state)) } }
        compose.onNodeWithText("Unpacking “Nutki.zip”…").assertExists()
        state = ZipImport.Failed(ZipImportException.Reason.NO_PDFS, null)
        compose.onNodeWithText("There are no PDFs in this ZIP.").assertExists()
        compose.onNodeWithText("Close").performClick()
        assertEquals(listOf("dismiss"), calls)
    }

    @Test
    fun the_welcome_screen_offers_a_zip() {
        compose.setContent {
            NotaeTheme { WelcomeScreen(com.c2yz.notae.core.SetupScreen.Unconfigured, importAvailable = false, onAction = {}, zip = imports()) }
        }
        compose.onNodeWithText("Import scores from a ZIP").performClick()
        compose.onNodeWithText("Choose ZIP file").assertExists()
    }
}
