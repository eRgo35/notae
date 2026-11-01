package com.c2yz.notae.ui.library

import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
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

/**
 * The thumbnail at [path], from the cache or decoded off the main thread; `null` until then, or when there is none.
 * The state is per path, so a new path never shows the previous one's picture.
 */
@Composable
internal fun rememberThumbnail(path: String?): State<ImageBitmap?> {
    val state = remember(path) { mutableStateOf(path?.let { ThumbnailCache.get(it) }) }
    LaunchedEffect(path) {
        if (path == null || state.value != null) return@LaunchedEffect
        state.value = withContext(Dispatchers.IO) {
            BitmapFactory.decodeFile(path)?.asImageBitmap()?.also { ThumbnailCache.put(path, it) }
        }
    }
    return state
}
