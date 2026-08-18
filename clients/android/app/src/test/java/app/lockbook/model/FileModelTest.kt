package app.lockbook.model

import net.lockbook.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class FileModelTest {
    @Test
    fun `owned parent is returned when ancestry reaches user root`() {
        val root = folder("root", "root")
        val projects = folder("projects", root.id)
        val notes = folder("notes", projects.id)
        val document = document("document", notes.id)
        val model = model(root, projects, notes, document)

        assertEquals(notes.id, model.ownedParentOf(document.id)?.id)
    }

    @Test
    fun `shared parent is rejected when ancestry reaches a different root`() {
        val root = folder("root", "root")
        val sharedRoot = folder("shared-root", "shared-root")
        val sharedDocument = document("shared-document", sharedRoot.id)
        val model = model(root, sharedRoot, sharedDocument)

        assertNull(model.ownedParentOf(sharedDocument.id))
    }

    @Test
    fun `parent is rejected when ancestry is incomplete`() {
        val root = folder("root", "root")
        val detachedParent = folder("detached", "missing-parent")
        val detachedDocument = document("detached-document", detachedParent.id)
        val model = model(root, detachedParent, detachedDocument)

        assertNull(model.ownedParentOf(detachedDocument.id))
    }

    @Test
    fun `parent is rejected when ancestry contains a cycle`() {
        val root = folder("root", "root")
        val first = folder("first", "second")
        val second = folder("second", first.id)
        val cyclicDocument = document("cyclic-document", first.id)
        val model = model(root, first, second, cyclicDocument)

        assertNull(model.ownedParentOf(cyclicDocument.id))
    }

    private fun model(
        root: File,
        vararg files: File,
    ): FileModel {
        val allFiles = listOf(root) + files
        return FileModel(
            root = root,
            parent = root,
            idsAndFiles = allFiles.associateBy { it.id },
            children = files.filter { it.parent == root.id },
        )
    }

    private fun folder(
        id: String,
        parentId: String,
    ): File = file(id, parentId, File.FileType.Folder)

    private fun document(
        id: String,
        parentId: String,
    ): File = file(id, parentId, File.FileType.Document)

    private fun file(
        id: String,
        parentId: String,
        type: File.FileType,
    ): File =
        File().apply {
            this.id = id
            parent = parentId
            name = id
            this.type = type
            lastModified = 0
            lastModifiedBy = ""
            shares = emptyArray()
        }
}
