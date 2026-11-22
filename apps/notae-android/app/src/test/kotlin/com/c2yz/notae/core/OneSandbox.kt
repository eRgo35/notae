package com.c2yz.notae.core

/**
 * Every test that touches the native core must run in one Robolectric sandbox: the default SDK with NATIVE graphics.
 * Rust keeps one process-wide table of UniFFI callbacks, which the bindings register from whichever sandbox's class
 * loader loaded them last, and each sandbox has its own handle map. A core in an earlier sandbox then gets "Invalid
 * handle" on every callback: snapshots and effects are lost and scans never finish, depending on the test order.
 *
 * So a test that creates a core uses `@GraphicsMode(NATIVE)` and no `@Config(sdk = …)`, and a test that only renders
 * uses `Fixture.base` instead of a core. [assertOneSandbox] turns a breach into a clear failure instead of a timeout.
 */
internal fun assertOneSandbox() {
    // A system property, because it is the one place every sandbox's class loader shares.
    val loader = System.identityHashCode(NotaeCore::class.java.classLoader).toString()
    val first = System.getProperty(PROPERTY) ?: loader.also { System.setProperty(PROPERTY, it) }
    check(first == loader) {
        "A second Robolectric sandbox created a NotaeCore; it would take the UniFFI callbacks from the first. " +
            "Use @GraphicsMode(NATIVE) and no @Config(sdk = …) in tests that create a core (OneSandbox.kt)."
    }
}

/** A core over [dataDir] for a test, checked to be in the one sandbox. */
internal fun testCore(dataDir: String, platform: PlatformLibrary, listener: CoreListener): NotaeCore {
    assertOneSandbox()
    loadNativeRuntimeClasses()
    return NotaeCore(dataDir, platform, listener)
}

/**
 * Robolectric's native runtime looks up the java.nio buffer classes the first time it needs them, on whichever thread
 * that is. From the core's own thread (a scan starts on the folder pick) such a lookup finds only classes the JVM has
 * already loaded, and a miss aborts the whole test JVM ("JniConstants: Class not found: java/nio/ShortBuffer", then
 * FloatBuffer once ShortBuffer was loaded). Loading them on the test thread first makes every lookup a hit.
 */
private fun loadNativeRuntimeClasses() {
    listOf(
        "java.nio.Buffer", "java.nio.ByteBuffer", "java.nio.CharBuffer", "java.nio.ShortBuffer", "java.nio.IntBuffer",
        "java.nio.LongBuffer", "java.nio.FloatBuffer", "java.nio.DoubleBuffer", "java.io.FileDescriptor",
    ).forEach { Class.forName(it) }
}

private const val PROPERTY = "notae.test.coreSandbox"
