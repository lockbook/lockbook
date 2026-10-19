package app.lockbook.model

import net.lockbook.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class WorkspaceDocumentsTest {
    @Test
    fun `pins keep their order and do not appear again in recents`() {
        val documents = listOf(document("a", 100), document("b", 300), document("c", 200))
        val items = workspaceDocumentItems(listOf(PinnedFile("a"), PinnedFile("c")), documents)

        assertEquals(listOf("a", "c", "b"), items.filterIsInstance<WorkspaceDocumentItem.Document>().map { it.file.id })
        assertEquals(3, items.size)
    }

    @Test
    fun `recents are newest first and limited without limiting pins`() {
        val documents = (1..20).map { document(it.toString(), it.toLong()) }
        val items = workspaceDocumentItems(listOf(PinnedFile("1")), documents)

        assertEquals(
            listOf("1") + (20 downTo 9).map(Int::toString),
            items.filterIsInstance<WorkspaceDocumentItem.Document>().map { it.file.id },
        )
    }

    @Test
    fun `new accounts and folder-only accounts see suggestions rather than fake documents`() {
        val folder = document("folder", 1).apply { type = File.FileType.Folder }
        for (documents in listOf(emptyList(), listOf(folder))) {
            val items = workspaceDocumentItems(listOf(PinnedFile("missing"), PinnedFile("folder")), documents)
            assertEquals(StarterDocument.entries, items.filterIsInstance<WorkspaceDocumentItem.Starter>().map { it.template })
            assertTrue(items.none { it is WorkspaceDocumentItem.Document })
        }
    }

    @Test
    fun `real documents replace starter suggestions`() {
        val items = workspaceDocumentItems(emptyList(), listOf(document("a", 1)))
        assertTrue(items.none { it is WorkspaceDocumentItem.Starter })
    }

    @Test
    fun `document presentation captures names before mutable metadata changes`() {
        val file = document("a", 1)
        val item = workspaceDocumentItems(emptyList(), listOf(file)).filterIsInstance<WorkspaceDocumentItem.Document>().single()
        file.name = "renamed.md"
        assertEquals("a.md", item.name)
    }

    @Test
    fun `only markdown documents appear in pins and recents`() {
        val documents =
            listOf(
                document("note", 1).apply { name = "NOTE.MD" },
                document("drawing", 2).apply { name = "drawing.SVG" },
                document("image", 5).apply { name = "note.md.png" },
                document("pdf", 4).apply { name = "document.pdf" },
                document("text", 3).apply { name = "text.txt" },
            )
        val items = workspaceDocumentItems(listOf(PinnedFile("drawing"), PinnedFile("image"), PinnedFile("note")), documents)
        assertEquals(listOf("note"), items.filterIsInstance<WorkspaceDocumentItem.Document>().map { it.file.id })
    }

    @Test
    fun `accounts with only viewable files see starter cards`() {
        val image = document("image", 1).apply { name = "photo.png" }
        val drawing = document("drawing", 2).apply { name = "drawing.svg" }
        val items = workspaceDocumentItems(listOf(PinnedFile("image"), PinnedFile("drawing")), listOf(image, drawing))
        assertEquals(StarterDocument.entries, items.filterIsInstance<WorkspaceDocumentItem.Starter>().map { it.template })
    }

    @Test
    fun `view-only documents do not consume the recent card limit`() {
        val images = (1..20).map { document("image$it", 100).apply { name = "$it.png" } }
        val notes = (1..12).map { document("note$it", it.toLong()) }
        val items = workspaceDocumentItems(emptyList(), images + notes)
        assertEquals(12, items.filterIsInstance<WorkspaceDocumentItem.Document>().size)
    }

    private fun document(
        id: String,
        modified: Long,
    ): File =
        File().apply {
            this.id = id
            parent = "root"
            name = "$id.md"
            type = File.FileType.Document
            lastModified = modified
            lastModifiedBy = ""
            shares = emptyArray()
        }
}
