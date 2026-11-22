package com.c2yz.notae

import android.app.Application

/**
 * The app as every Robolectric test starts it (`robolectric.properties`). Robolectric's native runtime looks up the
 * java.nio buffer classes the first time it needs them, on whichever thread that is. From a thread the runtime did not
 * start on (the core's own thread, a renderer's) such a lookup finds only classes the JVM has already loaded, and a
 * miss aborts the whole test JVM ("JniConstants: Class not found: java/nio/ShortBuffer"; FloatBuffer once ShortBuffer
 * alone was loaded first). Loading them here, before any test runs, makes every lookup a hit.
 */
class NativeRuntimeTestApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        listOf(
            "java.nio.Buffer", "java.nio.ByteBuffer", "java.nio.CharBuffer", "java.nio.ShortBuffer", "java.nio.IntBuffer",
            "java.nio.LongBuffer", "java.nio.FloatBuffer", "java.nio.DoubleBuffer", "java.io.FileDescriptor",
        ).forEach { Class.forName(it) }
    }
}
