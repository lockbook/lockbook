package app.lockbook.model

import android.app.Activity.RESULT_CANCELED
import android.app.Application
import android.content.SharedPreferences.OnSharedPreferenceChangeListener
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.preference.PreferenceManager
import androidx.work.WorkManager
import app.lockbook.R
import app.lockbook.ui.BreadCrumb
import app.lockbook.util.*
import app.lockbook.util.Messages.UNEXPECTED_CLIENT_ERROR
import app.lockbook.util.RequestResultCodes.HANDWRITING_EDITOR_REQUEST_CODE
import app.lockbook.util.RequestResultCodes.TEXT_EDITOR_REQUEST_CODE
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
import app.lockbook.util.WorkManagerTags.PERIODIC_SYNC_TAG
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*
import timber.log.Timber

class ListFilesViewModel(path: String, application: Application) :
    AndroidViewModel(application),
    ListFilesClickInterface {
    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    private val fileModel = FileModel(path)
    var selectedFiles = listOf<Boolean>()
    val syncingStatus = SyncingStatus()
    var isFABOpen = false

    private val _stopSyncSnackBar = SingleMutableLiveData<Unit>()
    private val _stopProgressSpinner = SingleMutableLiveData<Unit>()
    private val _showSyncSnackBar = SingleMutableLiveData<Int>()
    private val _updateSyncSnackBar = SingleMutableLiveData<Int>()
    private val _showPreSyncSnackBar = SingleMutableLiveData<Int>()
    private val _showOfflineSnackBar = SingleMutableLiveData<Unit>()
    private val _updateProgressSnackBar = SingleMutableLiveData<Int>()
    private val _navigateToFileEditor = SingleMutableLiveData<EditableFile>()
    private val _navigateToHandwritingEditor = SingleMutableLiveData<EditableFile>()
    private val _switchFileLayout = SingleMutableLiveData<Unit>()
    private val _switchMenu = SingleMutableLiveData<Unit>()
    private val _collapseExpandFAB = SingleMutableLiveData<Boolean>()
    private val _showCreateFileDialog = SingleMutableLiveData<CreateFileInfo>()
    private val _showMoveFileDialog = SingleMutableLiveData<MoveFileInfo>()
    private val _showFileInfoDialog = SingleMutableLiveData<FileMetadata>()
    private val _showRenameFileDialog = SingleMutableLiveData<RenameFileInfo>()
    private val _uncheckAllFiles = SingleMutableLiveData<Unit>()
    private val _showSuccessfulDeletion = SingleMutableLiveData<Unit>()
    private val _errorHasOccurred = SingleMutableLiveData<String>()
    private val _unexpectedErrorHasOccurred = SingleMutableLiveData<String>()

    val files: LiveData<List<FileMetadata>>
        get() = fileModel.files

    val stopSyncSnackBar: LiveData<Unit>
        get() = _stopSyncSnackBar

    val stopProgressSpinner: LiveData<Unit>
        get() = _stopProgressSpinner

    val showSyncSnackBar: LiveData<Int>
        get() = _showSyncSnackBar

    val updateSyncSnackBar: LiveData<Int>
        get() = _updateSyncSnackBar

    val showPreSyncSnackBar: LiveData<Int>
        get() = _showPreSyncSnackBar

    val showOfflineSnackBar: LiveData<Unit>
        get() = _showOfflineSnackBar

    val updateProgressSnackBar: LiveData<Int>
        get() = _updateProgressSnackBar

    val navigateToFileEditor: LiveData<EditableFile>
        get() = _navigateToFileEditor

    val navigateToHandwritingEditor: LiveData<EditableFile>
        get() = _navigateToHandwritingEditor

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

    val showSuccessfulDeletion: LiveData<Unit>
        get() = _showSuccessfulDeletion

    val errorHasOccurred: LiveData<String>
        get() = _errorHasOccurred

    val fileModelErrorHasOccurred: LiveData<String>
        get() = fileModel.errorHasOccurred

    val fileModeUnexpectedErrorHasOccurred: LiveData<String>
        get() = fileModel.unexpectedErrorHasOccurred

    val unexpectedErrorHasOccurred: LiveData<String>
        get() = _unexpectedErrorHasOccurred

    init {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                setUpPreferenceChangeListener()
                isThisAnImport()
                fileModel.startUpInRoot()
            }
        }
    }

    private fun isThisAnImport() {
        if (PreferenceManager.getDefaultSharedPreferences(getApplication())
            .getBoolean(IS_THIS_AN_IMPORT_KEY, false)
        ) {
            incrementalSync()
            PreferenceManager.getDefaultSharedPreferences(getApplication()).edit().putBoolean(
                IS_THIS_AN_IMPORT_KEY,
                false
            ).apply()
            syncingStatus.isSyncing = false
            syncingStatus.maxProgress = 0
        }
    }

    private fun incrementalSyncIfNotRunning() {
        if (!syncingStatus.isSyncing) {
            incrementalSync()
            fileModel.refreshFiles()
            syncingStatus.isSyncing = false
            syncingStatus.maxProgress = 0
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
                    _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                    Timber.e("Unable to recognize preference key: $key")
                }
            }.exhaustive
        }

        PreferenceManager.getDefaultSharedPreferences(getApplication())
            .registerOnSharedPreferenceChangeListener(listener)
    }

    fun quitOrNot(): Boolean {
        if (selectedFiles.contains(true)) {
            collapseMoreOptionsMenu()
            return true
        } else if (fileModel.isAtRoot()) {
            return false
        }
        fileModel.upADirectory()

        return true
    }

    fun handleRefreshAtParent(position: Int) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                fileModel.refreshAtParent(position)
            }
        }
    }

    fun handleUpdateBreadcrumbWithLatest() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                fileModel.updateBreadCrumbWithLatest()
            }
        }
    }

    fun handleActivityResult(requestCode: Int) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                when (requestCode) {
                    TEXT_EDITOR_REQUEST_CODE, HANDWRITING_EDITOR_REQUEST_CODE -> syncBasedOnPreferences()
                    RESULT_CANCELED -> {}
                    else -> {
                        Timber.e("Unable to recognize match requestCode: $requestCode.")
                        _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                    }
                }.exhaustive
            }
        }
    }

    private fun syncBasedOnPreferences() {
        if (PreferenceManager.getDefaultSharedPreferences(getApplication())
            .getBoolean(SYNC_AUTOMATICALLY_KEY, false)
        ) {
            incrementalSyncIfNotRunning()
        }
    }

    fun onSwipeToRefresh() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                incrementalSyncIfNotRunning()
                _stopProgressSpinner.postValue(Unit)
            }
        }
    }

    fun onNewDocumentFABClicked(isDrawing: Boolean) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                isFABOpen = !isFABOpen
                _collapseExpandFAB.postValue(false)
                _showCreateFileDialog.postValue(CreateFileInfo(fileModel.parentFileMetadata.id, Klaxon().toJsonString(FileType.Document), isDrawing))
            }
        }
    }

    fun onNewFolderFABClicked() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                isFABOpen = !isFABOpen
                _collapseExpandFAB.postValue(false)
                _showCreateFileDialog.postValue(CreateFileInfo(fileModel.parentFileMetadata.id, Klaxon().toJsonString(FileType.Folder), false))
            }
        }
    }

    fun collapseExpandFAB() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                isFABOpen = !isFABOpen
                _collapseExpandFAB.postValue(isFABOpen)
            }
        }
    }

    fun onMenuItemPressed(id: Int) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
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
                                _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                            }
                        }
                    }
                    R.id.menu_list_files_delete -> {
                        files.value?.let { files ->
                            val checkedIds = getSelectedFiles(files).map { file -> file.id }
                            collapseMoreOptionsMenu()
                            if (fileModel.deleteFiles(checkedIds)) {
                                _showSuccessfulDeletion.postValue(Unit)
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
                                _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
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
                        _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                    }
                }.exhaustive
            }
        }
    }

    private fun getSelectedFiles(files: List<FileMetadata>): List<FileMetadata> = files.filterIndexed { index, _ ->
        selectedFiles[index]
    }

    private fun collapseMoreOptionsMenu() {
        selectedFiles = MutableList(files.value?.size ?: 0) { false }
        _switchMenu.postValue(Unit)
        _uncheckAllFiles.postValue(Unit)
    }

    private fun incrementalSync() {
        syncingStatus.isSyncing = true

        val account = when (val accountResult = CoreModel.getAccount(fileModel.config)) {
            is Ok -> accountResult.value
            is Err -> return when (val error = accountResult.error) {
                is GetAccountError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is GetAccountError.Unexpected -> {
                    Timber.e("Unable to get account: ${error.error}")
                }
            }
        }.exhaustive

        val syncErrors = hashMapOf<String, ExecuteWorkError>()

        var workCalculated =
            when (val syncWorkResult = CoreModel.calculateWork(fileModel.config)) {
                is Ok -> syncWorkResult.value
                is Err -> return when (val error = syncWorkResult.error) {
                    is CalculateWorkError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                    is CalculateWorkError.CouldNotReachServer -> _showOfflineSnackBar.postValue(Unit)
                    is CalculateWorkError.ClientUpdateRequired -> _errorHasOccurred.postValue("Update required.")
                    is CalculateWorkError.Unexpected -> {
                        Timber.e("Unable to calculate syncWork: ${error.error}")
                        _unexpectedErrorHasOccurred.postValue(
                            error.error
                        )
                    }
                }
            }.exhaustive

        if (workCalculated.workUnits.isEmpty()) {
            _showPreSyncSnackBar.postValue(workCalculated.workUnits.size)
            return
        }

        _showSyncSnackBar.postValue(workCalculated.workUnits.size)

        var currentProgress = 0
        syncingStatus.maxProgress = workCalculated.workUnits.size

        for (test in 0..10) {
            for (workUnit in workCalculated.workUnits) {
                when (
                    val executeFileSyncWorkResult =
                        CoreModel.executeWork(fileModel.config, account, workUnit)
                ) {
                    is Ok -> {
                        currentProgress++
                        syncErrors.remove(workUnit.content.metadata.id)
                        _updateProgressSnackBar.postValue(currentProgress)
                    }
                    is Err ->
                        syncErrors[workUnit.content.metadata.id] =
                            executeFileSyncWorkResult.error
                }.exhaustive
            }

            if (syncErrors.isEmpty()) {
                val setLastSyncedResult =
                    CoreModel.setLastSynced(
                        fileModel.config,
                        workCalculated.mostRecentUpdateFromServer
                    )
                if (setLastSyncedResult is Err) {
                    Timber.e("Unable to set most recent sync date: ${setLastSyncedResult.error}")
                    _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                }
            }

            workCalculated =
                when (val syncWorkResult = CoreModel.calculateWork(fileModel.config)) {
                    is Ok -> syncWorkResult.value
                    is Err -> return when (val error = syncWorkResult.error) {
                        is CalculateWorkError.NoAccount -> {
                            _stopSyncSnackBar.postValue(Unit)
                            _errorHasOccurred.postValue("Error! No account!")
                        }
                        is CalculateWorkError.CouldNotReachServer -> _showOfflineSnackBar.postValue(Unit)
                        is CalculateWorkError.ClientUpdateRequired -> _errorHasOccurred.postValue("Update required.")
                        is CalculateWorkError.Unexpected -> {
                            Timber.e("Unable to calculate syncWork: ${error.error}")
                            _stopSyncSnackBar.postValue(Unit)
                            _unexpectedErrorHasOccurred.postValue(
                                error.error
                            )
                        }
                    }
                }.exhaustive

            if (workCalculated.workUnits.isEmpty()) {
                break
            }

            if ((currentProgress + workCalculated.workUnits.size) > syncingStatus.maxProgress) {
                syncingStatus.maxProgress = workCalculated.workUnits.size + currentProgress
                _updateSyncSnackBar.postValue(syncingStatus.maxProgress)
            }
        }

        if (syncErrors.isNotEmpty()) {
            Timber.e("Couldn't resolve all syncErrors: ${Klaxon().toJsonString(syncErrors)}")
            _errorHasOccurred.postValue("Couldn't sync all files.")
            _stopSyncSnackBar.postValue(Unit)
        } else {
            _showPreSyncSnackBar.postValue(workCalculated.workUnits.size)
        }
    }

    override fun onItemClick(position: Int, isSelecting: Boolean, selection: List<Boolean>) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                if (isSelecting) {
                    selectedFiles = selection
                    _switchMenu.postValue(Unit)
                } else {
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

    private fun enterDocument(fileMetadata: FileMetadata) {
        val editableFileResult =
            EditableFile(fileMetadata.name, fileMetadata.id)
        fileModel.lastDocumentAccessed = fileMetadata
        if (fileMetadata.name.endsWith(".draw")) {
            _navigateToHandwritingEditor.postValue(editableFileResult)
        } else {
            _navigateToFileEditor.postValue(editableFileResult)
        }
    }

    fun refreshAndAssessChanges(newDocument: FileMetadata?) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                collapseMoreOptionsMenu()
                fileModel.refreshFiles()

                if (newDocument != null && PreferenceManager.getDefaultSharedPreferences(getApplication())
                    .getBoolean(OPEN_NEW_DOC_AUTOMATICALLY_KEY, true)
                ) {
                    enterDocument(newDocument)
                }
            }
        }
    }

    override fun onLongClick(position: Int, selection: List<Boolean>) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                selectedFiles = selection
                _switchMenu.postValue(Unit)
            }
        }
    }
}
