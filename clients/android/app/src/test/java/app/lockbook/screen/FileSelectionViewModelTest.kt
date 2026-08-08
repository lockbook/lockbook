package app.lockbook.screen

import net.lockbook.File
import org.junit.Assert.assertEquals
import org.junit.Test

class FileSelectionViewModelTest {
    private val model = FileSelectionViewModel()

    @Test
    fun `toggle produces one selection state and clears the last selection`() {
        val first = file("first")
        val second = file("second")

        model.toggle(FileSelectionSource.Files, first)
        model.toggle(FileSelectionSource.Files, second)

        assertEquals(setOf("first", "second"), model.uiState.value.selectedIds)
        assertEquals(FileSelectionSource.Files, model.uiState.value.source)

        model.toggle(FileSelectionSource.Files, first)
        model.toggle(FileSelectionSource.Files, second)

        assertEquals(FileSelectionUiState(), model.uiState.value)
    }

    @Test
    fun `events from a new source replace the previous source selection`() {
        model.toggle(FileSelectionSource.Files, file("home"))
        model.toggle(FileSelectionSource.Recents, file("recent"))

        assertEquals(setOf("recent"), model.uiState.value.selectedIds)
        assertEquals(FileSelectionSource.Recents, model.uiState.value.source)
    }

    @Test
    fun `only the owning source can reconcile selection`() {
        model.toggle(FileSelectionSource.Files, file("kept"))

        model.reconcile(FileSelectionSource.Recents, emptyList())
        assertEquals(setOf("kept"), model.uiState.value.selectedIds)

        model.reconcile(FileSelectionSource.Files, emptyList())
        assertEquals(FileSelectionUiState(), model.uiState.value)
    }

    private fun file(id: String): File =
        File().apply {
            this.id = id
            parent = "root"
            name = id
            type = File.FileType.Document
            lastModified = 0
            lastModifiedBy = ""
            shares = emptyArray()
        }
}
