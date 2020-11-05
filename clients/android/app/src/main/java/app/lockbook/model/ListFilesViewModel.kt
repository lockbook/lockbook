package app.lockbook.model

import android.app.Activity.RESULT_CANCELED
import android.app.Application
import android.content.Intent
import android.content.SharedPreferences.OnSharedPreferenceChangeListener
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.preference.PreferenceManager
import androidx.work.WorkManager
import app.lockbook.R
import app.lockbook.ui.FileModel
import app.lockbook.util.*
import app.lockbook.util.Messages.UNEXPECTED_CLIENT_ERROR
import app.lockbook.util.RequestResultCodes.DELETE_RESULT_CODE
import app.lockbook.util.RequestResultCodes.HANDWRITING_EDITOR_REQUEST_CODE
import app.lockbook.util.RequestResultCodes.POP_UP_INFO_REQUEST_CODE
import app.lockbook.util.RequestResultCodes.RENAME_RESULT_CODE
import app.lockbook.util.RequestResultCodes.TEXT_EDITOR_REQUEST_CODE
import app.lockbook.util.SharedPreferences.BACKGROUND_SYNC_ENABLED_KEY
import app.lockbook.util.SharedPreferences.BACKGROUND_SYNC_PERIOD_KEY
import app.lockbook.util.SharedPreferences.BIOMETRIC_OPTION_KEY
import app.lockbook.util.SharedPreferences.EXPORT_ACCOUNT_QR_KEY
import app.lockbook.util.SharedPreferences.EXPORT_ACCOUNT_RAW_KEY
import app.lockbook.util.SharedPreferences.IS_THIS_AN_IMPORT_KEY
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
    var fileMenuShowing = false
    var renameFileDialogStatus = RenameFileDialogInfo()
    var createFileDialogInfo = CreateFileDialogInfo()

    private val _stopSyncSnackBar = SingleMutableLiveData<Unit>()
    private val _stopProgressSpinner = SingleMutableLiveData<Unit>()
    private val _showSyncSnackBar = SingleMutableLiveData<Int>()
    private val _showPreSyncSnackBar = SingleMutableLiveData<Int>()
    private val _showOfflineSnackBar = SingleMutableLiveData<Unit>()
    private val _updateProgressSnackBar = SingleMutableLiveData<Int>()
    private val _navigateToFileEditor = SingleMutableLiveData<EditableFile>()
    private val _navigateToHandwritingEditor = SingleMutableLiveData<EditableFile>()
    private val _switchMenu = SingleMutableLiveData<Unit>()
    private val _collapseExpandFAB = SingleMutableLiveData<Boolean>()
    private val _showCreateFileDialog = SingleMutableLiveData<Unit>()
    private val _showMoveFileDialog = SingleMutableLiveData<Array<String>>()
    private val _showFileInfoDialog = SingleMutableLiveData<FileMetadata>()
    private val _showRenameFileDialog = SingleMutableLiveData<Unit>()
    private val _uncheckAllFiles = SingleMutableLiveData<Unit>()
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

    val switchMenu: LiveData<Unit>
        get() = _switchMenu

    val collapseExpandFAB: LiveData<Boolean>
        get() = _collapseExpandFAB

    val showCreateFileDialog: LiveData<Unit>
        get() = _showCreateFileDialog

    val showMoveFileDialog: LiveData<Array<String>>
        get() = _showMoveFileDialog

    val showFileInfoDialog: LiveData<FileMetadata>
        get() = _showFileInfoDialog

    val showRenameFileDialog: LiveData<Unit>
        get() = _showRenameFileDialog

    val uncheckAllFiles: LiveData<Unit>
        get() = _uncheckAllFiles

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
                SYNC_AUTOMATICALLY_KEY, SORT_FILES_KEY, EXPORT_ACCOUNT_RAW_KEY, EXPORT_ACCOUNT_QR_KEY, BIOMETRIC_OPTION_KEY, IS_THIS_AN_IMPORT_KEY, BACKGROUND_SYNC_PERIOD_KEY -> Unit
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
        if (fileModel.isAtRoot()) {
            return false
        }
        fileModel.upADirectory()

        return true
    }

    fun handleActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                when (requestCode) {
                    POP_UP_INFO_REQUEST_CODE -> {
                        if (data != null) {
                            handlePopUpInfoRequest(
                                resultCode,
                                data
                            )
                        } else {
                            Timber.e("Data from activity result is null.")
                            _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                        }
                    }
                    TEXT_EDITOR_REQUEST_CODE, HANDWRITING_EDITOR_REQUEST_CODE -> syncBasedOnPreferences()
                    RESULT_CANCELED -> {
                    }
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

    fun handleNewFileRequest(name: String) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                fileModel.createInsertRefreshFiles(name, Klaxon().toJsonString(createFileDialogInfo.fileCreationType))
                syncBasedOnPreferences()
            }
        }
    }

    fun handleRenameRequest(newName: String) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                files.value?.let { files ->
                    fileModel.renameRefreshFiles(files[0].id, newName)
                    syncBasedOnPreferences()
                }
            }
        }
    }

    private fun handlePopUpInfoRequest(resultCode: Int, data: Intent) {
        val id = data.getStringExtra("id")
        if (id is String) {
            when (resultCode) {
                RENAME_RESULT_CODE -> {
                    val newName = data.getStringExtra("new_name")
                    if (newName != null) {
                        fileModel.renameRefreshFiles(id, newName)
                    } else {
                        Timber.e("new_name is null.")
                        _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                    }
                }
                DELETE_RESULT_CODE -> fileModel.deleteRefreshFiles(id)
                else -> {
                    Timber.e("Result code not matched: $resultCode")
                    _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                }
            }.exhaustive
        } else {
            Timber.e("id is null.")
            _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
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

    fun onNewDocumentFABClicked() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                createFileDialogInfo.fileCreationType = FileType.Document
                isFABOpen = !isFABOpen
                _collapseExpandFAB.postValue(false)
                createFileDialogInfo.isDialogOpen = true
                _showCreateFileDialog.postValue(Unit)
            }
        }
    }

    fun onNewFolderFABClicked() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                createFileDialogInfo.fileCreationType = FileType.Folder
                isFABOpen = !isFABOpen
                _collapseExpandFAB.postValue(false)
                createFileDialogInfo.isDialogOpen = true
                _showCreateFileDialog.postValue(Unit)
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
                    R.id.menu_list_files_sort_last_changed -> pref.putString(
                        SORT_FILES_KEY,
                        SORT_FILES_LAST_CHANGED
                    ).apply()
                    R.id.menu_list_files_sort_a_z ->
                        pref.putString(SORT_FILES_KEY, SORT_FILES_A_Z)
                            .apply()
                    R.id.menu_list_files_sort_z_a ->
                        pref.putString(SORT_FILES_KEY, SORT_FILES_Z_A)
                            .apply()
                    R.id.menu_list_files_sort_first_changed -> pref.putString(
                        SORT_FILES_KEY,
                        SORT_FILES_FIRST_CHANGED
                    ).apply()
                    R.id.menu_list_files_sort_type -> pref.putString(
                        SORT_FILES_KEY,
                        SORT_FILES_TYPE
                    ).apply()
                    R.id.menu_list_files_rename -> {
                        renameFileDialogStatus.isDialogOpen = true
                        _showRenameFileDialog.postValue(Unit)
                    }
                    R.id.menu_list_files_delete -> {

                    }
                    R.id.menu_list_files_info -> {
                        files.value?.let { files ->

                            val checkedFiles = files.filterIndexed { index, _ ->
                                selectedFiles[index]
                            }
                            Timber.e("START")
                            selectedFiles.forEach { Timber.e(it.toString()) }
                            Timber.e("END")
                            if (checkedFiles.size == 1) {
                                _showFileInfoDialog.postValue(checkedFiles[0])
                            } else {
                                _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                            }
                        }
                    }
                    R.id.menu_list_files_move -> {
                        _showMoveFileDialog.postValue(files.value?.filterIndexed { index, _ ->
                            selectedFiles[index]
                        }?.map { fileMetadata -> fileMetadata.id }?.toTypedArray())
                    }
                    else -> {
                        Timber.e("Unrecognized sort item id.")
                        _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                    }
                }.exhaustive

                val files = fileModel.files.value
                if (files is List<FileMetadata>) {
                    fileModel.matchToDefaultSortOption(files)
                } else {
                    _errorHasOccurred.postValue("Unable to retrieve files from LiveData.")
                }
            }
        }
    }

    fun collapseMoreOptionsMenu() {
        _switchMenu.postValue(Unit)
        selectedFiles = MutableList(files.value?.size ?: 0) { false }
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

        var syncWork =
            when (val syncWorkResult = CoreModel.calculateFileSyncWork(fileModel.config)) {
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

        if (syncWork.workUnits.isNotEmpty()) {
            _showSyncSnackBar.postValue(syncWork.workUnits.size)
        }

        var currentProgress = 0
        syncingStatus.maxProgress = syncWork.workUnits.size
        val syncErrors = hashMapOf<String, ExecuteWorkError>()
        repeat(10) {
            if ((currentProgress + syncWork.workUnits.size) > syncingStatus.maxProgress) {
                syncingStatus.maxProgress = currentProgress + syncWork.workUnits.size
                _showSyncSnackBar.postValue(syncingStatus.maxProgress)
            }

            if (syncWork.workUnits.isEmpty()) {
                return if (syncErrors.isEmpty()) {
                    val setLastSyncedResult =
                        CoreModel.setLastSynced(
                            fileModel.config,
                            syncWork.mostRecentUpdateFromServer
                        )
                    if (setLastSyncedResult is Err) {
                        Timber.e("Unable to set most recent update date: ${setLastSyncedResult.error}")
                        _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                    } else {
                        _showPreSyncSnackBar.postValue(syncWork.workUnits.size)
                    }
                } else {
                    Timber.e("Despite all work being gone, syncErrors still persist.")
                    _errorHasOccurred.postValue(UNEXPECTED_CLIENT_ERROR)
                    _stopSyncSnackBar.postValue(Unit)
                }
            }
            for (workUnit in syncWork.workUnits) {
                when (
                    val executeFileSyncWorkResult =
                        CoreModel.executeFileSyncWork(fileModel.config, account, workUnit)
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

            syncWork =
                when (val syncWorkResult = CoreModel.calculateFileSyncWork(fileModel.config)) {
                    is Ok -> syncWorkResult.value
                    is Err -> return when (val error = syncWorkResult.error) {
                        is CalculateWorkError.NoAccount -> {
                            _errorHasOccurred.postValue("Error! No account!")
                            _stopSyncSnackBar.postValue(Unit)
                        }
                        is CalculateWorkError.CouldNotReachServer -> _showOfflineSnackBar.postValue(Unit)
                        is CalculateWorkError.ClientUpdateRequired -> _errorHasOccurred.postValue("Update required.")
                        is CalculateWorkError.Unexpected -> {
                            Timber.e("Unable to calculate syncWork: ${error.error}")
                            _unexpectedErrorHasOccurred.postValue(
                                error.error
                            )
                            _stopSyncSnackBar.postValue(Unit)
                        }
                    }
                }.exhaustive
        }
        if (syncErrors.isNotEmpty()) {
            Timber.e("Couldn't resolve all syncErrors: ${Klaxon().toJsonString(syncErrors)}")
            _errorHasOccurred.postValue("Couldn't sync all files.")
            _stopSyncSnackBar.postValue(Unit)
        } else {
            _stopSyncSnackBar.postValue(Unit)
            _showPreSyncSnackBar.postValue(syncWork.workUnits.size)
        }
    }

    override fun onItemClick(position: Int, isSelecting: Boolean, selection: List<Boolean>) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                fileModel.files.value?.let {
                    if(isSelecting) {
                        setSelectionAndChange(selection)
                    } else {

                        val fileMetadata = it[position]

                        if (fileMetadata.fileType == FileType.Folder) {
                            fileModel.intoFolder(fileMetadata)
                            selectedFiles = MutableList(files.value?.size ?: 0) {
                                false
                            }
                        } else {
                            val editableFileResult =
                                EditableFile(fileMetadata.name, fileMetadata.id)
                            fileModel.lastDocumentAccessed = fileMetadata
                            if (fileMetadata.name.endsWith(".draw")) {
                                _navigateToHandwritingEditor.postValue(editableFileResult)
                            } else {
                                _navigateToFileEditor.postValue(editableFileResult)
                            }
                        }
                    }
                }
            }
        }
    }

    override fun onLongClick(position: Int, selection: List<Boolean>) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                setSelectionAndChange(selection)
            }
        }
    }

    private fun setSelectionAndChange(selection: List<Boolean>) {
        val before = selectedFiles.contains(true)

        selectedFiles = selection

        if (before != selectedFiles.contains(true)) {
            _switchMenu.postValue(Unit)
        }
    }
}
