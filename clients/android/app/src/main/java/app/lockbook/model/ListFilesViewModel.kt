package app.lockbook.model

import android.app.Application
import android.content.SharedPreferences.OnSharedPreferenceChangeListener
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.viewModelScope
import androidx.preference.PreferenceManager
import androidx.work.WorkManager
import app.lockbook.App
import app.lockbook.App.Companion.PERIODIC_SYNC_TAG
import app.lockbook.R
import app.lockbook.screen.ListFilesFragment
import app.lockbook.ui.BreadCrumb
import app.lockbook.ui.CreateFileInfo
import app.lockbook.ui.MoveFileInfo
import app.lockbook.ui.RenameFileInfo
import app.lockbook.util.*
import app.lockbook.util.FileMetadata
import app.lockbook.util.FileType
import app.lockbook.util.SharedPreferences.BACKGROUND_SYNC_ENABLED_KEY
import app.lockbook.util.SharedPreferences.BACKGROUND_SYNC_PERIOD_KEY
import app.lockbook.util.SharedPreferences.BIOMETRIC_OPTION_KEY
import app.lockbook.util.SharedPreferences.EXPORT_ACCOUNT_QR_KEY
import app.lockbook.util.SharedPreferences.EXPORT_ACCOUNT_RAW_KEY
import app.lockbook.util.SharedPreferences.FILE_LAYOUT_KEY
import app.lockbook.util.SharedPreferences.GRID_LAYOUT
import app.lockbook.util.SharedPreferences.IS_THIS_AN_IMPORT_KEY
import app.lockbook.util.SharedPreferences.LINEAR_LAYOUT
import app.lockbook.util.SharedPreferences.OPEN_NEW_DOC_AUTOMATICALLY_KEY
import app.lockbook.util.SharedPreferences.SORT_FILES_A_Z
import app.lockbook.util.SharedPreferences.SORT_FILES_FIRST_CHANGED
import app.lockbook.util.SharedPreferences.SORT_FILES_KEY
import app.lockbook.util.SharedPreferences.SORT_FILES_LAST_CHANGED
import app.lockbook.util.SharedPreferences.SORT_FILES_TYPE
import app.lockbook.util.SharedPreferences.SORT_FILES_Z_A
import app.lockbook.util.SharedPreferences.SYNC_AUTOMATICALLY_KEY
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*
import timber.log.Timber

data class EditableFile(
    val name: String,
    val id: String,
)

class ListFilesViewModel(path: String, application: Application) :
    AndroidViewModel(application),
    ListFilesClickInterface {
    private val config = Config(path)
    var selectedFiles = listOf<Boolean>()

    var isFABOpen = false

    private val _stopProgressSpinner = SingleMutableLiveData<Unit>()
    private val _navigateToFileEditor = SingleMutableLiveData<EditableFile>()
    private val _navigateToDrawing = SingleMutableLiveData<EditableFile>()
    private val _switchFileLayout = SingleMutableLiveData<Unit>()
    private val _switchMenu = SingleMutableLiveData<Unit>()
    private val _collapseExpandFAB = SingleMutableLiveData<Boolean>()
    private val _showCreateFileDialog = SingleMutableLiveData<CreateFileInfo>()
    private val _showMoveFileDialog = SingleMutableLiveData<MoveFileInfo>()
    private val _showFileInfoDialog = SingleMutableLiveData<FileMetadata>()
    private val _showRenameFileDialog = SingleMutableLiveData<RenameFileInfo>()
    private val _uncheckAllFiles = SingleMutableLiveData<Unit>()
    private val _showSnackBar = SingleMutableLiveData<String>()
    private val _errorHasOccurred = SingleMutableLiveData<String>()

    private val _showSyncSnackBar = SingleMutableLiveData<Unit>()

    private val _unexpectedErrorHasOccurred = SingleMutableLiveData<String>()

    val stopProgressSpinner: LiveData<Unit>
        get() = _stopProgressSpinner

    val files: LiveData<List<FileMetadata>>
        get() = fileModel.files

    val showSyncSnackBar: LiveData<Unit>
        get() = _showSyncSnackBar

    val navigateToFileEditor: LiveData<EditableFile>
        get() = _navigateToFileEditor

    val navigateToDrawing: LiveData<EditableFile>
        get() = _navigateToDrawing

    val switchFileLayout: LiveData<Unit>
        get() = _switchFileLayout

    val switchMenu: LiveData<Unit>
        get() = _switchMenu

    val collapseExpandFAB: LiveData<Boolean>
        get() = _collapseExpandFAB

    val showCreateFileDialog: LiveData<CreateFileInfo>
        get() = _showCreateFileDialog

    val showMoveFileDialog: LiveData<MoveFileInfo>
        get() = _showMoveFileDialog

    val showFileInfoDialog: LiveData<FileMetadata>
        get() = _showFileInfoDialog

    val showRenameFileDialog: LiveData<RenameFileInfo>
        get() = _showRenameFileDialog

    val uncheckAllFiles: LiveData<Unit>
        get() = _uncheckAllFiles

    val updateBreadcrumbBar: LiveData<List<BreadCrumb>>
        get() = fileModel.updateBreadcrumbBar

    val showSnackBar: LiveData<String>
        get() = _showSnackBar

    val errorHasOccurred: LiveData<String>
        get() = _errorHasOccurred

    val unexpectedErrorHasOccurred: LiveData<String>
        get() = _unexpectedErrorHasOccurred

    private val fileModel = FileModel(config, _errorHasOccurred, _unexpectedErrorHasOccurred)

    init {
        init(null)
    }

    private fun init(listFilesFragment: ListFilesFragment?) {
        viewModelScope.launch(Dispatchers.IO) {
            setUpPreferenceChangeListener()
            isThisAnImport(listFilesFragment)
            fileModel.startUpInRoot()
        }
    }

    fun onBackPress(): Boolean = when {
        selectedFiles.contains(true) -> {
            collapseMoreOptionsMenu()
            true
        }
        !fileModel.isAtRoot() -> {
            fileModel.upADirectory()
            true
        }
        else -> false
    }

    fun onOpenedActivityEnd(listFilesFragment: ListFilesFragment) {
        viewModelScope.launch(Dispatchers.IO) {
            syncBasedOnPreferences(listFilesFragment)
        }
    }

    fun onSwipeToRefresh(listFilesFragment: ListFilesFragment) {
        viewModelScope.launch(Dispatchers.IO) {
            syncBasedOnPreferences(listFilesFragment)
            fileModel.refreshFiles()
            _stopProgressSpinner.postValue(Unit)
        }
    }

    fun onNewDocumentFABClicked(isDrawing: Boolean) {
        viewModelScope.launch(Dispatchers.IO) {
            isFABOpen = !isFABOpen
            _collapseExpandFAB.postValue(false)
            _showCreateFileDialog.postValue(CreateFileInfo(fileModel.parentFileMetadata.id, Klaxon().toJsonString(FileType.Document), isDrawing))
        }
    }

    fun onNewFolderFABClicked() {
        viewModelScope.launch(Dispatchers.IO) {
            isFABOpen = !isFABOpen
            _collapseExpandFAB.postValue(false)
            _showCreateFileDialog.postValue(CreateFileInfo(fileModel.parentFileMetadata.id, Klaxon().toJsonString(FileType.Folder), false))
        }
    }

    fun onMenuItemPressed(id: Int) {
        viewModelScope.launch(Dispatchers.IO) {
            val pref = PreferenceManager.getDefaultSharedPreferences(getApplication()).edit()
            when (id) {
                R.id.menu_list_files_sort_last_changed -> {
                    pref.putString(
                        SORT_FILES_KEY,
                        SORT_FILES_LAST_CHANGED
                    ).apply()
                    fileModel.refreshFiles()
                }
                R.id.menu_list_files_sort_a_z -> {
                    pref.putString(SORT_FILES_KEY, SORT_FILES_A_Z)
                        .apply()
                    fileModel.refreshFiles()
                }
                R.id.menu_list_files_sort_z_a -> {
                    pref.putString(SORT_FILES_KEY, SORT_FILES_Z_A)
                        .apply()
                    fileModel.refreshFiles()
                }
                R.id.menu_list_files_sort_first_changed -> {
                    pref.putString(
                        SORT_FILES_KEY,
                        SORT_FILES_FIRST_CHANGED
                    ).apply()
                    fileModel.refreshFiles()
                }
                R.id.menu_list_files_sort_type -> {
                    pref.putString(
                        SORT_FILES_KEY,
                        SORT_FILES_TYPE
                    ).apply()
                    fileModel.refreshFiles()
                }
                R.id.menu_list_files_linear_view -> {
                    pref.putString(
                        FILE_LAYOUT_KEY,
                        LINEAR_LAYOUT
                    ).apply()
                    _switchFileLayout.postValue(Unit)
                }
                R.id.menu_list_files_grid_view -> {
                    pref.putString(
                        FILE_LAYOUT_KEY,
                        GRID_LAYOUT
                    ).apply()
                    _switchFileLayout.postValue(Unit)
                }
                R.id.menu_list_files_rename -> {
                    files.value?.let { files ->
                        val checkedFiles = getSelectedFiles(files)
                        if (checkedFiles.size == 1) {
                            _showRenameFileDialog.postValue(RenameFileInfo(checkedFiles[0].id, checkedFiles[0].name))
                        } else {
                            _errorHasOccurred.postValue(BASIC_ERROR)
                        }
                    }
                }
                R.id.menu_list_files_delete -> {
                    files.value?.let { files ->
                        val checkedIds = getSelectedFiles(files).map { file -> file.id }
                        collapseMoreOptionsMenu()
                        if (fileModel.deleteFiles(checkedIds)) {
                            _showSnackBar.postValue("Successfully deleted the file(s)")
                        }

                        fileModel.refreshFiles()
                    }
                }
                R.id.menu_list_files_info -> {
                    files.value?.let { files ->
                        val checkedFiles = getSelectedFiles(files)
                        if (checkedFiles.size == 1) {
                            collapseMoreOptionsMenu()
                            _showFileInfoDialog.postValue(checkedFiles[0])
                        } else {
                            _errorHasOccurred.postValue(BASIC_ERROR)
                        }
                    }
                }
                R.id.menu_list_files_move -> {
                    files.value?.let { files ->
                        _showMoveFileDialog.postValue(
                            MoveFileInfo(
                                getSelectedFiles(files)
                                    .map { fileMetadata -> fileMetadata.id }.toTypedArray(),
                                getSelectedFiles(files)
                                    .map { fileMetadata -> fileMetadata.name }.toTypedArray()
                            )
                        )
                    }
                }
                else -> {
                    Timber.e("Unrecognized sort item id.")
                    _errorHasOccurred.postValue(BASIC_ERROR)
                }
            }.exhaustive
        }
    }

    override fun onItemClick(position: Int, isSelecting: Boolean, selection: List<Boolean>) {
        viewModelScope.launch(Dispatchers.IO) {
            when (isSelecting) {
                true -> {
                    selectedFiles = selection
                    _switchMenu.postValue(Unit)
                }
                false -> {
                    fileModel.files.value?.let { files ->
                        val fileMetadata = files[position]

                        if (fileMetadata.fileType == FileType.Folder) {
                            fileModel.intoFolder(fileMetadata)
                            selectedFiles = MutableList(files.size) {
                                false
                            }
                        } else {
                            enterDocument(fileMetadata)
                        }
                    }
                }
            }
        }
    }

    override fun onLongClick(position: Int, selection: List<Boolean>) {
        viewModelScope.launch(Dispatchers.IO) {
            selectedFiles = selection
            _switchMenu.postValue(Unit)
        }
    }

    fun refreshFiles(newDocument: FileMetadata?) {
        viewModelScope.launch(Dispatchers.IO) {
            collapseMoreOptionsMenu()
            fileModel.refreshFiles()

            if (newDocument != null && PreferenceManager.getDefaultSharedPreferences(getApplication())
                .getBoolean(OPEN_NEW_DOC_AUTOMATICALLY_KEY, true)
            ) {
                enterDocument(newDocument)
            }
        }
    }

    fun handleRefreshAtParent(position: Int) {
        viewModelScope.launch(Dispatchers.IO) {
            fileModel.refreshAtParent(position)
        }
    }

    fun collapseExpandFAB() {
        viewModelScope.launch(Dispatchers.IO) {
            isFABOpen = !isFABOpen
            _collapseExpandFAB.postValue(isFABOpen)
        }
    }

    private fun isThisAnImport(listFilesFragment: ListFilesFragment?) {
        if (PreferenceManager.getDefaultSharedPreferences(getApplication())
            .getBoolean(IS_THIS_AN_IMPORT_KEY, false)
        ) {
            if(listFilesFragment != null) {
                syncBasedOnPreferences(listFilesFragment)
                PreferenceManager.getDefaultSharedPreferences(getApplication()).edit().putBoolean(
                        IS_THIS_AN_IMPORT_KEY,
                        false
                ).apply()
            } else {
                _errorHasOccurred.postValue(BASIC_ERROR)
            }

        }
    }

    private fun setUpPreferenceChangeListener() {
        val listener = OnSharedPreferenceChangeListener { _, key ->
            when (key) {
                BACKGROUND_SYNC_ENABLED_KEY -> {
                    WorkManager.getInstance(getApplication())
                        .cancelAllWorkByTag(PERIODIC_SYNC_TAG)
                    Unit
                }
                SORT_FILES_KEY -> {
                    fileModel.refreshFiles()
                }
                SYNC_AUTOMATICALLY_KEY, EXPORT_ACCOUNT_RAW_KEY, EXPORT_ACCOUNT_QR_KEY, BIOMETRIC_OPTION_KEY, IS_THIS_AN_IMPORT_KEY, BACKGROUND_SYNC_PERIOD_KEY, FILE_LAYOUT_KEY -> Unit
                else -> {
                    _errorHasOccurred.postValue(BASIC_ERROR)
                    Timber.e("Unable to recognize preference key: $key")
                }
            }.exhaustive
        }

        PreferenceManager.getDefaultSharedPreferences(getApplication())
            .registerOnSharedPreferenceChangeListener(listener)
    }

    private fun getSelectedFiles(files: List<FileMetadata>): List<FileMetadata> = files.filterIndexed { index, _ ->
        selectedFiles[index]
    }

    private fun collapseMoreOptionsMenu() {
        selectedFiles = MutableList(files.value?.size ?: 0) { false }
        _switchMenu.postValue(Unit)
        _uncheckAllFiles.postValue(Unit)
    }

    private fun enterDocument(fileMetadata: FileMetadata) {
        val editableFileResult =
            EditableFile(fileMetadata.name, fileMetadata.id)
        fileModel.lastDocumentAccessed = fileMetadata
        if (fileMetadata.name.endsWith(".draw")) {
            _navigateToDrawing.postValue(editableFileResult)
        } else {
            _navigateToFileEditor.postValue(editableFileResult)
        }
    }

    fun syncBasedOnPreferences(listFilesFragment: ListFilesFragment) {
        if (PreferenceManager.getDefaultSharedPreferences(App.instance)
                        .getBoolean(SYNC_AUTOMATICALLY_KEY, false)
        ) {
            _showSyncSnackBar.postValue(Unit)

            when(val syncResult = CoreModel.sync(config, listFilesFragment)) {
                is Ok -> {
                    _showSnackBar.postValue("Up to date!")
                }
                is Err -> when(val error = syncResult.error) {
                    SyncAllError.NoAccount -> _errorHasOccurred.postValue("No account.")
                    SyncAllError.CouldNotReachServer -> _errorHasOccurred.postValue("Network unavailable.")
                    SyncAllError.ExecuteWorkError -> _errorHasOccurred.postValue("An unrecoverable execute work error has occurred.")
                    SyncAllError.ClientUpdateRequired -> _errorHasOccurred.postValue("Update required.")
                    is SyncAllError.Unexpected -> {
                        Timber.e("Unable to sync: ${error.error}")
                        _unexpectedErrorHasOccurred.postValue(error.error)
                    }
                }
            }.exhaustive
        }
    }
}
