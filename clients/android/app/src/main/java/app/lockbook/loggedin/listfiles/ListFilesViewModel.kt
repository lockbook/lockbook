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
import androidx.lifecycle.MutableLiveData
import androidx.preference.PreferenceManager
import androidx.work.WorkManager
import app.lockbook.App
import app.lockbook.R
import app.lockbook.utils.*
import app.lockbook.utils.Messages.UNEXPECTED_ERROR_OCCURRED
import app.lockbook.utils.RequestResultCodes.DELETE_RESULT_CODE
import app.lockbook.utils.RequestResultCodes.POP_UP_INFO_REQUEST_CODE
import app.lockbook.utils.RequestResultCodes.RENAME_RESULT_CODE
import app.lockbook.utils.RequestResultCodes.TEXT_EDITOR_REQUEST_CODE
import app.lockbook.utils.SharedPreferences.BACKGROUND_SYNC_ENABLED_KEY
import app.lockbook.utils.SharedPreferences.BACKGROUND_SYNC_PERIOD_KEY
import app.lockbook.utils.SharedPreferences.BIOMETRIC_OPTION_KEY
import app.lockbook.utils.SharedPreferences.EXPORT_ACCOUNT_QR_KEY
import app.lockbook.utils.SharedPreferences.EXPORT_ACCOUNT_RAW_KEY
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
    val fileModel = FileModel(path)
    var isFABOpen = false
    var syncMaxProgress = 0

    private val _earlyStopSyncSnackBar = MutableLiveData<Unit>()
    private val _stopProgressSpinner = MutableLiveData<Unit>()
    private val _showSyncSnackBar = MutableLiveData<Int>()
    private val _showPreSyncSnackBar = MutableLiveData<Int>()
    private val _showOfflineSnackBar = MutableLiveData<Unit>()
    private val _updateProgressSnackBar = MutableLiveData<Int>()
    private val _navigateToFileEditor = MutableLiveData<EditableFile>()
    private val _navigateToPopUpInfo = MutableLiveData<FileMetadata>()
    private val _collapseExpandFAB = MutableLiveData<Unit>()
    private val _createFileNameDialog = MutableLiveData<Unit>()
    private val _errorHasOccurred = MutableLiveData<String>()

    val earlyStopSyncSnackBar: LiveData<Unit>
        get() = _earlyStopSyncSnackBar

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

    val navigateToPopUpInfo: LiveData<FileMetadata>
        get() = _navigateToPopUpInfo

    val collapseExpandFAB: LiveData<Unit>
        get() = _collapseExpandFAB

    val createFileNameDialog: LiveData<Unit>
        get() = _createFileNameDialog

    val errorHasOccurred: LiveData<String>
        get() = _errorHasOccurred

    fun startUpFiles() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                setUpPreferenceChangeListener()
                fileModel.startUpInRoot()
                setUpInternetListeners()
            }
        }
    }

    private fun syncSnackBar() {
        when (val syncWorkResult = fileModel.determineSizeOfSyncWork()) {
            is Ok -> if (PreferenceManager.getDefaultSharedPreferences(getApplication())
                    .getBoolean(SYNC_AUTOMATICALLY_KEY, false)
            ) {
                if (syncMaxProgress == 0) {
                    incrementalSync()
                }
            } else {
                _showPreSyncSnackBar.postValue(syncWorkResult.value)
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
                super.onAvailable(network)
                if (fileModel.syncWorkAvailable()) {
                    syncSnackBar()
                }
            }

            override fun onLost(network: Network) {
                super.onLost(network)
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

    private fun setUpPreferenceChangeListener() {
        val listener = OnSharedPreferenceChangeListener { _, key ->
            when (key) {
                BACKGROUND_SYNC_ENABLED_KEY ->
                    WorkManager.getInstance(getApplication())
                        .cancelAllWorkByTag(PERIODIC_SYNC_TAG)
                BACKGROUND_SYNC_PERIOD_KEY -> {
                }
                SYNC_AUTOMATICALLY_KEY, SORT_FILES_KEY, EXPORT_ACCOUNT_RAW_KEY, EXPORT_ACCOUNT_QR_KEY, BIOMETRIC_OPTION_KEY -> {
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

    private fun handleTextEditorRequest() {
        if (PreferenceManager.getDefaultSharedPreferences(getApplication())
                .getBoolean(SYNC_AUTOMATICALLY_KEY, false)
        ) {
            incrementalSyncProgressSnackBar()
        }
    }

    fun handleNewFileRequest(name: String) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                fileModel.createInsertRefreshFiles(name, Klaxon().toJsonString(fileCreationType))
                if (PreferenceManager.getDefaultSharedPreferences(getApplication())
                        .getBoolean(SYNC_AUTOMATICALLY_KEY, false)
                ) {
                    if (syncMaxProgress == 0) {
                        incrementalSync()
                    }
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
                incrementalSyncProgressSnackBar()
                _stopProgressSpinner.postValue(Unit)
            }
        }
    }

    private fun incrementalSyncProgressSnackBar() {
        incrementalSync()
        fileModel.refreshFiles()
    }

    fun onNewDocumentFABClicked() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                fileCreationType = FileType.Document
                _collapseExpandFAB.postValue(Unit)
                _createFileNameDialog.postValue(Unit)
            }
        }
    }

    fun onNewFolderFABClicked() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                fileCreationType = FileType.Folder
                _collapseExpandFAB.postValue(Unit)
                _createFileNameDialog.postValue(Unit)
            }
        }
    }

    fun collapseFAB() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                _collapseExpandFAB.postValue(Unit)
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
        val syncErrors = hashMapOf<String, ExecuteWorkError>()

        val account = when (val accountResult = fileModel.coreModel.getAccount()) {
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

        syncMaxProgress = when (val syncWorkResult = fileModel.coreModel.calculateFileSyncWork()) {
            is Ok -> syncWorkResult.value.work_units.size
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

        _showSyncSnackBar.postValue(syncMaxProgress)
        var currentProgress = 0

        repeat(10) {
            val syncWork = when (val syncWorkResult = fileModel.coreModel.calculateFileSyncWork()) {
                is Ok -> syncWorkResult.value
                is Err -> {
                    when (val error = syncWorkResult.error) {
                        is CalculateWorkError.NoAccount -> {
                            _errorHasOccurred.postValue("Error! No account!")
                            _earlyStopSyncSnackBar.postValue(Unit)
                        }
                        is CalculateWorkError.CouldNotReachServer -> {
                        }
                        is CalculateWorkError.UnexpectedError -> {
                            Timber.e("Unable to calculate syncWork: ${error.error}")
                            _errorHasOccurred.postValue(
                                UNEXPECTED_ERROR_OCCURRED
                            )
                            _earlyStopSyncSnackBar.postValue(Unit)
                        }
                        else -> {
                            Timber.e("CalculateWorkError not matched: ${error::class.simpleName}.")
                            _errorHasOccurred.postValue(
                                UNEXPECTED_ERROR_OCCURRED
                            )
                            _earlyStopSyncSnackBar.postValue(Unit)
                        }
                    }


                    syncMaxProgress = 0
                    return
                }
            }

            if (syncWork.work_units.isEmpty()) {
                if (syncErrors.isEmpty()) {
                    val setLastSyncedResult =
                        fileModel.coreModel.setLastSynced(syncWork.most_recent_update_from_server)
                    if (setLastSyncedResult is Err) {
                        Timber.e("Unable to set most recent update date: ${setLastSyncedResult.error}")
                        _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
                    }

                } else {
                    Timber.e("Despite all work being gone, syncErrors still persist.")
                    _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
                    _earlyStopSyncSnackBar.postValue(Unit)
                }

                syncMaxProgress = 0
                return
            }

            for (workUnit in syncWork.work_units) {
                when (
                    val executeFileSyncWorkResult =
                        fileModel.coreModel.executeFileSyncWork(account, workUnit)
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
        }

        if (syncErrors.isNotEmpty()) {
            Timber.e("Couldn't resolve all syncErrors.")
            _errorHasOccurred.postValue("Couldn't sync all files.")
            _earlyStopSyncSnackBar.postValue(Unit)
        }

        syncMaxProgress = 0
    }

    override fun onItemClick(position: Int) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                fileModel.files.value?.let {
                    val fileMetadata = it[position]

                    if (fileMetadata.file_type == FileType.Folder) {
                        fileModel.intoFolder(fileMetadata)
                    } else {
                        val editableFileResult = fileModel.handleReadDocument(fileMetadata)
                        if (editableFileResult is EditableFile) {
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
