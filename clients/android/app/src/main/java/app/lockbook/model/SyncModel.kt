package app.lockbook.model

import androidx.preference.PreferenceManager
import app.lockbook.App
import app.lockbook.R
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import timber.log.Timber

class SyncModel(private val config: Config, private val _showSnackBar: SingleMutableLiveData<String>, private val _errorHasOccurred: SingleMutableLiveData<String>, private val _unexpectedErrorHasOccurred: SingleMutableLiveData<String>) {

    var syncStatus: SyncStatus = SyncStatus.IsNotSyncing
    val _showSyncSnackBar = SingleMutableLiveData<Unit>()
    val _updateSyncSnackBar = SingleMutableLiveData<Pair<Int, Int>>()

    fun trySync() {
        if (syncStatus is SyncStatus.IsNotSyncing
        ) {
            sync()
            syncStatus = SyncStatus.IsNotSyncing
        }
    }

    fun syncBasedOnPreferences() {
        if (PreferenceManager.getDefaultSharedPreferences(App.instance)
            .getBoolean(SharedPreferences.SYNC_AUTOMATICALLY_KEY, false)
        ) {
            trySync()
        }
    }

    fun updateSyncProgressAndTotal(total: Int, progress: Int) { // used by core over ffi
        val newStatus = SyncStatus.IsSyncing(total, progress)

        when (syncStatus) {
            SyncStatus.IsNotSyncing -> _errorHasOccurred.postValue(BASIC_ERROR)
            is SyncStatus.IsSyncing -> {
                syncStatus = newStatus
                _updateSyncSnackBar.postValue(Pair(newStatus.total, newStatus.progress))
            }
        }
    }

    private fun sync() {
        val upToDateMsg = App.instance.resources.getString(R.string.list_files_sync_finished_snackbar)

        when (val workCalculatedResult = CoreModel.calculateWork(config)) {
            is Ok -> {
                if (workCalculatedResult.value.workUnits.isEmpty()) {
                    return _showSnackBar.postValue(upToDateMsg)
                }
            }
            is Err -> return when (val error = workCalculatedResult.error) {
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
        }

        syncStatus = SyncStatus.IsSyncing(0, 1)
        _showSyncSnackBar.postValue(Unit)

        when (val syncResult = CoreModel.sync(config, this)) {
            is Ok -> {
                _showSnackBar.postValue(upToDateMsg)
            }
            is Err -> when (val error = syncResult.error) {
                SyncAllError.NoAccount -> _errorHasOccurred.postValue("No account.")
                SyncAllError.CouldNotReachServer -> _errorHasOccurred.postValue("Network unavailable.")
                SyncAllError.ClientUpdateRequired -> _errorHasOccurred.postValue("Update required.")
                is SyncAllError.Unexpected -> {
                    Timber.e("Unable to sync: ${error.error}")
                    _unexpectedErrorHasOccurred.postValue(error.error)
                }
            }
        }.exhaustive
    }
}

sealed class SyncStatus() {
    object IsNotSyncing : SyncStatus()
    data class IsSyncing(var total: Int, var progress: Int) : SyncStatus()
}
