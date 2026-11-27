package com.c2yz.notae.platform

import android.content.ContentResolver
import android.content.Context
import android.content.Intent
import android.database.Cursor
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.pdf.PdfRenderer
import android.net.Uri
import android.os.Build
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import androidx.core.graphics.createBitmap
import androidx.core.net.toUri
import com.c2yz.notae.core.DocumentContent
import com.c2yz.notae.core.DocumentEntry
import com.c2yz.notae.core.PlatformException
import com.c2yz.notae.core.PlatformLibrary
import java.io.File
import java.io.FileNotFoundException
import java.io.FileOutputStream
import java.io.InputStream
import java.security.MessageDigest
import java.util.concurrent.ConcurrentHashMap
import kotlin.math.max
import kotlin.math.roundToInt

/** The folder holding the core's PC sidecar files. */
internal const val SIDECAR_DIR = ".notae"

/** Root folders that are not scanned: the sidecar, transcription sources, original scans and other material. Matches the sidecar tool. */
internal val SKIPPED_ROOT_DIRS = setOf(SIDECAR_DIR, "sources", "skany", "inne")

/**
 * The core's [PlatformLibrary] over the Storage Access Framework. A handle is the library's tree
 * URI; a path is `/`-separated below it, built from display names as 0.3.0 built it
 * (`v0.3.0:apps/notae-android/src/saf_tree.rs`), so an existing index keeps matching.
 *
 * Every failure reaches the core as a [PlatformException]: a revoked grant ([SecurityException])
 * as [PlatformException.PermissionLost], anything else as [PlatformException.Unavailable].
 */
class SafLibrary(context: Context) : PlatformLibrary {
    private val resolver: ContentResolver = context.contentResolver

    override fun checkRoot(handle: String) = platformCall {
        val tree = handle.toUri()
        // A library imported from a ZIP: a folder in Notae's own storage, which only goes when the app's data is cleared.
        fileRoot(tree)?.let { root ->
            if (!root.isDirectory) throw PlatformException.PermissionLost()
            return@platformCall
        }
        // Retaking a grant that was revoked throws SecurityException.
        resolver.takePersistableUriPermission(tree, Intent.FLAG_GRANT_READ_URI_PERMISSION)
        val root = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
        // A deleted or moved tree: providers throw FileNotFoundException, or IllegalArgumentException
        // (DocumentsProvider wraps the former; ExternalStorageProvider throws the latter itself).
        val found = try {
            resolver.query(root, arrayOf(Document.COLUMN_DOCUMENT_ID), null, null)?.use { it.moveToFirst() }
        } catch (_: FileNotFoundException) {
            false
        } catch (_: IllegalArgumentException) {
            false
        }
        if (found != true) throw PlatformException.PermissionLost()
    }

    /** Directory listings kept for one scan, so repeated path lookups share a query; cleared by [enumerate]. */
    private val listings = ConcurrentHashMap<String, List<DocumentRow>>()

    private fun listing(tree: Uri, parentId: String): List<DocumentRow> =
        listings.getOrPut("$tree|$parentId") { children(resolver, tree, parentId) }

    override fun enumerate(handle: String): List<DocumentEntry> = platformCall {
        listings.clear()
        val tree = handle.toUri()
        val entries = mutableListOf<DocumentEntry>()
        fileRoot(tree)?.let { root ->
            enumerateFiles(root, "", entries)
            return@platformCall entries
        }
        enumerateChildren(tree, DocumentsContract.getTreeDocumentId(tree), "", entries)
        entries
    }

    /** The raw text of every page, items joined by spaces and pages by newlines; the core normalizes it. */
    override fun extractTextLayer(handle: String, path: String): String? = platformCall {
        val document = documentUri(resolver, handle.toUri(), path)
        withRenderer(document) { renderer ->
            check(renderer.pageCount > 0) { "PDF has no readable pages" }
            if (Build.VERSION.SDK_INT < TEXT_CONTENTS_API) return@withRenderer null
            (0 until renderer.pageCount).joinToString("\n") { index ->
                renderer.openPage(index).use { page -> page.textContents.joinToString(" ") { it.text } }
            }
        }
    }

    /**
     * Page 1 as a [THUMBNAIL_WIDTH_PX]-wide PNG on white, as 0.3.0's `renderNotaeThumbnail`.
     * Any rendering failure, an [OutOfMemoryError] for an extreme page shape included, gives false.
     */
    override fun renderThumbnail(handle: String, path: String, outPng: String): Boolean = platformCall {
        val document = documentUri(resolver, handle.toUri(), path)
        try {
            withRenderer(document) { renderer -> renderer.pageCount > 0 && renderFirstPage(renderer, outPng) }
        } catch (_: Throwable) {
            false
        }
    }

    private fun enumerateChildren(tree: Uri, parentId: String, parentPath: String, entries: MutableList<DocumentEntry>) {
        for (child in listing(tree, parentId)) {
            if (child.isDirectory) {
                // The sidecar folder is the core's own and the other skipped folders hold no scores.
                if (parentPath.isEmpty() && child.name in SKIPPED_ROOT_DIRS) continue
                enumerateChildren(tree, child.id, joinPath(parentPath, child.name), entries)
            } else if (isPdfName(child.name)) {
                entries += DocumentEntry(joinPath(parentPath, child.name), child.name, child.size.toULong(), child.modified)
            }
        }
    }

    private fun enumerateFiles(dir: File, parentPath: String, entries: MutableList<DocumentEntry>) {
        val children = dir.listFiles() ?: throw FileNotFoundException("cannot list $dir")
        for (child in children) {
            if (child.isDirectory) {
                if (parentPath.isEmpty() && child.name in SKIPPED_ROOT_DIRS) continue
                enumerateFiles(child, joinPath(parentPath, child.name), entries)
            } else if (isPdfName(child.name)) {
                entries += DocumentEntry(joinPath(parentPath, child.name), child.name, child.length().toULong(), child.lastModified().takeIf { it > 0 })
            }
        }
    }

    /** One PDF's content key; a document that cannot be read is [DocumentContent.Unreadable]. */
    override fun hash(handle: String, path: String): DocumentContent = platformCall {
        try {
            val stream = resolver.openInputStream(documentUri(resolver, handle.toUri(), path, listing = ::listing))
                ?: throw FileNotFoundException("no stream for $path")
            DocumentContent.Sha256(stream.use { sha256Hex(it) })
        } catch (error: SecurityException) {
            throw error
        } catch (_: Exception) {
            DocumentContent.Unreadable
        }
    }

    /** The bytes of the library-relative [path], or null when it does not exist. */
    override fun readFile(handle: String, path: String): ByteArray? = platformCall {
        try {
            resolver.openInputStream(documentUri(resolver, handle.toUri(), path, requirePdf = false, listing = ::listing))?.use { it.readBytes() }
        } catch (_: FileNotFoundException) {
            null
        }
    }

    private fun <T> withRenderer(document: Uri, block: (PdfRenderer) -> T): T {
        val descriptor = resolver.openFileDescriptor(document, "r") ?: throw FileNotFoundException("no descriptor for $document")
        // The renderer closes the descriptor, once it exists.
        val renderer = try {
            PdfRenderer(descriptor)
        } catch (error: Exception) {
            descriptor.close()
            throw error
        }
        return renderer.use(block)
    }

    private fun renderFirstPage(renderer: PdfRenderer, outPng: String): Boolean = renderer.openPage(0).use { page ->
        val height = max(1, (THUMBNAIL_WIDTH_PX * page.height.toFloat() / max(1, page.width)).roundToInt())
        val bitmap = createBitmap(THUMBNAIL_WIDTH_PX, height)
        try {
            bitmap.eraseColor(Color.WHITE)
            page.render(bitmap, null, null, PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY)
            FileOutputStream(outPng).use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        } finally {
            bitmap.recycle()
        }
    }

    private companion object {
        /** `PdfRenderer.Page.getTextContents` arrived in API 35. */
        const val TEXT_CONTENTS_API = 35

        /** The core's `thumbnails::THUMBNAIL_WIDTH_PX`. */
        const val THUMBNAIL_WIDTH_PX = 400
    }
}

/** The lowercase hex SHA-256 of [input], streamed; the core's content key. */
fun sha256Hex(input: InputStream): String {
    val digest = MessageDigest.getInstance("SHA-256")
    val buffer = ByteArray(64 * 1024)
    while (true) {
        val read = input.read(buffer)
        if (read < 0) break
        digest.update(buffer, 0, read)
    }
    return digest.digest().joinToString("") { "%02x".format(it) }
}

/** Resolves the library-relative [path] below [tree] to its document URI, walking display names; a PDF unless [requirePdf] is false. */
internal fun documentUri(resolver: ContentResolver, tree: Uri, path: String, requirePdf: Boolean = true,
    listing: (Uri, String) -> List<DocumentRow> = { t, id -> children(resolver, t, id) },
): Uri {
    val segments = path.split('/')
    if (requirePdf && !isPdfName(segments.last())) throw IllegalArgumentException("not a PDF library path: $path")
    fileRoot(tree)?.let { root -> return Uri.fromFile(libraryFile(root, path)) }
    var parentId = DocumentsContract.getTreeDocumentId(tree)
    segments.forEachIndexed { index, segment ->
        val isFile = index == segments.lastIndex
        val matches = listing(tree, parentId).filter { it.name == segment && it.isDirectory != isFile }
        parentId = when (matches.size) {
            1 -> matches.single().id
            0 -> throw FileNotFoundException("SAF document not found: $path")
            else -> throw IllegalStateException("ambiguous SAF path segment \"$segment\" in $path")
        }
    }
    return DocumentsContract.buildDocumentUriUsingTree(tree, parentId)
}

/** The folder a `file:` library handle names (a library imported from a ZIP); null for a SAF tree. */
internal fun fileRoot(tree: Uri): File? =
    if (tree.scheme == ContentResolver.SCHEME_FILE) File(tree.path ?: throw IllegalArgumentException("Invalid URI: $tree")) else null

/** The existing file at the library-relative [path] below [root]; a path that leaves [root] is refused. */
internal fun libraryFile(root: File, path: String): File {
    val base = root.canonicalFile
    val file = File(base, path).canonicalFile
    require(file.path.startsWith(base.path + File.separator)) { "not a library path: $path" }
    if (!file.isFile) throw FileNotFoundException("not found: $path")
    return file
}

/**
 * Runs a [PlatformLibrary] call so only a [PlatformException] leaves it: a revoked grant becomes
 * [PlatformException.PermissionLost], any other throwable [PlatformException.Unavailable]. Errors
 * too, since UniFFI's callback wrapper catches only `Exception` and anything else would reach the
 * core as a success with a default value.
 */
internal inline fun <T> platformCall(block: () -> T): T = try {
    block()
} catch (error: PlatformException) {
    throw error
} catch (_: SecurityException) {
    throw PlatformException.PermissionLost()
} catch (error: Throwable) {
    throw PlatformException.Unavailable(error.message ?: error.toString())
}

internal class DocumentRow(val id: String, val name: String, val isDirectory: Boolean, val size: Long, val modified: Long?)

/** The immediate children of [parentId], in the provider's order. */
internal fun children(resolver: ContentResolver, tree: Uri, parentId: String): List<DocumentRow> {
    val uri = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parentId)
    val projection = arrayOf(
        Document.COLUMN_DOCUMENT_ID,
        Document.COLUMN_DISPLAY_NAME,
        Document.COLUMN_MIME_TYPE,
        Document.COLUMN_SIZE,
        Document.COLUMN_LAST_MODIFIED,
    )
    val cursor = resolver.query(uri, projection, null, null)
        ?: throw IllegalStateException("cannot list SAF children of document $parentId")
    return cursor.use {
        buildList {
            while (it.moveToNext()) {
                add(
                    DocumentRow(
                        it.string(Document.COLUMN_DOCUMENT_ID),
                        it.string(Document.COLUMN_DISPLAY_NAME),
                        it.string(Document.COLUMN_MIME_TYPE) == Document.MIME_TYPE_DIR,
                        // Providers report an unknown size or time as null, 0 or a negative; neither may match an index.
                        it.long(Document.COLUMN_SIZE)?.coerceAtLeast(0L) ?: 0L,
                        it.long(Document.COLUMN_LAST_MODIFIED)?.takeIf { millis -> millis > 0 },
                    ),
                )
            }
        }
    }
}

private fun Cursor.long(column: String): Long? =
    getColumnIndexOrThrow(column).let { if (isNull(it)) null else getLong(it) }

private fun Cursor.string(column: String): String =
    getString(getColumnIndexOrThrow(column)) ?: throw IllegalStateException("SAF cursor returned null for column $column")

/** A display name must be one path segment; otherwise the whole listing fails, as in 0.3.0. */
private fun joinPath(parent: String, name: String): String {
    require(name.isNotEmpty() && name != "." && name != ".." && '/' !in name && '\\' !in name) {
        "SAF display name is not one path segment: \"$name\""
    }
    return if (parent.isEmpty()) name else "$parent/$name"
}

private fun isPdfName(name: String) = name.length >= 4 && name.endsWith(".pdf", ignoreCase = true)
