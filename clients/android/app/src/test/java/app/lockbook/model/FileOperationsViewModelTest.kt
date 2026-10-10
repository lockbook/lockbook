package app.lockbook.model

import androidx.lifecycle.SavedStateHandle
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class FileOperationsViewModelTest {
    @Test
    fun `restored picker waits for its result instead of opening another picker`() {
        val state = SavedStateHandle(mapOf("pending_export_files" to arrayListOf("/cache/share/note.md")))
        val model = FileOperationsViewModel(state)
        assertEquals(ExportState.AwaitingDestination, model.exporting.value)
        assertNull(model.takeExportPickerRequest())

        // A second export cannot replace files belonging to the outstanding picker.
        model.prepareExport(emptyList(), java.io.File("/unused"))
        assertEquals(arrayListOf("/cache/share/note.md"), state.get<ArrayList<String>>("pending_export_files"))
        model.finishExport()
        assertEquals(ExportState.Idle, model.exporting.value)
        assertEquals(ExportState.Idle, FileOperationsViewModel(state).exporting.value)
    }

    @Test
    fun `interrupted work reports failure rather than repeating a partially completed copy`() {
        val state = SavedStateHandle(mapOf("export_in_progress" to true))
        val model = FileOperationsViewModel(state)
        assertEquals(ExportState.Failed(), model.exporting.value)
        model.finishExport()
        assertEquals(ExportState.Idle, FileOperationsViewModel(state).exporting.value)
    }

    @Test
    fun `unavailable picker clears its pending files and retains failure until acknowledged`() {
        val state = SavedStateHandle(mapOf("pending_export_files" to arrayListOf("/cache/share/note.md")))
        val model = FileOperationsViewModel(state)
        model.exportPickerFailed()
        assertEquals(ExportState.Failed(), model.exporting.value)
        assertEquals(emptyList<String>(), state.get<ArrayList<String>>("pending_export_files"))
        model.finishExport()
        assertEquals(ExportState.Idle, model.exporting.value)
    }
}
