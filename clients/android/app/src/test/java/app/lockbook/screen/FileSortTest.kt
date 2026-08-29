package app.lockbook.screen

import app.lockbook.util.FileViewHolderInfo
import net.lockbook.File
import org.junit.Assert.assertEquals
import org.junit.Test

class FileSortTest {
    @Test
    fun `alphabetical sorting keeps folders first and applies direction within each type`() {
        val items =
            listOf(
                document("2", "Alpha", 20),
                folder("3", "Zulu", 30),
                folder("1", "Beta", 10),
                document("4", "Charlie", 40),
            )

        val ascending =
            sortFileItems(
                items,
                FileSortOptions(FileSortCriterion.Alphabetical, FileSortDirection.Ascending),
            )
        val descending =
            sortFileItems(
                items,
                FileSortOptions(FileSortCriterion.Alphabetical, FileSortDirection.Descending),
            )

        assertEquals(listOf("Beta", "Zulu", "Alpha", "Charlie"), ascending.names())
        assertEquals(listOf("Zulu", "Beta", "Charlie", "Alpha"), descending.names())
    }

    @Test
    fun `date sorting applies direction within folders and documents`() {
        val items =
            listOf(
                document("2", "New document", 40),
                folder("3", "New folder", 30),
                folder("1", "Old folder", 10),
                document("4", "Old document", 20),
            )

        val ascending =
            sortFileItems(
                items,
                FileSortOptions(FileSortCriterion.LastModified, FileSortDirection.Ascending),
            )
        val descending =
            sortFileItems(
                items,
                FileSortOptions(FileSortCriterion.LastModified, FileSortDirection.Descending),
            )

        assertEquals(listOf("Old folder", "New folder", "Old document", "New document"), ascending.names())
        assertEquals(listOf("New folder", "Old folder", "New document", "Old document"), descending.names())
    }

    private fun folder(
        id: String,
        name: String,
        modified: Long,
    ): FileViewHolderInfo =
        FileViewHolderInfo.FolderViewHolderInfo(
            file(id, name, File.FileType.Folder, modified),
            false,
            false,
            false,
        )

    private fun document(
        id: String,
        name: String,
        modified: Long,
    ): FileViewHolderInfo =
        FileViewHolderInfo.DocumentViewHolderInfo(
            file(id, name, File.FileType.Document, modified),
            false,
            false,
            false,
        )

    private fun file(
        id: String,
        name: String,
        type: File.FileType,
        modified: Long,
    ): File =
        File().apply {
            this.id = id
            parent = "root"
            this.name = name
            this.type = type
            lastModified = modified
            lastModifiedBy = ""
            shares = emptyArray()
        }

    private fun List<FileViewHolderInfo>.names(): List<String> = map { it.fileMetadata.name }
}
