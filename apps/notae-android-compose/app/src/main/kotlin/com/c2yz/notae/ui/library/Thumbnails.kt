package com.c2yz.notae.ui.library

import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.produceState
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * The first-page PNGs the core's scan rendered, by file path (design spec §4.2: `BitmapFactory` into an in-memory
 * LRU cache, no image library). A thumbnail is keyed by content, so a path always holds the same picture.
 */
private object ThumbnailCache : LruCache<String, ImageBitmap>(MAX_KIB) {
    override fun sizeOf(key: String, value: ImageBitmap): Int = (value.width * value.height * 4 / 1024).coerceAtLeast(1)
}

/** A quarter of a typical tablet heap's 64 MiB budget for images, in KiB. */
private const val MAX_KIB = 16 * 1024

/** The thumbnail at [path], decoded off the main thread; `null` until then, or when there is none. */
@Composable
internal fun rememberThumbnail(path: String?): State<ImageBitmap?> =
    produceState(path?.let { ThumbnailCache.get(it) }, path) {
        if (path == null || value != null) return@produceState
        value = withContext(Dispatchers.IO) {
            BitmapFactory.decodeFile(path)?.asImageBitmap()?.also { ThumbnailCache.put(path, it) }
        }
    }
