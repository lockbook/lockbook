package app.lockbook.model

import androidx.preference.PreferenceManager
import app.lockbook.App
import app.lockbook.App.Companion.config
import app.lockbook.R
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok

class SyncModel(
    private val _showSyncSnackBar: SingleMutableLiveData<Unit>,
    private val _updateSyncSnackBar: SingleMutableLiveData<Pair<Int, Int>>,
    private val _notifyWithSnackbar: SingleMutableLiveData<String>,
    private val _notifyError: SingleMutableLiveData<LbError>
) {

    var syncStatus: SyncStatus = SyncStatus.IsNotSyncing

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
            SyncStatus.IsNotSyncing -> LbError.basicError()
            is SyncStatus.IsSyncing -> {
                syncStatus = newStatus
                _updateSyncSnackBar.postValue(Pair(newStatus.total, newStatus.progress))
            }
        }
    }

    private fun sync() {
        val upToDateMsg =
            App.instance.resources.getString(R.string.list_files_sync_finished_snackbar)

        when (val workCalculatedResult = CoreModel.calculateWork(config)) {
            is Ok -> if (workCalculatedResult.value.localFiles.size + workCalculatedResult.value.serverFiles.size + workCalculatedResult.value.serverUnknownNameCount == 0) {
                return _notifyWithSnackbar.postValue(upToDateMsg)
            }
            is Err -> return _notifyError.postValue(workCalculatedResult.error.toLbError())
        }

        syncStatus = SyncStatus.IsSyncing(0, 1)
        _showSyncSnackBar.postValue(Unit)

        when (val syncResult = CoreModel.sync(config, this)) {
            is Ok -> _notifyWithSnackbar.postValue(upToDateMsg)
            is Err -> _notifyError.postValue(syncResult.error.toLbError())
        }
    }
}

sealed class SyncStatus {
    object IsNotSyncing : SyncStatus()
    data class IsSyncing(var total: Int, var progress: Int) : SyncStatus()
}
