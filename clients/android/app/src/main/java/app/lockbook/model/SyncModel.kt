package app.lockbook.model

import android.content.Context
import android.content.res.Resources
import androidx.preference.PreferenceManager
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

    fun trySync(context: Context) {
        if (syncStatus is SyncStatus.IsNotSyncing
        ) {
            sync(context.resources)
            syncStatus = SyncStatus.IsNotSyncing
        }
    }

    fun syncBasedOnPreferences(context: Context) {
        if (PreferenceManager.getDefaultSharedPreferences(context)
            .getBoolean(getString(context.resources, R.string.sync_automatically_key), false)
        ) {
            trySync(context)
        }
    }

    fun updateSyncProgressAndTotal(total: Int, progress: Int) { // used by core over ffi
        val newStatus = SyncStatus.IsSyncing(total, progress)

        syncStatus = newStatus
        _updateSyncSnackBar.postValue(Pair(newStatus.total, newStatus.progress))
    }

    private fun sync(resources: Resources) {
        val upToDateMsg =
            resources.getString(R.string.list_files_sync_finished_snackbar)

        when (val workCalculatedResult = CoreModel.calculateWork(config)) {
            is Ok -> if (workCalculatedResult.value.localFiles.size + workCalculatedResult.value.serverFiles.size + workCalculatedResult.value.serverUnknownNameCount == 0) {
                return _notifyWithSnackbar.postValue(upToDateMsg)
            }
            is Err -> return _notifyError.postValue(workCalculatedResult.error.toLbError(resources))
        }

        syncStatus = SyncStatus.IsSyncing(0, 1)
        _showSyncSnackBar.postValue(Unit)

        when (val syncResult = CoreModel.sync(config, this)) {
            is Ok -> _notifyWithSnackbar.postValue(upToDateMsg)
            is Err -> _notifyError.postValue(syncResult.error.toLbError(resources))
        }
    }
}

sealed class SyncStatus {
    object IsNotSyncing : SyncStatus()
    data class IsSyncing(var total: Int, var progress: Int) : SyncStatus()
}
