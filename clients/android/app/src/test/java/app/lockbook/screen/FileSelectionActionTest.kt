package app.lockbook.screen

import net.lockbook.File
import org.junit.Assert.assertEquals
import org.junit.Test

class FileSelectionActionTest {
    @Test
    fun `selection bar state derives visibility count and actions`() {
        val hidden = FileSelectionUiState()
        val selected =
            FileSelectionUiState(
                source = FileSelectionSource.Files,
                selectedFiles = listOf(file(File.FileType.Document), file(File.FileType.Document)),
            )

        assertEquals(false, hidden.isActive)
        assertEquals(0, hidden.selectedCount)
        assertEquals(true, selected.isActive)
        assertEquals(2, selected.selectedCount)
        assertEquals(
            setOf(
                FileSelectionAction.Move,
                FileSelectionAction.Pin,
                FileSelectionAction.Export,
                FileSelectionAction.Delete,
            ),
            selected.visibleActions,
        )
    }

    @Test
    fun `single document exposes every file action`() {
        val available = FileSelectionAction.entries.filter { it.isAvailableFor(listOf(file(File.FileType.Document))) }

        assertEquals(FileSelectionAction.entries, available)
    }

    @Test
    fun `multiple documents expose only multi-selection actions`() {
        val files = listOf(file(File.FileType.Document), file(File.FileType.Document))
        val available = FileSelectionAction.entries.filter { it.isAvailableFor(files) }

        assertEquals(
            listOf(
                FileSelectionAction.Move,
                FileSelectionAction.Pin,
                FileSelectionAction.Export,
                FileSelectionAction.Delete,
            ),
            available,
        )
    }

    @Test
    fun `folders cannot be duplicated`() {
        val available = FileSelectionAction.entries.filter { it.isAvailableFor(listOf(file(File.FileType.Folder))) }

        assertEquals(false, FileSelectionAction.Duplicate in available)
    }

    private fun file(type: File.FileType): File =
        File().apply {
            id = type.name
            parent = "root"
            name = type.name
            this.type = type
            lastModified = 0
            lastModifiedBy = ""
            shares = emptyArray()
        }
}
