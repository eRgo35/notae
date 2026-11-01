package com.c2yz.notae.platform

import android.content.Context
import android.provider.Settings

/** The system's animator duration scale (developer options or "Remove animations"); 1 when unset. */
fun animatorDurationScale(context: Context): Float =
    Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f)

/** Animations run unless the scale is 0, as in 0.3.0 (`v0.3.0:apps/notae-android/src/motion.rs`). */
fun animationsEnabled(context: Context): Boolean = animatorDurationScale(context) != 0f
