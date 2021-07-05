package app.lockbook.model

import android.app.Application
import android.content.Context
import android.content.res.Resources
import androidx.annotation.StringRes
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import androidx.preference.PreferenceManager
import app.lockbook.R
import app.lockbook.ui.BreadCrumbItem
import app.lockbook.ui.CreateFileInfo
import app.lockbook.ui.MoveFileInfo
import app.lockbook.ui.RenameFileInfo
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.map
import kotlinx.coroutines.*
import timber.log.Timber
import java.io.File

data class EditableFile(
    val name: String,
    val id: String,
)

class ListFilesViewModel(application: Application, isThisAnImport: Boolean) :
    AndroidViewModel(application),
    ListFilesClickInterface {
    var selectedFiles = listOf<ClientFileMetadata>()
    var isFABOpen = false

    private val _stopProgressSpinner = SingleMutableLiveData<Unit>()
    private val _files = MutableLiveData<List<ClientFileMetadata>>()
    private val _updateBreadcrumbBar = MutableLiveData<List<BreadCrumbItem>>()
    private val _navigateToFileEditor = SingleMutableLiveData<EditableFile>()
    private val _navigateToDrawing = SingleMutableLiveData<EditableFile>()
    private val _switchFileLayout = SingleMutableLiveData<Unit>()
    private val _expandCloseMenu = SingleMutableLiveData<Boolean>()
    private val _collapseExpandFAB = SingleMutableLiveData<Boolean>()
    private val _showCreateFileDialog = SingleMutableLiveData<CreateFileInfo>()
    private val _showMoveFileDialog = SingleMutableLiveData<MoveFileInfo>()
    private val _showFileInfoDialog = SingleMutableLiveData<ClientFileMetadata>()
    private val _showRenameFileDialog = SingleMutableLiveData<RenameFileInfo>()
    private val _uncheckAllFiles = SingleMutableLiveData<Unit>()
    private val _shareDocument = SingleMutableLiveData<ArrayList<File>>()
    private val _notifyWithSnackbar = SingleMutableLiveData<String>()
    private val _showSyncSnackBar = SingleMutableLiveData<Unit>()
    private val _updateSyncSnackBar = SingleMutableLiveData<Pair<Int, Int>>()
    private val _showHideProgressOverlay = SingleMutableLiveData<Boolean>()
    private val _notifyError = SingleMutableLiveData<LbError>()

    val stopProgressSpinner: LiveData<Unit>
        get() = _stopProgressSpinner

    val files: LiveData<List<ClientFileMetadata>>
        get() = _files

    val showSyncSnackBar: LiveData<Unit>
        get() = _showSyncSnackBar

    val updateSyncSnackBar: LiveData<Pair<Int, Int>>
        get() = _updateSyncSnackBar

    val navigateToFileEditor: LiveData<EditableFile>
        get() = _navigateToFileEditor

    val navigateToDrawing: LiveData<EditableFile>
        get() = _navigateToDrawing

    val switchFileLayout: LiveData<Unit>
        get() = _switchFileLayout

    val expandCloseMenu: LiveData<Boolean>
        get() = _expandCloseMenu

    val collapseExpandFAB: LiveData<Boolean>
        get() = _collapseExpandFAB

    val showCreateFileDialog: LiveData<CreateFileInfo>
        get() = _showCreateFileDialog

    val showMoveFileDialog: LiveData<MoveFileInfo>
        get() = _showMoveFileDialog

    val showFileInfoDialog: LiveData<ClientFileMetadata>
        get() = _showFileInfoDialog

    val showRenameFileDialog: LiveData<RenameFileInfo>
        get() = _showRenameFileDialog

    val uncheckAllFiles: LiveData<Unit>
        get() = _uncheckAllFiles

    val updateBreadcrumbBar: LiveData<List<BreadCrumbItem>>
        get() = _updateBreadcrumbBar

    val shareDocument: LiveData<ArrayList<File>>
        get() = _shareDocument

    val notifyWithSnackbar: LiveData<String>
        get() = _notifyWithSnackbar

    val showHideProgressOverlay: LiveData<Boolean>
        get() = _showHideProgressOverlay

    val notifyError: LiveData<LbError>
        get() = _notifyError

    lateinit var fileModel: FileModel

    val shareModel = ShareModel(
        _shareDocument,
        _showHideProgressOverlay,
        _notifyError,
    )
    val syncModel = SyncModel(
        _showSyncSnackBar,
        _updateSyncSnackBar,
        _notifyWithSnackbar,
        _notifyError,
    )

    init {
        viewModelScope.launch(Dispatchers.IO) {
            if (isThisAnImport) {
                syncModel.trySync(getContext())
            }
            startUpInRoot()
        }
    }

    private fun startUpInRoot() {
        when (val createAtRootResult = FileModel.createAtRoot(getContext())) {
            is Ok -> {
                fileModel = createAtRootResult.value
                refreshFiles()
                _updateBreadcrumbBar.postValue(fileModel.fileDir.map { BreadCrumbItem(it.name) })
            }
            is Err -> _notifyError.postValue(createAtRootResult.error)
        }
    }

    fun onBackPress(): Boolean = when {
        shareModel.isLoadingOverlayVisible -> {
            true
        }
        selectedFiles.isNotEmpty() -> {
            collapseMoreOptionsMenu()
            true
        }
        !fileModel.isAtRoot() -> {
            val intoParentResult = fileModel.intoParent()
            if (intoParentResult is Err) {
                _notifyError.postValue(intoParentResult.error.toLbError(getRes()))
            }
            _files.postValue(fileModel.children)
            _updateBreadcrumbBar.postValue(fileModel.fileDir.map { BreadCrumbItem(it.name) })
            true
        }
        else -> false
    }

    fun onOpenedActivityEnd() {
        viewModelScope.launch(Dispatchers.IO) {
            syncModel.syncBasedOnPreferences(getContext())
        }
    }

    fun onSwipeToRefresh() {
        viewModelScope.launch(Dispatchers.IO) {
            syncModel.trySync(getContext())
            refreshFiles()
            _stopProgressSpinner.postValue(Unit)
        }
    }

    fun onNewDocumentFABClicked(isDrawing: Boolean) {
        viewModelScope.launch(Dispatchers.IO) {
            isFABOpen = !isFABOpen
            _collapseExpandFAB.postValue(false)
            _showCreateFileDialog.postValue(
                CreateFileInfo(
                    fileModel.parent.id,
                    FileType.Document.name,
                    isDrawing
                )
            )
        }
    }

    fun onNewFolderFABClicked() {
        viewModelScope.launch(Dispatchers.IO) {
            isFABOpen = !isFABOpen
            _collapseExpandFAB.postValue(false)
            _showCreateFileDialog.postValue(
                CreateFileInfo(
                    fileModel.parent.id,
                    FileType.Folder.name,
                    false
                )
            )
        }
    }

    private fun refreshFiles() {
        val refreshChildrenResult = fileModel.refreshChildren()
        if (refreshChildrenResult is Err) {
            return _notifyError.postValue(refreshChildrenResult.error.toLbError(getRes()))
        }
        collapseMoreOptionsMenu()
        _files.postValue(fileModel.children)
    }

    fun onMenuItemPressed(id: Int) {
        viewModelScope.launch(Dispatchers.IO) {
            val pref = PreferenceManager.getDefaultSharedPreferences(getApplication()).edit()
            when (id) {
                R.id.menu_list_files_sort_last_changed -> {
                    pref.putString(
                        getString(R.string.sort_files_key),
                        getString(R.string.sort_files_last_changed_value)
                    ).apply()
                    fileModel.setSortStyle(SortStyle.LastChanged)
                    _files.postValue(fileModel.children)
                }
                R.id.menu_list_files_sort_a_z -> {
                    pref.putString(
                        getString(R.string.sort_files_key),
                        getString(R.string.sort_files_a_z_value)
                    ).apply()
                    fileModel.setSortStyle(SortStyle.AToZ)
                    _files.postValue(fileModel.children)
                }
                R.id.menu_list_files_sort_z_a -> {
                    pref.putString(
                        getString(R.string.sort_files_key),
                        getString(R.string.sort_files_z_a_value)
                    )
                        .apply()
                    fileModel.setSortStyle(SortStyle.ZToA)
                    _files.postValue(fileModel.children)
                }
                R.id.menu_list_files_sort_first_changed -> {
                    pref.putString(
                        getString(R.string.sort_files_key),
                        getString(R.string.sort_files_first_changed_value)
                    ).apply()
                    fileModel.setSortStyle(SortStyle.FirstChanged)
                    _files.postValue(fileModel.children)
                }
                R.id.menu_list_files_sort_type -> {
                    pref.putString(
                        getString(R.string.sort_files_key),
                        getString(R.string.sort_files_type_value)
                    ).apply()
                    fileModel.setSortStyle(SortStyle.FileType)
                    _files.postValue(fileModel.children)
                }
                R.id.menu_list_files_linear_view -> {
                    pref.putString(
                        getString(R.string.file_layout_key),
                        getString(R.string.file_layout_linear_value)
                    ).apply()
                    _switchFileLayout.postValue(Unit)
                }
                R.id.menu_list_files_grid_view -> {
                    pref.putString(
                        getString(R.string.file_layout_key),
                        getString(R.string.file_layout_grid_value)
                    ).apply()
                    _switchFileLayout.postValue(Unit)
                }
                R.id.menu_list_files_rename -> {
                    if (selectedFiles.size == 1) {
                        _showRenameFileDialog.postValue(
                            RenameFileInfo(
                                selectedFiles[0].id,
                                selectedFiles[0].name
                            )
                        )
                    } else {
                        postBasicError()
                    }
                }
                R.id.menu_list_files_delete -> {
                    when (val deleteFilesResult = FileModel.deleteFiles(selectedFiles.map { it.id })) {
                        is Ok -> _notifyWithSnackbar.postValue("Successfully deleted the file(s)")
                        is Err -> _notifyError.postValue(deleteFilesResult.error.toLbError(getRes()))
                    }

                    collapseMoreOptionsMenu()
                    refreshFiles()
                }
                R.id.menu_list_files_info -> {
                    if (selectedFiles.size == 1) {
                        _showFileInfoDialog.postValue(selectedFiles[0])
                    } else {
                        postBasicError()
                    }
                }
                R.id.menu_list_files_move -> {
                    _showMoveFileDialog.postValue(
                        MoveFileInfo(
                            selectedFiles
                                .map { fileMetadata -> fileMetadata.id }.toTypedArray(),
                            selectedFiles
                                .map { fileMetadata -> fileMetadata.name }.toTypedArray()
                        )
                    )
                }
                R.id.menu_list_files_share -> {
                    shareModel.shareDocuments(getContext(), selectedFiles)
                }
                else -> {
                    Timber.e("Unrecognized sort item id.")
                    postBasicError()
                }
            }
        }
    }

    override fun onItemClick(position: Int, newSelectedFiles: List<ClientFileMetadata>) {
        viewModelScope.launch(Dispatchers.IO) {
            val oldSelectedFiles = selectedFiles.toList()

            if (newSelectedFiles != selectedFiles) {
                if (newSelectedFiles.isEmpty()) {
                    _expandCloseMenu.postValue(false)
                } else if (newSelectedFiles.isNotEmpty() && selectedFiles.isEmpty() || newSelectedFiles.size > 1 && selectedFiles.size == 1 || selectedFiles.size > 1 && newSelectedFiles.size == 1) {
                    _expandCloseMenu.postValue(true)
                }
                selectedFiles = newSelectedFiles.toList()
            }

            if (selectedFiles.isEmpty() && oldSelectedFiles.isEmpty()) {
                val file = fileModel.children[position]

                if (file.fileType == FileType.Folder) {
                    val intoChildResult = fileModel.intoChild(file)
                    if (intoChildResult is Err) {
                        return@launch _notifyError.postValue(intoChildResult.error.toLbError(getRes()))
                    }
                    _files.postValue(fileModel.children)
                    _updateBreadcrumbBar.postValue(fileModel.fileDir.map { BreadCrumbItem(it.name) })
                } else {
                    enterDocument(file)
                }
            }
        }
    }

    override fun onLongClick(position: Int, newSelectedFiles: List<ClientFileMetadata>) {
        viewModelScope.launch(Dispatchers.IO) {
            if (newSelectedFiles != selectedFiles) {
                if (newSelectedFiles.isEmpty()) {
                    _expandCloseMenu.postValue(false)
                } else if (newSelectedFiles.isNotEmpty() && selectedFiles.isEmpty()) {
                    _expandCloseMenu.postValue(true)
                }
                selectedFiles = newSelectedFiles.toList()
            }
        }
    }

    fun onCreateFileDialogEnded(newDocument: ClientFileMetadata?) {
        viewModelScope.launch(Dispatchers.IO) {
            refreshFiles()

            if (newDocument != null && PreferenceManager.getDefaultSharedPreferences(getApplication())
                .getBoolean(getString(R.string.open_new_doc_automatically_key), true)
            ) {
                enterDocument(newDocument)
            }
        }
    }

    fun refreshAtPastParent(position: Int) {
        viewModelScope.launch(Dispatchers.IO) {
            val refreshChildrenResult = fileModel.refreshChildrenAtPastParent(position)
            if (refreshChildrenResult is Err) {
                return@launch _notifyError.postValue(refreshChildrenResult.error.toLbError(getRes()))
            }
            collapseMoreOptionsMenu()
            _files.postValue(fileModel.children)
            _updateBreadcrumbBar.postValue(fileModel.fileDir.map { BreadCrumbItem(it.name) })
        }
    }

    fun collapseExpandFAB() {
        viewModelScope.launch(Dispatchers.IO) {
            isFABOpen = !isFABOpen
            _collapseExpandFAB.postValue(isFABOpen)
        }
    }

    fun collapseMoreOptionsMenu() {
        selectedFiles = listOf()
        _expandCloseMenu.postValue(false)
        _uncheckAllFiles.postValue(Unit)
    }

    private fun postBasicError() {
        _notifyError.postValue(LbError.basicError(getRes()))
    }

    private fun enterDocument(fileMetadata: ClientFileMetadata) {
        val editableFileResult =
            EditableFile(fileMetadata.name, fileMetadata.id)
        if (fileMetadata.name.endsWith(".draw")) {
            _navigateToDrawing.postValue(editableFileResult)
        } else {
            _navigateToFileEditor.postValue(editableFileResult)
        }
    }
}

fun AndroidViewModel.getContext(): Context {
    return this.getApplication<Application>()
}

fun AndroidViewModel.getRes(): Resources {
    return this.getApplication<Application>().resources
}

fun AndroidViewModel.getString(
    @StringRes stringRes: Int,
    vararg formatArgs: Any = emptyArray()
): String {
    return getString(this.getRes(), stringRes, *formatArgs)
}
