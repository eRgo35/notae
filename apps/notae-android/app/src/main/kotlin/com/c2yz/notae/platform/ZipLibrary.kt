package com.c2yz.notae.platform

import android.content.ContentResolver
import android.net.Uri
import android.provider.OpenableColumns
import java.io.BufferedInputStream
import java.io.File
import java.io.IOException
import java.io.InputStream
import java.nio.charset.Charset
import java.util.zip.ZipException
import java.util.zip.ZipInputStream

/** The folder in Notae's files directory holding a library imported from a ZIP. */
const val ZIP_LIBRARY_DIR = "zip-library"

/** The space an import leaves free at least, so Notae's index and thumbnails still fit. */
private const val SPARE_BYTES = 50L * 1024 * 1024

/** Why a ZIP could not become the library; [detail] is the platform's message, when there is one. */
class ZipImportException(val reason: Reason, detail: String? = null) : IOException(detail ?: reason.name) {
    enum class Reason { NOT_ZIP, NO_PDFS, NO_SPACE, UNREADABLE }
}

/** The core's library handle for the ZIP library below [filesDir]: a `file:` URI. */
fun zipLibraryHandle(filesDir: File): String = Uri.fromFile(File(filesDir, ZIP_LIBRARY_DIR)).toString()

/** Deletes the copy a ZIP import left in [filesDir], once a folder is the library again; the user's ZIP is not touched. */
fun removeZipLibrary(filesDir: File) {
    File(filesDir, ZIP_LIBRARY_DIR).deleteRecursively()
}

/**
 * Unpacks the ZIP that [open] streams into `filesDir/zip-library`, replacing what was there, and returns that folder. It
 * keeps PDFs and the PC sidecar (`.notae/`), skipping hidden and other files and `__MACOSX`; names are read as UTF-8,
 * or as the Polish Windows code page when they are not UTF-8. It unpacks beside the library first, so a failure keeps
 * the old one, and throws [ZipImportException]: not a ZIP, no PDFs in it, less than [SPARE_BYTES] would stay free
 * ([usableSpace]), or unreadable (an entry that would land outside the folder included). [open] may be called twice.
 */
fun unpackZipLibrary(open: () -> InputStream, filesDir: File, usableSpace: () -> Long = { filesDir.usableSpace }): File {
    val target = File(filesDir, ZIP_LIBRARY_DIR)
    val staging = File(filesDir, "$ZIP_LIBRARY_DIR.new")
    try {
        val pdfs = try {
            unpackInto(open, staging, Charsets.UTF_8, usableSpace)
        } catch (_: NotUtf8) {
            // A name that is not UTF-8: most likely a ZIP made on Windows. A broken ZIP fails again.
            unpackInto(open, staging, windowsCharset(), usableSpace)
        }
        if (pdfs == 0) throw ZipImportException(ZipImportException.Reason.NO_PDFS)
        target.deleteRecursively()
        if (!staging.renameTo(target)) throw ZipImportException(ZipImportException.Reason.UNREADABLE, "cannot move the unpacked scores into place")
        return target
    } catch (error: ZipImportException) {
        throw error
    } catch (error: IllegalArgumentException) {
        throw ZipImportException(ZipImportException.Reason.UNREADABLE, error.message)
    } catch (error: IOException) {
        throw ZipImportException(ZipImportException.Reason.UNREADABLE, error.message)
    } finally {
        staging.deleteRecursively()
    }
}

/** A ZIP entry name that is not UTF-8. */
private class NotUtf8 : IOException()

private fun windowsCharset(): Charset =
    listOf("IBM852", "Cp852", "IBM437").firstOrNull(Charset::isSupported)?.let(Charset::forName) ?: Charsets.ISO_8859_1

/** Unpacks into a fresh [dir]; the number of PDFs written. */
private fun unpackInto(open: () -> InputStream, dir: File, charset: Charset, usableSpace: () -> Long): Int {
    dir.deleteRecursively()
    if (!dir.mkdirs()) throw IOException("cannot create $dir")
    val root = dir.canonicalFile
    val free = usableSpace()
    var written = 0L
    var pdfs = 0
    BufferedInputStream(open()).use { raw ->
        raw.mark(4)
        val head = ByteArray(4)
        val read = raw.readNBytesCompat(head)
        if (read < 4 || !isZipHead(head)) throw ZipImportException(ZipImportException.Reason.NOT_ZIP)
        raw.reset()
        ZipInputStream(raw, charset).use { zip ->
            val buffer = ByteArray(64 * 1024)
            while (true) {
                // The JDK throws on a name that is not UTF-8, while Android decodes it with U+FFFD for the bad bytes.
                val entry = try {
                    zip.nextEntry
                } catch (error: ZipException) {
                    if (charset == Charsets.UTF_8) throw NotUtf8() else throw error
                } catch (error: IllegalArgumentException) {
                    if (charset == Charsets.UTF_8) throw NotUtf8() else throw error
                } ?: break
                if (charset == Charsets.UTF_8 && '\uFFFD' in entry.name) throw NotUtf8()
                // Windows tools may write `\` between folders; on Android that would be part of one file's name.
                val name = entry.name.replace('\\', '/')
                val out = File(root, name).canonicalFile
                if (!out.path.startsWith(root.path + File.separator)) {
                    throw ZipImportException(ZipImportException.Reason.UNREADABLE, "an entry leaves the folder: ${entry.name}")
                }
                if (entry.isDirectory || !keep(name)) continue
                out.parentFile?.mkdirs()
                out.outputStream().use { file ->
                    while (true) {
                        val n = zip.read(buffer)
                        if (n < 0) break
                        written += n
                        if (free - written < SPARE_BYTES) throw ZipImportException(ZipImportException.Reason.NO_SPACE)
                        file.write(buffer, 0, n)
                    }
                }
                if (free - written < SPARE_BYTES) throw ZipImportException(ZipImportException.Reason.NO_SPACE)
                // The ZIP's own time, so a score unchanged in a new ZIP is not reported as updated.
                if (entry.time > 0) out.setLastModified(entry.time)
                if (isPdf(name)) pdfs++
            }
        }
    }
    return pdfs
}

/** True for the first bytes of a ZIP (`PK\u0003\u0004`). */
fun isZipHead(head: ByteArray): Boolean =
    head.size >= 4 && head[0] == 'P'.code.toByte() && head[1] == 'K'.code.toByte() && head[2] == 3.toByte() && head[3] == 4.toByte()

private fun InputStream.readNBytesCompat(into: ByteArray): Int {
    var total = 0
    while (total < into.size) {
        val n = read(into, total, into.size - total)
        if (n < 0) break
        total += n
    }
    return total
}

private fun isPdf(name: String) = name.endsWith(".pdf", ignoreCase = true)

/**
 * PDFs and the sidecar folder; nothing hidden (a segment starting with a dot, `.notae` aside), nothing from macOS and
 * nothing in the root folders a scan skips ([SKIPPED_ROOT_DIRS]).
 */
private fun keep(name: String): Boolean {
    val segments = name.split('/').filter { it.isNotEmpty() }
    if (segments.isEmpty() || segments.first() == "__MACOSX") return false
    val sidecar = segments.first() == SIDECAR_DIR
    if (!sidecar && segments.size > 1 && segments.first() in SKIPPED_ROOT_DIRS) return false
    val hidden = segments.drop(if (sidecar) 1 else 0).any { it.startsWith(".") }
    return !hidden && (sidecar || isPdf(segments.last()))
}

/** A ZIP import in progress, over whatever screen is open. */
sealed interface ZipImport {
    /** A ZIP shared to Notae while a library is open: replace it? */
    data class Confirm(val name: String, val uri: Uri) : ZipImport

    data class Unpacking(val name: String) : ZipImport

    /** The import stopped; the library is as it was. [detail] is the platform's message. */
    data class Failed(val reason: ZipImportException.Reason, val detail: String?) : ZipImport
}

/** The name [uri]'s provider shows for it, or the last part of its path. Blocking: call it on `Dispatchers.IO`. */
fun displayName(resolver: ContentResolver, uri: Uri): String {
    val shown = try {
        resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { if (it.moveToFirst()) it.getString(0) else null }
    } catch (_: Exception) {
        null
    }
    return shown?.takeIf { it.isNotBlank() } ?: uri.lastPathSegment?.substringAfterLast('/')?.takeIf { it.isNotBlank() } ?: "ZIP"
}

/** Whether the file at [uri] starts as a ZIP does. Blocking: call it on `Dispatchers.IO`. */
fun isZip(resolver: ContentResolver, uri: Uri): Boolean = try {
    resolver.openInputStream(uri)?.use { input ->
        val head = ByteArray(4)
        input.readNBytesCompat(head) == 4 && isZipHead(head)
    } ?: false
} catch (_: Exception) {
    false
}
