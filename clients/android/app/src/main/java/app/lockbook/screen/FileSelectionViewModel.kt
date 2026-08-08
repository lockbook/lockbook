package app.lockbook.screen

import androidx.lifecycle.ViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import net.lockbook.File

internal enum class FileSelectionSource {
    Files,
    Recents,
}

internal data class FileSelectionUiState(
    val source: FileSelectionSource? = null,
    val selectedFiles: List<File> = emptyList(),
) {
    val selectedCount: Int = selectedFiles.size
    val isActive: Boolean = selectedFiles.isNotEmpty()
    val selectedIds: Set<String> = selectedFiles.mapTo(mutableSetOf()) { file -> file.id }
    val visibleActions: Set<FileSelectionAction> =
        FileSelectionAction.entries
            .filterTo(mutableSetOf()) { action -> action.isAvailableFor(selectedFiles) }

    fun selectedIdsFor(selectionSource: FileSelectionSource): Set<String> = selectedIds.takeIf { source == selectionSource }.orEmpty()
}

internal class FileSelectionViewModel : ViewModel() {
    private val _uiState = MutableStateFlow(FileSelectionUiState())
    val uiState = _uiState.asStateFlow()

    fun toggle(
        source: FileSelectionSource,
        file: File,
    ) {
        _uiState.update { state ->
            val currentFiles = state.selectedFiles.takeIf { state.source == source }.orEmpty()
            val updatedFiles =
                if (file.id in currentFiles.map { selected -> selected.id }) {
                    currentFiles.filterNot { selected -> selected.id == file.id }
                } else {
                    currentFiles + file
                }
            FileSelectionUiState(
                source = source.takeIf { updatedFiles.isNotEmpty() },
                selectedFiles = updatedFiles,
            )
        }
    }

    fun reconcile(
        source: FileSelectionSource,
        availableFiles: List<File>,
    ) {
        _uiState.update { state ->
            if (state.source != source) return@update state
            val selectedIds = state.selectedIds
            val selectedFiles = availableFiles.filter { file -> file.id in selectedIds }
            FileSelectionUiState(
                source = source.takeIf { selectedFiles.isNotEmpty() },
                selectedFiles = selectedFiles,
            )
        }
    }

    fun clear() {
        _uiState.value = FileSelectionUiState()
    }
}
