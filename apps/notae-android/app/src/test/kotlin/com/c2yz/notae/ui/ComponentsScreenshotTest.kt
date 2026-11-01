package com.c2yz.notae.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.c2yz.notae.R
import com.c2yz.notae.ui.components.ActionSheet
import com.c2yz.notae.ui.components.IconButton
import com.c2yz.notae.ui.components.NotaeCard
import com.c2yz.notae.ui.components.NotaeDialog
import com.c2yz.notae.ui.components.PillButton
import com.c2yz.notae.ui.components.SheetAction
import com.c2yz.notae.ui.components.SidebarItem
import com.c2yz.notae.ui.theme.NotaeTheme
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Light and dark screenshots of each base component, with the label and 48 dp checks. Baselines: app/src/test/screenshots. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w800dp-h600dp-xhdpi")
class ComponentsScreenshotTest {
    @get:Rule
    val compose = createComposeRule()

    private fun shot(name: String, dark: Boolean, content: @Composable () -> Unit) {
        var density = Density(1f)
        compose.setContent {
            density = LocalDensity.current
            NotaeTheme(dark = dark) {
                Box(Modifier.fillMaxSize().background(NotaeTheme.colors.background), Alignment.Center) { content() }
            }
        }
        compose.assertAllClickablesLabelled()
        compose.assertClickTargetsAtLeast48dp(density)
        compose.onRoot().captureRoboImage("src/test/screenshots/${name}_${if (dark) "dark" else "light"}.png")
    }


    @Composable
    private fun CardSample() = NotaeCard(Modifier.width(320.dp).height(120.dp)) {
        Text("Card", Modifier.padding(24.dp))
    }

    @Composable
    private fun PillsSample() = Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            PillButton("Save", {}, primary = true)
            PillButton("Cancel", {}, tonal = true)
            PillButton("Plain", {})
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            PillButton("Scan", {}, icon = painterResource(R.drawable.ic_scan), primary = true)
            PillButton("Disabled", {}, primary = true, enabled = false)
            PillButton("Disabled", {}, tonal = true, enabled = false)
        }
    }

    @Composable
    private fun IconsSample() = Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        IconButton("Search", painterResource(R.drawable.ic_search), {})
        IconButton("Star", painterResource(R.drawable.ic_star_filled), {}, selected = true)
        IconButton("Settings", painterResource(R.drawable.ic_settings), {}, enabled = false)
        IconButton("More", painterResource(R.drawable.ic_more_vertical), {})
    }

    @Composable
    private fun SidebarSample() = NotaeCard(Modifier.width(280.dp)) {
        Column(Modifier.padding(8.dp)) {
            SidebarItem("Library", {}, icon = painterResource(R.drawable.ic_library), count = 42, selected = true)
            SidebarItem("Sets", {}, icon = painterResource(R.drawable.ic_folder), count = 7)
            SidebarItem("Settings", {}, icon = painterResource(R.drawable.ic_settings))
            SidebarItem("A composer with a very long name that has to be elided", {}, count = 3)
        }
    }

    @Composable
    private fun SheetSample() = ActionSheet("Actions", onDismiss = {}) {
        Text("Actions", color = NotaeTheme.colors.foreground)
        SheetAction("Add to set", painterResource(R.drawable.ic_add), {})
        SheetAction("Delete", painterResource(R.drawable.ic_delete), {})
    }

    @Composable
    private fun DialogSample() = NotaeDialog("Delete set") {
        Text("Delete this set?", color = NotaeTheme.colors.foreground)
        Row(Modifier.align(Alignment.End), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            PillButton("Cancel", {})
            PillButton("Delete", {}, primary = true)
        }
    }

    @Test fun card_light() = shot("card", false) { CardSample() }
    @Test fun card_dark() = shot("card", true) { CardSample() }
    @Test fun pillButton_light() = shot("pill_button", false) { PillsSample() }
    @Test fun pillButton_dark() = shot("pill_button", true) { PillsSample() }
    @Test fun iconButton_light() = shot("icon_button", false) { IconsSample() }
    @Test fun iconButton_dark() = shot("icon_button", true) { IconsSample() }
    @Test fun sidebarItem_light() = shot("sidebar_item", false) { SidebarSample() }
    @Test fun sidebarItem_dark() = shot("sidebar_item", true) { SidebarSample() }
    @Test fun actionSheet_light() = shot("action_sheet", false) { SheetSample() }
    @Test fun actionSheet_dark() = shot("action_sheet", true) { SheetSample() }
    @Test fun dialog_light() = shot("dialog", false) { DialogSample() }
    @Test fun dialog_dark() = shot("dialog", true) { DialogSample() }
}
