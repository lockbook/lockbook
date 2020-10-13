package app.lockbook.loggedin.listfiles

import android.app.Activity.RESULT_CANCELED
import android.app.Application
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences.OnSharedPreferenceChangeListener
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkRequest
import android.net.wifi.SupplicantState
import android.net.wifi.WifiManager
import android.telephony.TelephonyManager
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.preference.PreferenceManager
import androidx.work.WorkManager
import app.lockbook.R
import app.lockbook.utils.*
import app.lockbook.utils.Messages.UNEXPECTED_ERROR_OCCURRED
import app.lockbook.utils.RequestResultCodes.DELETE_RESULT_CODE
import app.lockbook.utils.RequestResultCodes.HANDWRITING_EDITOR_REQUEST_CODE
import app.lockbook.utils.RequestResultCodes.POP_UP_INFO_REQUEST_CODE
import app.lockbook.utils.RequestResultCodes.RENAME_RESULT_CODE
import app.lockbook.utils.RequestResultCodes.TEXT_EDITOR_REQUEST_CODE
import app.lockbook.utils.SharedPreferences.BACKGROUND_SYNC_ENABLED_KEY
import app.lockbook.utils.SharedPreferences.BACKGROUND_SYNC_PERIOD_KEY
import app.lockbook.utils.SharedPreferences.BIOMETRIC_OPTION_KEY
import app.lockbook.utils.SharedPreferences.EXPORT_ACCOUNT_QR_KEY
import app.lockbook.utils.SharedPreferences.EXPORT_ACCOUNT_RAW_KEY
import app.lockbook.utils.SharedPreferences.IS_THIS_AN_IMPORT_KEY
import app.lockbook.utils.SharedPreferences.SORT_FILES_A_Z
import app.lockbook.utils.SharedPreferences.SORT_FILES_FIRST_CHANGED
import app.lockbook.utils.SharedPreferences.SORT_FILES_KEY
import app.lockbook.utils.SharedPreferences.SORT_FILES_LAST_CHANGED
import app.lockbook.utils.SharedPreferences.SORT_FILES_TYPE
import app.lockbook.utils.SharedPreferences.SORT_FILES_Z_A
import app.lockbook.utils.SharedPreferences.SYNC_AUTOMATICALLY_KEY
import app.lockbook.utils.WorkManagerTags.PERIODIC_SYNC_TAG
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*
import timber.log.Timber
import kotlin.collections.set

class ListFilesViewModel(path: String, application: Application) :
    AndroidViewModel(application),
    ClickInterface {
    private lateinit var fileCreationType: FileType
    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    private val fileModel = FileModel(path)
    val syncingStatus = SyncingStatus()
    var isFABOpen = false
    var dialogStatus = DialogStatus()

    private val _stopSyncSnackBar = SingleMutableLiveData<Unit>()
    private val _stopProgressSpinner = SingleMutableLiveData<Unit>()
    private val _showSyncSnackBar = SingleMutableLiveData<Int>()
    private val _showPreSyncSnackBar = SingleMutableLiveData<Int>()
    private val _showOfflineSnackBar = SingleMutableLiveData<Unit>()
    private val _updateProgressSnackBar = SingleMutableLiveData<Int>()
    private val _navigateToFileEditor = SingleMutableLiveData<EditableFile>()
    private val _navigateToHandwritingEditor = SingleMutableLiveData<EditableFile>()
    private val _navigateToPopUpInfo = SingleMutableLiveData<FileMetadata>()
    private val _collapseExpandFAB = SingleMutableLiveData<Boolean>()
    private val _createFileNameDialog = SingleMutableLiveData<Unit>()
    private val _errorHasOccurred = SingleMutableLiveData<String>()

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

    val navigateToPopUpInfo: LiveData<FileMetadata>
        get() = _navigateToPopUpInfo

    val collapseExpandFAB: LiveData<Boolean>
        get() = _collapseExpandFAB

    val createFileNameDialog: LiveData<Unit>
        get() = _createFileNameDialog

    val errorHasOccurred: LiveData<String>
        get() = _errorHasOccurred

    val fileModelErrorHasOccurred: LiveData<String>
        get() = fileModel.errorHasOccurred

    init {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                setUpPreferenceChangeListener()
                isThisAnImport()
                fileModel.startUpInRoot()
                setUpInternetListeners()
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

    private fun syncSnackBar() {
        when (val syncWorkResult = fileModel.determineSizeOfSyncWork()) {
            is Ok ->
                if (PreferenceManager.getDefaultSharedPreferences(getApplication())
                    .getBoolean(SYNC_AUTOMATICALLY_KEY, false)
                ) {
                    incrementalSyncIfNotRunning()
                }
            is Err -> when (val error = syncWorkResult.error) {
                is CalculateWorkError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is CalculateWorkError.CouldNotReachServer -> {
                    Timber.e("Could not reach server despite being online.")
                    _errorHasOccurred.postValue(
                        UNEXPECTED_ERROR_OCCURRED
                    )
                }
                is CalculateWorkError.UnexpectedError -> {
                    Timber.e("Unable to calculate syncWork: ${error.error}")
                    _errorHasOccurred.postValue(
                        UNEXPECTED_ERROR_OCCURRED
                    )
                }
            }
        }
    }

    private fun setUpInternetListeners() {
        val connectivityManager =
            getApplication<Application>().getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager

        val networkCallback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                if (fileModel.syncWorkAvailable()) {
                    syncSnackBar()
                }
            }

            override fun onLost(network: Network) {
                _showOfflineSnackBar.postValue(Unit)
            }
        }

        connectivityManager.registerNetworkCallback(
            NetworkRequest.Builder().build(),
            networkCallback
        )
        val wifiManager =
            getApplication<Application>().applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        val simManager =
            getApplication<Application>().applicationContext.getSystemService(Context.TELEPHONY_SERVICE) as TelephonyManager
        if (wifiManager.connectionInfo.supplicantState != SupplicantState.COMPLETED && simManager.dataState != TelephonyManager.DATA_CONNECTED) {
            _showOfflineSnackBar.postValue(Unit)
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
                BACKGROUND_SYNC_ENABLED_KEY ->
                    WorkManager.getInstance(getApplication())
                        .cancelAllWorkByTag(PERIODIC_SYNC_TAG)
                SYNC_AUTOMATICALLY_KEY, SORT_FILES_KEY, EXPORT_ACCOUNT_RAW_KEY, EXPORT_ACCOUNT_QR_KEY, BIOMETRIC_OPTION_KEY -> {
                }
                IS_THIS_AN_IMPORT_KEY, BACKGROUND_SYNC_PERIOD_KEY -> {
                }
                else -> {
                    _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
                    Timber.e("Unable to recognize preference key: $key")
                }
            }
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
                when {
                    requestCode == POP_UP_INFO_REQUEST_CODE && data is Intent -> handlePopUpInfoRequest(
                        resultCode,
                        data
                    )
                    TEXT_EDITOR_REQUEST_CODE == requestCode -> handleTextEditorRequest()
                    HANDWRITING_EDITOR_REQUEST_CODE == requestCode -> handleHandwritingEditorRequest()
                    resultCode == RESULT_CANCELED -> {
                    }
                    else -> {
                        Timber.e("Unable to recognize match requestCode and/or resultCode and/or data.")
                        _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
                    }
                }
            }
        }
    }

    private fun handleHandwritingEditorRequest() {
        if (PreferenceManager.getDefaultSharedPreferences(getApplication())
            .getBoolean(SYNC_AUTOMATICALLY_KEY, false)
        ) {
            incrementalSyncIfNotRunning()
        }
    }

    private fun handleTextEditorRequest() {
        if (PreferenceManager.getDefaultSharedPreferences(getApplication())
            .getBoolean(SYNC_AUTOMATICALLY_KEY, false)
        ) {
            incrementalSyncIfNotRunning()
        }
    }

    fun handleNewFileRequest(name: String) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                fileModel.createInsertRefreshFiles(name, Klaxon().toJsonString(fileCreationType))
                if (PreferenceManager.getDefaultSharedPreferences(getApplication())
                    .getBoolean(SYNC_AUTOMATICALLY_KEY, false)
                ) {
                    incrementalSyncIfNotRunning()
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
                        _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
                    }
                }
                DELETE_RESULT_CODE -> fileModel.deleteRefreshFiles(id)
                else -> {
                    Timber.e("Result code not matched: $resultCode")
                    _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
                }
            }
        } else {
            Timber.e("id is null.")
            _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
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
                fileCreationType = FileType.Document
                isFABOpen = !isFABOpen
                _collapseExpandFAB.postValue(false)
                dialogStatus.isDialogOpen = true
                _createFileNameDialog.postValue(Unit)
            }
        }
    }

    fun onNewFolderFABClicked() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                fileCreationType = FileType.Folder
                isFABOpen = !isFABOpen
                _collapseExpandFAB.postValue(false)
                dialogStatus.isDialogOpen = true
                _createFileNameDialog.postValue(Unit)
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

    fun onSortPressed(id: Int) {
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
                    else -> {
                        Timber.e("Unrecognized sort item id.")
                        _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
                    }
                }

                val files = fileModel.files.value
                if (files is List<FileMetadata>) {
                    fileModel.matchToDefaultSortOption(files)
                } else {
                    _errorHasOccurred.postValue("Unable to retrieve files from LiveData.")
                }
            }
        }
    }

    private fun incrementalSync() {
        syncingStatus.isSyncing = true

        val account = when (val accountResult = CoreModel.getAccount(fileModel.config)) {
            is Ok -> accountResult.value
            is Err -> return when (val error = accountResult.error) {
                is GetAccountError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is GetAccountError.UnexpectedError -> {
                    Timber.e("Unable to get account: ${error.error}")
                }
                else -> {
                    Timber.e("GetAccountError not matched: ${error::class.simpleName}.")
                    _errorHasOccurred.postValue(
                        UNEXPECTED_ERROR_OCCURRED
                    )
                }
            }
        }

        var syncWork =
            when (val syncWorkResult = CoreModel.calculateFileSyncWork(fileModel.config)) {
                is Ok -> syncWorkResult.value
                is Err -> return when (val error = syncWorkResult.error) {
                    is CalculateWorkError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                    is CalculateWorkError.CouldNotReachServer -> {
                    }
                    is CalculateWorkError.UnexpectedError -> {
                        Timber.e("Unable to calculate syncWork: ${error.error}")
                        _errorHasOccurred.postValue(
                            UNEXPECTED_ERROR_OCCURRED
                        )
                    }
                    else -> {
                        Timber.e("CalculateWorkError not matched: ${error::class.simpleName}.")
                        _errorHasOccurred.postValue(
                            UNEXPECTED_ERROR_OCCURRED
                        )
                    }
                }
            }

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
                        _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
                    } else {
                        _showPreSyncSnackBar.postValue(syncWork.workUnits.size)
                    }
                } else {
                    Timber.e("Despite all work being gone, syncErrors still persist.")
                    _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
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
                        _updateProgressSnackBar.postValue(currentProgress)
                        syncErrors.remove(workUnit.content.metadata.id)
                    }
                    is Err ->
                        syncErrors[workUnit.content.metadata.id] =
                            executeFileSyncWorkResult.error
                }
            }

            syncWork =
                when (val syncWorkResult = CoreModel.calculateFileSyncWork(fileModel.config)) {
                    is Ok -> syncWorkResult.value
                    is Err -> return when (val error = syncWorkResult.error) {
                        is CalculateWorkError.NoAccount -> {
                            _errorHasOccurred.postValue("Error! No account!")
                            _stopSyncSnackBar.postValue(Unit)
                        }
                        is CalculateWorkError.CouldNotReachServer -> {
                        }
                        is CalculateWorkError.UnexpectedError -> {
                            Timber.e("Unable to calculate syncWork: ${error.error}")
                            _errorHasOccurred.postValue(
                                UNEXPECTED_ERROR_OCCURRED
                            )
                            _stopSyncSnackBar.postValue(Unit)
                        }
                        else -> {
                            Timber.e("CalculateWorkError not matched: ${error::class.simpleName}.")
                            _errorHasOccurred.postValue(
                                UNEXPECTED_ERROR_OCCURRED
                            )
                            _stopSyncSnackBar.postValue(Unit)
                        }
                    }
                }
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

    override fun onItemClick(position: Int) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                fileModel.files.value?.let {
                    val fileMetadata = it[position]

                    if (fileMetadata.fileType == FileType.Folder) {
                        fileModel.intoFolder(fileMetadata)
                    } else {
                        val editableFileResult = EditableFile(fileMetadata.name, fileMetadata.id)
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

    override fun onLongClick(position: Int) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                fileModel.files.value?.let {
                    _navigateToPopUpInfo.postValue(it[position])
                }
            }
        }
    }
}
