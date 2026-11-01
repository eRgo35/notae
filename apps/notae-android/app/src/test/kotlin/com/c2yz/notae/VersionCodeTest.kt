package com.c2yz.notae

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import versionCodeFor

class VersionCodeTest {
    @Test
    fun matchesTheCargoApkCodes() {
        assertEquals(16777730, versionCodeFor("0.2.2"))
    }

    @Test
    fun risesWithTheVersion() {
        assertTrue(versionCodeFor("0.5.0") > versionCodeFor("0.3.0"))
    }
}
