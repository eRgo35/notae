package org.notae.android;

import android.app.NativeActivity;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.Color;
import android.graphics.pdf.PdfRenderer;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import java.io.FileOutputStream;

public final class NotaeActivity extends NativeActivity {
    static {
        System.loadLibrary("notae_android");
    }

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
