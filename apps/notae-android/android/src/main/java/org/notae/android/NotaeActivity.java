package org.notae.android;

import android.app.NativeActivity;
import android.content.Intent;
import android.os.Build;
import android.provider.Settings;

public final class NotaeActivity extends NativeActivity {
    static {
        System.loadLibrary("notae_android");
    }

    private native void nativeOnActivityResult(
        int requestCode, int resultCode, String treeUri, int readGrantFlags);

    // Material dark roles use Android's wallpaper-derived tonal palettes.
    // API 23–30 retains Notae's lavender baseline palette in Slint.
    public int getNotaeColor(int role) {
        if (Build.VERSION.SDK_INT < 31) return 0;
        int neutral = getColor(android.R.color.system_neutral1_900);
        int container = getColor(android.R.color.system_neutral1_800);
        switch (role) {
            case 0: return mix(0xff000000, neutral, 0.78f);
            case 1: return neutral;
            case 2: return mix(neutral, container, 0.4f);
            case 3: return container;
            case 4: return getColor(android.R.color.system_neutral1_100);
            case 5: return getColor(android.R.color.system_neutral2_200);
            case 6: return getColor(android.R.color.system_accent1_200);
            case 7: return getColor(android.R.color.system_accent1_800);
            case 8: return getColor(android.R.color.system_accent1_700);
            case 9: return getColor(android.R.color.system_neutral2_600);
            default: throw new IllegalArgumentException("Unknown Notae color role");
        }
    }

    private static int mix(int first, int second, float amount) {
        int red = Math.round(((first >> 16) & 255) * (1 - amount) + ((second >> 16) & 255) * amount);
        int green = Math.round(((first >> 8) & 255) * (1 - amount) + ((second >> 8) & 255) * amount);
        int blue = Math.round((first & 255) * (1 - amount) + (second & 255) * amount);
        return 0xff000000 | (red << 16) | (green << 8) | blue;
    }

    public boolean isNotaeMotionReduced() {
        return Settings.Global.getFloat(
            getContentResolver(), Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f;
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);

        String treeUri = data == null || data.getData() == null
            ? null : data.getData().toString();
        int readGrantFlags = data == null
            ? 0 : data.getFlags() & Intent.FLAG_GRANT_READ_URI_PERMISSION;
        nativeOnActivityResult(requestCode, resultCode, treeUri, readGrantFlags);
    }
}
