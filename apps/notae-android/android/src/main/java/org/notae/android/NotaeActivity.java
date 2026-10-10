package org.notae.android;

import android.app.NativeActivity;
import android.content.Intent;

public final class NotaeActivity extends NativeActivity {
    private native void nativeOnActivityResult(
        int requestCode, int resultCode, String treeUri, int readGrantFlags);

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
