package app.lockbook.model

import androidx.lifecycle.LiveData
import androidx.preference.PreferenceManager
import app.lockbook.App
import app.lockbook.R
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import timber.log.Timber

class SyncModel(private val config: Config, private val _showSnackBar: SingleMutableLiveData<String>, private val _errorHasOccurred: SingleMutableLiveData<String>, private val _unexpectedErrorHasOccurred: SingleMutableLiveData<String>) {

    var syncStatus: SyncStatus = SyncStatus.IsNotSyncing

    private val _stopSyncSnackBar = SingleMutableLiveData<Unit>()
    private val _showSyncSnackBar = SingleMutableLiveData<Int>()
    private val _showPreSyncSnackBar = SingleMutableLiveData<Int>()
    private val _updateProgressSnackBar = SingleMutableLiveData<Int>()

    val stopSyncSnackBar: LiveData<Unit>
        get() = _stopSyncSnackBar

    val showSyncSnackBar: LiveData<Int>
        get() = _showSyncSnackBar

    val showPreSyncSnackBar: LiveData<Int>
        get() = _showPreSyncSnackBar

    val updateProgressSnackBar: LiveData<Int>
        get() = _updateProgressSnackBar

    fun startSync() {
        if (syncStatus is SyncStatus.IsNotSyncing) {
            incrementalSync()
            syncStatus = SyncStatus.IsNotSyncing
        }
    }

    fun syncBasedOnPreferences() {
        if (PreferenceManager.getDefaultSharedPreferences(App.instance)
            .getBoolean(SharedPreferences.SYNC_AUTOMATICALLY_KEY, false)
        ) {
            startSync()
        }
    }

    private fun incrementalSync() {
        val tempSyncStatus = SyncStatus.IsSyncing(0, 0)

        val account = when (val accountResult = CoreModel.getAccount(config)) {
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
            when (val syncWorkResult = CoreModel.calculateWork(config)) {
                is Ok -> syncWorkResult.value
                is Err -> return when (val error = syncWorkResult.error) {
                    is CalculateWorkError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                    is CalculateWorkError.CouldNotReachServer -> _showSnackBar.postValue(App.instance.resources.getString(R.string.list_files_offline_snackbar))
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

        tempSyncStatus.progress = 0
        tempSyncStatus.maxProgress = workCalculated.workUnits.size

        syncStatus = tempSyncStatus

        for (test in 0..10) {
            for (workUnit in workCalculated.workUnits) {
                when (
                    val executeFileSyncWorkResult =
                        CoreModel.executeWork(config, account, workUnit)
                ) {
                    is Ok -> {
                        (syncStatus as SyncStatus.IsSyncing).progress++
                        syncErrors.remove(workUnit.content.metadata.id)
                        _updateProgressSnackBar.postValue((syncStatus as SyncStatus.IsSyncing).progress)
                    }
                    is Err ->
                        syncErrors[workUnit.content.metadata.id] =
                            executeFileSyncWorkResult.error
                }.exhaustive
            }

            if (syncErrors.isEmpty()) {
                val setLastSyncedResult =
                    CoreModel.setLastSynced(
                        config,
                        workCalculated.mostRecentUpdateFromServer
                    )
                if (setLastSyncedResult is Err) {
                    Timber.e("Unable to set most recent sync date: ${setLastSyncedResult.error}")
                    _errorHasOccurred.postValue(BASIC_ERROR)
                }
            }

            workCalculated =
                when (val syncWorkResult = CoreModel.calculateWork(config)) {
                    is Ok -> syncWorkResult.value
                    is Err -> return when (val error = syncWorkResult.error) {
                        is CalculateWorkError.NoAccount -> {
                            _stopSyncSnackBar.postValue(Unit)
                            _errorHasOccurred.postValue("Error! No account!")
                        }
                        is CalculateWorkError.CouldNotReachServer -> _showSnackBar.postValue(App.instance.resources.getString(R.string.list_files_offline_snackbar))
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
            } else {
                (syncStatus as SyncStatus.IsSyncing).maxProgress = workCalculated.workUnits.size
                _showSyncSnackBar.postValue((syncStatus as SyncStatus.IsSyncing).maxProgress)
                (syncStatus as SyncStatus.IsSyncing).progress = 0
            }
        }

        if (syncErrors.isNotEmpty()) {
            Timber.e("Couldn't resolve all syncErrors: ${Klaxon().toJsonString(syncErrors)}")
            _stopSyncSnackBar.postValue(Unit)
            _errorHasOccurred.postValue("Couldn't sync all files.")
        } else {
            _showPreSyncSnackBar.postValue(workCalculated.workUnits.size)
        }
    }
}
