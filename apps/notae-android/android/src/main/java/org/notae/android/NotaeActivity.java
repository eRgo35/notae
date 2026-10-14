package org.notae.android;

import android.app.NativeActivity;
import android.content.Intent;
import android.provider.Settings;

public final class NotaeActivity extends NativeActivity {
    static {
        System.loadLibrary("notae_android");
    }

    private native void nativeOnActivityResult(
        int requestCode, int resultCode, String treeUri, int readGrantFlags);

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
