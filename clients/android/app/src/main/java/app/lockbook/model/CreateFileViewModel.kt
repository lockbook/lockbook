package app.lockbook.model

import androidx.annotation.DrawableRes
import androidx.annotation.StringRes
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.lockbook.R
import app.lockbook.util.SingleMutableLiveData
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbError
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

private fun defaultFileName(): String = SimpleDateFormat("yyyy-MM-dd", Locale.US).format(Date())

enum class NewFileType(
    @param:StringRes val labelRes: Int,
    @param:DrawableRes val iconRes: Int,
    val extension: String?,
    val isDocument: Boolean,
) {
    Note(R.string.note, R.drawable.ic_outline_insert_drive_file_24, ".md", true),
    Drawing(R.string.drawing, R.drawable.ic_outline_draw_24, ".svg", true),
    Folder(R.string.folder, R.drawable.ic_baseline_folder_24, null, false),
    Other(R.string.other, R.drawable.ic_outline_insert_drive_file_24, null, true),
}

internal fun NewFileType.completeName(baseName: String): String = baseName + (extension ?: "")

enum class CreateFilePage {
    Details,
    FolderPicker,
}

enum class CreateLocation {
    FocusedFolder,
    Alongside,
    Custom,
}

data class FolderChoice(
    val file: File,
    val path: String,
    val depth: Int,
    val hasChildren: Boolean,
    val isExpanded: Boolean = false,
)

data class CreateFileUiState(
    val page: CreateFilePage = CreateFilePage.Details,
    val type: NewFileType = NewFileType.Note,
    val name: String = defaultFileName(),
    val location: CreateLocation = CreateLocation.FocusedFolder,
    val parentId: String? = null,
    val focusedFolderId: String? = null,
    val focusedFolderLabel: String? = null,
    val focusedFolderIsRoot: Boolean = false,
    val alongsideParentId: String? = null,
    val alongsideLabel: String? = null,
    val customLabel: String? = null,
    val pickerEntries: List<FolderChoice> = emptyList(),
    val expandedFolderIds: Set<String> = emptySet(),
    val folderQuery: String = "",
    val selectedFolderId: String? = null,
    val isLoading: Boolean = true,
    val isCreating: Boolean = false,
    val error: String? = null,
)

class CreateFileViewModel : ViewModel() {
    private val _state = MutableLiveData(CreateFileUiState())
    val state: LiveData<CreateFileUiState> = _state

    private val _createdFile = SingleMutableLiveData<File>()
    val createdFile: LiveData<File> = _createdFile

    private var initialized = false
    private var lastAutoName = defaultFileName()
    private var nameRequest = 0

    fun initialize(
        initialParentId: String?,
        focusedFolderId: String?,
        alongsideFileId: String?,
    ) {
        if (initialized) return
        initialized = true

        viewModelScope.launch(Dispatchers.IO) {
            try {
                val listedFiles = Lb.listMetadatas().toList()
                val root = listedFiles.firstOrNull { it.isRoot } ?: Lb.getRoot()
                val files = if (listedFiles.any { it.id == root.id }) listedFiles else listedFiles + root
                val byId = files.associateBy { it.id }
                val alongsideFile = alongsideFileId?.let(byId::get)
                val alongsideParent = alongsideFile?.parent?.let(byId::get)
                val focusedFolder =
                    focusedFolderId
                        ?.let(byId::get)
                        ?.takeIf { it.type == File.FileType.Folder }
                        ?: root
                val initialParent =
                    initialParentId
                        ?.let(byId::get)
                        ?.takeIf { it.type == File.FileType.Folder }
                        ?: focusedFolder
                val location =
                    when (initialParent.id) {
                        focusedFolder.id -> CreateLocation.FocusedFolder
                        alongsideParent?.id -> CreateLocation.Alongside
                        else -> CreateLocation.Custom
                    }
                val childCounts = files.filter { !it.isRoot }.groupingBy { it.parent }.eachCount()
                val pickerEntries =
                    files
                        .map {
                            val path = buildPath(it, byId)
                            FolderChoice(
                                file = it,
                                path = path,
                                depth = path.count { character -> character == '/' },
                                hasChildren = childCounts.getOrDefault(it.id, 0) > 0,
                            )
                        }.sortedBy { it.path.lowercase() }
                val expandedFolderIds = ancestorIds(initialParent, byId) + root.id

                withContext(Dispatchers.Main) {
                    _state.value =
                        _state.value!!.copy(
                            location = location,
                            parentId = initialParent.id,
                            focusedFolderId = focusedFolder.id,
                            focusedFolderLabel = if (focusedFolder.isRoot) "Home" else focusedFolder.name,
                            focusedFolderIsRoot = focusedFolder.isRoot,
                            alongsideParentId = alongsideParent?.id,
                            alongsideLabel = alongsideFile?.name,
                            customLabel = initialParent.name.takeIf { location == CreateLocation.Custom },
                            selectedFolderId = initialParent.id,
                            pickerEntries = pickerEntries,
                            expandedFolderIds = expandedFolderIds,
                            isLoading = false,
                        )
                    refreshAutoName()
                }
            } catch (error: LbError) {
                withContext(Dispatchers.Main) {
                    _state.value = _state.value!!.copy(isLoading = false, error = error.msg)
                }
            }
        }
    }

    fun setType(type: NewFileType) {
        if (_state.value?.type == type) return
        _state.value = _state.value!!.copy(type = type, error = null)
        refreshAutoName()
    }

    fun setName(name: String) {
        if (_state.value?.name == name) return
        _state.value = _state.value!!.copy(name = name, error = null)
    }

    fun selectFocusedFolder() =
        selectLocation(
            CreateLocation.FocusedFolder,
            _state.value?.focusedFolderId,
            null,
        )

    fun selectAlongside() =
        selectLocation(
            CreateLocation.Alongside,
            _state.value?.alongsideParentId,
            null,
        )

    fun showFolderPicker() {
        _state.value = _state.value!!.copy(page = CreateFilePage.FolderPicker, folderQuery = "")
    }

    fun hideFolderPicker() {
        _state.value = _state.value!!.copy(page = CreateFilePage.Details, folderQuery = "")
    }

    fun setFolderQuery(query: String) {
        _state.value = _state.value!!.copy(folderQuery = query)
    }

    fun selectFolder(folder: File) {
        if (folder.type != File.FileType.Folder) return
        val state = _state.value ?: return
        val byId = state.pickerEntries.associateBy { it.file.id }
        _state.value =
            state.copy(
                selectedFolderId = folder.id,
                folderQuery = "",
                expandedFolderIds = state.expandedFolderIds + ancestorIds(folder, byId.mapValues { it.value.file }),
            )
    }

    fun toggleFolder(folder: File) {
        if (folder.type != File.FileType.Folder) return
        val expanded = _state.value!!.expandedFolderIds.toMutableSet()
        if (!expanded.remove(folder.id)) expanded.add(folder.id)
        _state.value = _state.value!!.copy(expandedFolderIds = expanded)
    }

    fun confirmFolder() {
        val state = _state.value ?: return
        val folder = state.pickerEntries.firstOrNull { it.file.id == state.selectedFolderId }?.file ?: return
        when (folder.id) {
            state.focusedFolderId -> selectFocusedFolder()
            state.alongsideParentId -> selectAlongside()
            else -> selectLocation(CreateLocation.Custom, folder.id, if (folder.isRoot) "Home" else folder.name)
        }
        hideFolderPicker()
    }

    fun create() {
        val state = _state.value ?: return
        val parentId = state.parentId ?: return
        val baseName = state.name.trim()
        if (baseName.isEmpty()) {
            _state.value = state.copy(error = "Name cannot be empty")
            return
        }

        _state.value = state.copy(isCreating = true, error = null)
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val file =
                    Lb.createFile(
                        state.type.completeName(baseName),
                        parentId,
                        state.type.isDocument,
                    )
                _createdFile.postValue(file)
            } catch (error: LbError) {
                withContext(Dispatchers.Main) {
                    _state.value = _state.value!!.copy(isCreating = false, error = error.msg)
                }
            }
        }
    }

    private fun selectLocation(
        location: CreateLocation,
        parentId: String?,
        customLabel: String?,
    ) {
        parentId ?: return
        _state.value =
            _state.value!!.copy(
                location = location,
                parentId = parentId,
                customLabel = customLabel,
                selectedFolderId = parentId,
                error = null,
            )
        refreshAutoName()
    }

    private fun refreshAutoName() {
        val state = _state.value ?: return
        val parentId = state.parentId ?: return
        if (state.name != lastAutoName) return

        val desired = defaultFileName() + (state.type.extension ?: "")
        val request = ++nameRequest
        viewModelScope.launch(Dispatchers.IO) {
            try {
                var next = Lb.nextName(parentId, desired)
                state.type.extension?.let { extension ->
                    if (next.endsWith(extension)) next = next.dropLast(extension.length)
                }
                withContext(Dispatchers.Main) {
                    if (request == nameRequest && _state.value?.name == lastAutoName) {
                        lastAutoName = next
                        _state.value = _state.value!!.copy(name = next)
                    }
                }
            } catch (_: LbError) {
                // Creation still performs authoritative validation; keep the desired fallback name.
            }
        }
    }

    private fun buildPath(
        file: File,
        byId: Map<String, File>,
    ): String {
        if (file.isRoot) return "Home"
        val names = mutableListOf(file.name)
        var parent = byId[file.parent]
        val visited = mutableSetOf(file.id)
        while (parent != null && !parent.isRoot && visited.add(parent.id)) {
            names += parent.name
            parent = byId[parent.parent]
        }
        return listOf("Home", *names.asReversed().toTypedArray()).joinToString(" / ")
    }

    private fun ancestorIds(
        file: File,
        byId: Map<String, File>,
    ): Set<String> {
        val ancestors = mutableSetOf<String>()
        var parent = byId[file.parent]
        while (parent != null && ancestors.add(parent.id) && !parent.isRoot) {
            parent = byId[parent.parent]
        }
        return ancestors
    }
}
