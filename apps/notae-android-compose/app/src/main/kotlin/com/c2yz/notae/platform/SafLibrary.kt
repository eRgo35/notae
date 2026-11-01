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
import java.io.FileNotFoundException
import java.io.FileOutputStream
import java.io.InputStream
import java.security.MessageDigest
import kotlin.math.max
import kotlin.math.roundToInt

/**
 * The core's [PlatformLibrary] over the Storage Access Framework. A handle is the library's tree
 * URI; a path is `/`-separated below it, built from display names as the Slint app built it
 * (`apps/notae-android/src/saf_tree.rs`), so an existing index keeps matching.
 *
 * Every failure reaches the core as a [PlatformException]: a revoked grant ([SecurityException])
 * as [PlatformException.PermissionLost], anything else as [PlatformException.Unavailable].
 */
class SafLibrary(context: Context) : PlatformLibrary {
    private val resolver: ContentResolver = context.contentResolver

    override fun checkRoot(handle: String) = platformCall {
        val tree = handle.toUri()
        // Retaking a grant that was revoked throws SecurityException.
        resolver.takePersistableUriPermission(tree, Intent.FLAG_GRANT_READ_URI_PERMISSION)
        val root = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
        val found = try {
            resolver.query(root, arrayOf(Document.COLUMN_DOCUMENT_ID), null, null)?.use { it.moveToFirst() }
        } catch (_: FileNotFoundException) {
            false
        }
        if (found != true) throw PlatformException.PermissionLost()
    }

    override fun enumerate(handle: String): List<DocumentEntry> = platformCall {
        val tree = handle.toUri()
        val entries = mutableListOf<DocumentEntry>()
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

    /** Page 1 as a [THUMBNAIL_WIDTH_PX]-wide PNG on white, as the Slint app's `renderNotaeThumbnail`. */
    override fun renderThumbnail(handle: String, path: String, outPng: String): Boolean = platformCall {
        val document = documentUri(resolver, handle.toUri(), path)
        try {
            withRenderer(document) { renderer -> renderer.pageCount > 0 && renderFirstPage(renderer, outPng) }
        } catch (_: Exception) {
            false
        }
    }

    private fun enumerateChildren(tree: Uri, parentId: String, parentPath: String, entries: MutableList<DocumentEntry>) {
        for (child in children(resolver, tree, parentId)) {
            if (child.isDirectory) {
                enumerateChildren(tree, child.id, joinPath(parentPath, child.name), entries)
            } else if (isPdfName(child.name)) {
                val path = joinPath(parentPath, child.name)
                entries += readEntry(DocumentsContract.buildDocumentUriUsingTree(tree, child.id), path, child.name)
            }
        }
    }

    /** One PDF's entry; a document that cannot be read is listed as unreadable with size 0. */
    private fun readEntry(document: Uri, path: String, name: String): DocumentEntry = try {
        val stream = resolver.openInputStream(document) ?: throw FileNotFoundException("no stream for $path")
        val counted = CountingStream(stream)
        val hex = counted.use { sha256Hex(it) }
        DocumentEntry(path, name, counted.count.toULong(), DocumentContent.Sha256(hex))
    } catch (error: SecurityException) {
        throw error
    } catch (_: Exception) {
        DocumentEntry(path, name, 0uL, DocumentContent.Unreadable)
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

    private class CountingStream(private val input: InputStream) : InputStream() {
        var count = 0L
            private set

        override fun read(): Int = input.read().also { if (it >= 0) count++ }

        override fun read(buffer: ByteArray, offset: Int, length: Int): Int =
            input.read(buffer, offset, length).also { if (it > 0) count += it }

        override fun close() = input.close()
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

/** Resolves the library-relative PDF [path] below [tree] to its document URI, walking display names. */
internal fun documentUri(resolver: ContentResolver, tree: Uri, path: String): Uri {
    val segments = path.split('/')
    if (!isPdfName(segments.last())) throw IllegalArgumentException("not a PDF library path: $path")
    var parentId = DocumentsContract.getTreeDocumentId(tree)
    segments.forEachIndexed { index, segment ->
        val isFile = index == segments.lastIndex
        val matches = children(resolver, tree, parentId).filter { it.name == segment && it.isDirectory != isFile }
        parentId = when (matches.size) {
            1 -> matches.single().id
            0 -> throw FileNotFoundException("SAF document not found: $path")
            else -> throw IllegalStateException("ambiguous SAF path segment \"$segment\" in $path")
        }
    }
    return DocumentsContract.buildDocumentUriUsingTree(tree, parentId)
}

/**
 * Runs a [PlatformLibrary] call so only a [PlatformException] leaves it: a revoked grant becomes
 * [PlatformException.PermissionLost], any other exception [PlatformException.Unavailable].
 */
internal inline fun <T> platformCall(block: () -> T): T = try {
    block()
} catch (error: PlatformException) {
    throw error
} catch (_: SecurityException) {
    throw PlatformException.PermissionLost()
} catch (error: Exception) {
    throw PlatformException.Unavailable(error.message ?: error.toString())
}

private class DocumentRow(val id: String, val name: String, val isDirectory: Boolean)

/** The immediate children of [parentId], in the provider's order. */
private fun children(resolver: ContentResolver, tree: Uri, parentId: String): List<DocumentRow> {
    val uri = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parentId)
    val projection = arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME, Document.COLUMN_MIME_TYPE)
    val cursor = resolver.query(uri, projection, null, null)
        ?: throw IllegalStateException("cannot list SAF children of document $parentId")
    return cursor.use {
        buildList {
            while (it.moveToNext()) {
                add(DocumentRow(it.string(Document.COLUMN_DOCUMENT_ID), it.string(Document.COLUMN_DISPLAY_NAME), it.string(Document.COLUMN_MIME_TYPE) == Document.MIME_TYPE_DIR))
            }
        }
    }
}

private fun Cursor.string(column: String): String =
    getString(getColumnIndexOrThrow(column)) ?: throw IllegalStateException("SAF cursor returned null for column $column")

/** A display name must be one path segment; otherwise the whole listing fails, as in the Slint app. */
private fun joinPath(parent: String, name: String): String {
    require(name.isNotEmpty() && name != "." && name != ".." && '/' !in name && '\\' !in name) {
        "SAF display name is not one path segment: \"$name\""
    }
    return if (parent.isEmpty()) name else "$parent/$name"
}

private fun isPdfName(name: String) = name.length >= 4 && name.endsWith(".pdf", ignoreCase = true)
