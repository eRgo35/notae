package com.c2yz.notae.platform

import org.junit.Assert.assertEquals
import org.junit.Test

class HashingTest {
    /**
     * The root PDF of the Slint app's SAF test (`apps/notae-android/tests/saf_catalog.rs`), whose
     * key that test computes with `sha2` and `Sha256Digest`'s lowercase hex.
     */
    @Test
    fun sha256_of_fixture_matches_rust() {
        val fixture = "%PDF-1.4\nroot".toByteArray(Charsets.US_ASCII)

        assertEquals(
            "5c64764463c027476923b6b556b811b3c3bad6d14639ceb01a6b5a7583516e6b",
            sha256Hex(fixture.inputStream()),
        )
    }
}
