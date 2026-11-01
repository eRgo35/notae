package com.c2yz.notae

import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class LauncherIconTest {
    @Test
    fun the_app_uses_the_notae_launcher_icon() {
        val app = RuntimeEnvironment.getApplication()
        val icon = app.applicationInfo.icon
        assertEquals("mipmap/ic_launcher", if (icon == 0) "none" else app.resources.getResourceName(icon).substringAfter(':'))
    }
}
