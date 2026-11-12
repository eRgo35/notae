package com.c2yz.notae.platform

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Intent
import android.util.Log
import android.widget.Toast
import androidx.core.net.toUri
import com.c2yz.notae.R

/**
 * Opens [url] in the browser with `ACTION_VIEW`, on the main thread; the browser fetches it, as Notae has no network
 * access. With no app to open it, a toast says so instead.
 */
fun openUrl(activity: Activity, url: String) {
    try {
        activity.startActivity(Intent(Intent.ACTION_VIEW, url.toUri()))
    } catch (error: ActivityNotFoundException) {
        Log.w("Notae", "no app can open $url", error)
        Toast.makeText(activity, R.string.settings_no_browser, Toast.LENGTH_LONG).show()
    }
}
