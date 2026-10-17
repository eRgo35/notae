package org.notae.android;

import android.app.NativeActivity;
import android.content.Intent;
import android.content.res.Configuration;
import android.content.ActivityNotFoundException;
import android.speech.RecognizerIntent;
import android.graphics.Bitmap;
import android.graphics.Color;
import android.graphics.drawable.ColorDrawable;
import android.graphics.pdf.PdfRenderer;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.provider.Settings;
import android.os.ParcelFileDescriptor;
import android.view.View;
import android.view.Window;
import android.view.WindowInsetsController;
import java.io.FileOutputStream;

public final class NotaeActivity extends NativeActivity {
    static {
        System.loadLibrary("notae_android");
    }
    private static final int VOICE_REQUEST_CODE = 0x4e4f;

    private native void nativeOnVoiceResult(int requestCode, int resultCode, String[] results);
    private native void nativeOnKeyboardConfigurationChanged(boolean present);
    private native void nativeOnAnimatorDurationScaleChanged(float scale);
    private native void nativeOnApplicationActiveChanged(boolean active);

    public boolean hasVoiceRecognizer() {
        Intent intent = new Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH);
        return intent.resolveActivity(getPackageManager()) != null;
    }

    public boolean startVoiceRecognition() {
        Intent intent = new Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH);
        intent.putExtra(
            RecognizerIntent.EXTRA_LANGUAGE_MODEL,
            RecognizerIntent.LANGUAGE_MODEL_FREE_FORM);
        intent.putExtra(
            RecognizerIntent.EXTRA_LANGUAGE,
            java.util.Locale.getDefault().toLanguageTag());
        try {
            startActivityForResult(intent, VOICE_REQUEST_CODE);
            return true;
        } catch (ActivityNotFoundException error) {
            return false;
        }
    }

    public boolean hasHardwareKeyboard() {
        Configuration configuration = getResources().getConfiguration();
        return configuration.keyboard == Configuration.KEYBOARD_QWERTY
            && configuration.hardKeyboardHidden == Configuration.HARDKEYBOARDHIDDEN_NO;
    }
    public float readAnimatorDurationScale() {
        return Settings.Global.getFloat(
            getContentResolver(),
            Settings.Global.ANIMATOR_DURATION_SCALE,
            1.0f);
    }

    private native void nativeOnActivityResult(
        int requestCode, int resultCode, String treeUri, int readGrantFlags);

    // Slint pads its content by the window manager's system-bar insets, so the window itself must
    // draw edge-to-edge and must not add a framework action bar; otherwise the UI is inset twice.
    @SuppressWarnings("deprecation") // The pre-API-30 path needs the legacy system UI flags.
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        requestWindowFeature(Window.FEATURE_NO_TITLE);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            getWindow().setDecorFitsSystemWindows(false);
        }
        super.onCreate(savedInstanceState);
        getWindow().setStatusBarColor(Color.TRANSPARENT);
        getWindow().setNavigationBarColor(Color.TRANSPARENT);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            getWindow().setStatusBarContrastEnforced(false);
            getWindow().setNavigationBarContrastEnforced(false);
        }
        boolean night = (getResources().getConfiguration().uiMode & Configuration.UI_MODE_NIGHT_MASK)
            == Configuration.UI_MODE_NIGHT_YES;
        getWindow().setBackgroundDrawable(new ColorDrawable(
            night ? Color.BLACK : Color.rgb(243, 243, 245)));
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            WindowInsetsController controller = getWindow().getInsetsController();
            if (controller != null) {
                int light = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS
                    | WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS;
                controller.setSystemBarsAppearance(night ? 0 : light, light);
            }
        } else {
            int flags = View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION;
            if (!night) {
                flags |= View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR;
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    flags |= View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR;
                }
            }
            getWindow().getDecorView().setSystemUiVisibility(flags);
        }
        nativeOnKeyboardConfigurationChanged(hasHardwareKeyboard());
    }

    @Override
    public void onConfigurationChanged(Configuration newConfig) {
        super.onConfigurationChanged(newConfig);
        nativeOnKeyboardConfigurationChanged(hasHardwareKeyboard());
    }

    @Override
    protected void onResume() {
        super.onResume();
        nativeOnApplicationActiveChanged(true);
        nativeOnAnimatorDurationScaleChanged(readAnimatorDurationScale());
    }

    @Override
    protected void onPause() {
        nativeOnApplicationActiveChanged(false);
        super.onPause();
    }


    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);

        if (requestCode == VOICE_REQUEST_CODE) {
            java.util.ArrayList<String> phrases = data == null
                ? null : data.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS);
            nativeOnVoiceResult(
                requestCode,
                resultCode,
                phrases == null ? null : phrases.toArray(new String[0]));
            return;
        }

        String treeUri = data == null || data.getData() == null
            ? null : data.getData().toString();
        int readGrantFlags = data == null
            ? 0 : data.getFlags() & Intent.FLAG_GRANT_READ_URI_PERMISSION;
        nativeOnActivityResult(requestCode, resultCode, treeUri, readGrantFlags);
    }

    // Renders page 1 of a SAF document to a PNG in app-private storage for the library grid.
    public boolean renderNotaeThumbnail(String uri, String outPath, int widthPx) {
        try (ParcelFileDescriptor descriptor =
                 getContentResolver().openFileDescriptor(Uri.parse(uri), "r")) {
            if (descriptor == null) return false;
            try (PdfRenderer renderer = new PdfRenderer(descriptor)) {
                if (renderer.getPageCount() == 0) return false;
                try (PdfRenderer.Page page = renderer.openPage(0)) {
                    int height = Math.max(1, Math.round(
                        widthPx * (float) page.getHeight() / Math.max(1, page.getWidth())));
                    Bitmap bitmap = Bitmap.createBitmap(widthPx, height, Bitmap.Config.ARGB_8888);
                    bitmap.eraseColor(Color.WHITE);
                    page.render(bitmap, null, null, PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY);
                    try (FileOutputStream out = new FileOutputStream(outPath)) {
                        boolean written = bitmap.compress(Bitmap.CompressFormat.PNG, 100, out);
                        bitmap.recycle();
                        return written;
                    }
                }
            }
        } catch (Exception error) {
            return false;
        }
    }
}
