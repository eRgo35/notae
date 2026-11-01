package com.c2yz.notae.platform

import android.Manifest
import android.content.pm.ProviderInfo
import android.database.Cursor
import android.database.MatrixCursor
import android.os.CancellationSignal
import android.os.ParcelFileDescriptor
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import android.provider.DocumentsProvider
import com.c2yz.notae.core.DocumentContent
import com.c2yz.notae.core.DocumentEntry
import com.c2yz.notae.core.PlatformException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import java.io.File
import java.io.FileNotFoundException

@RunWith(RobolectricTestRunner::class)
class SafLibraryTest {
    @get:Rule
    val files = TemporaryFolder()

    private lateinit var provider: FakeDocumentsProvider
    private val tree = DocumentsContract.buildTreeDocumentUri(AUTHORITY, "root").toString()
    private val library = SafLibrary(RuntimeEnvironment.getApplication())

    @Before
    fun registerProvider() {
        val info = ProviderInfo().apply {
            authority = AUTHORITY
            exported = true
            grantUriPermissions = true
            readPermission = Manifest.permission.MANAGE_DOCUMENTS
            writePermission = Manifest.permission.MANAGE_DOCUMENTS
        }
        provider = Robolectric.buildContentProvider(FakeDocumentsProvider::class.java).create(info).get()
    }

    @Test
    fun enumerate_reports_unreadable_document() {
        val root = "%PDF-1.4\nroot".toByteArray()
        val nested = "%PDF-1.7\nnested".toByteArray()
        provider.add("root", FakeDocument("root.pdf", file(root)))
        provider.add("root", FakeDocument("nested", directory = true))
        provider.add("nested", FakeDocument("practice.PDF", file(nested)))
        provider.add("root", FakeDocument("locked.pdf", null))
        provider.add("root", FakeDocument("notes.txt", file("text".toByteArray())))

        val entries = library.enumerate(tree)

        assertEquals(
            listOf(
                DocumentEntry("root.pdf", "root.pdf", root.size.toULong(), sha256(root)),
                DocumentEntry("nested/practice.PDF", "practice.PDF", nested.size.toULong(), sha256(nested)),
                DocumentEntry("locked.pdf", "locked.pdf", 0uL, DocumentContent.Unreadable),
            ),
            entries,
        )
    }

    @Test
    fun revoked_tree_maps_to_permission_lost() {
        provider.add("root", FakeDocument("root.pdf", file("%PDF-1.4\nroot".toByteArray())))
        provider.revoked = true

        assertThrows(PlatformException.PermissionLost::class.java) { library.enumerate(tree) }
        assertThrows(PlatformException.PermissionLost::class.java) { library.checkRoot(tree) }
        assertThrows(PlatformException.PermissionLost::class.java) { library.extractTextLayer(tree, "root.pdf") }
    }

    @Test
    fun missing_root_maps_to_permission_lost() {
        for (missing in listOf(IllegalArgumentException("Failed to determine if root is child"), FileNotFoundException("root"))) {
            provider.rootFailure = missing

            assertThrows(PlatformException.PermissionLost::class.java) { library.checkRoot(tree) }
        }
    }

    @Test
    fun a_throwable_that_is_not_an_exception_becomes_unavailable() {
        val error = assertThrows(PlatformException.Unavailable::class.java) {
            platformCall<Unit> { throw OutOfMemoryError("bitmap too large") }
        }

        assertEquals("bitmap too large", error.detail)
    }

    private fun file(bytes: ByteArray): File = files.newFile().apply { writeBytes(bytes) }

    private fun sha256(bytes: ByteArray) = DocumentContent.Sha256(sha256Hex(bytes.inputStream()))

    private companion object {
        const val AUTHORITY = "com.c2yz.notae.test.documents"
    }
}

/** A document the fake provider lists; a null [file] throws on open. */
data class FakeDocument(val name: String, val file: File? = null, val directory: Boolean = false)

/** A document tree in memory; [revoked] makes every call fail as a revoked grant does. */
class FakeDocumentsProvider : DocumentsProvider() {
    private val children = mutableMapOf<String, MutableList<Pair<String, FakeDocument>>>()
    var revoked = false

    /** Thrown when the root itself is queried, as a provider does for a deleted or moved tree. */
    var rootFailure: Exception? = null

    fun add(parent: String, document: FakeDocument) {
        children.getOrPut(parent) { mutableListOf() } += document.name to document
    }

    override fun onCreate() = true

    override fun queryRoots(projection: Array<out String>?): Cursor = MatrixCursor(arrayOf("root_id"))

    override fun queryDocument(documentId: String, projection: Array<out String>?): Cursor {
        checkGrant()
        rootFailure?.takeIf { documentId == "root" }?.let { throw it }
        val cursor = cursor()
        if (documentId == "root") {
            cursor.addRow(arrayOf<Any?>("root", "Scores", Document.MIME_TYPE_DIR, 0))
        } else {
            find(documentId)?.let { cursor.addRow(row(documentId, it)) }
        }
        return cursor
    }

    override fun queryChildDocuments(parentDocumentId: String, projection: Array<out String>?, sortOrder: String?): Cursor {
        checkGrant()
        val cursor = cursor()
        children[parentDocumentId].orEmpty().forEach { (id, document) -> cursor.addRow(row(id, document)) }
        return cursor
    }

    override fun openDocument(documentId: String, mode: String, signal: CancellationSignal?): ParcelFileDescriptor {
        checkGrant()
        val file = find(documentId)?.file ?: throw FileNotFoundException("cannot open $documentId")
        return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
    }

    override fun isChildDocument(parentDocumentId: String, documentId: String) = true

    private fun checkGrant() {
        if (revoked) throw SecurityException("Permission Denial: the tree grant was revoked")
    }

    private fun find(id: String) = children.values.flatten().firstOrNull { it.first == id }?.second

    private fun cursor() = MatrixCursor(
        arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME, Document.COLUMN_MIME_TYPE, Document.COLUMN_SIZE),
    )

    private fun row(id: String, document: FakeDocument): Array<Any?> = arrayOf(
        id,
        document.name,
        if (document.directory) Document.MIME_TYPE_DIR else if (document.name.endsWith(".txt")) "text/plain" else "application/pdf",
        document.file?.length() ?: 0,
    )
}
