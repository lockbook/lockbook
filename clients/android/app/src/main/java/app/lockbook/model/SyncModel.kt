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
    private val _notifyUpdateFilesUI: SingleMutableLiveData<UpdateFilesUI>
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
        _notifyUpdateFilesUI.postValue(UpdateFilesUI.UpdateSyncSnackBar(newStatus.total, newStatus.progress))
    }

    private fun sync(resources: Resources) {
        val upToDateMsg =
            resources.getString(R.string.list_files_sync_finished_snackbar)

        when (val workCalculatedResult = CoreModel.calculateWork(config)) {
            is Ok -> if (workCalculatedResult.value.localFiles.size + workCalculatedResult.value.serverFiles.size + workCalculatedResult.value.serverUnknownNameCount == 0) {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyWithSnackbar(upToDateMsg))
                return
            }
            is Err -> {
                _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(workCalculatedResult.error.toLbError(resources)))
                return
            }
        }

        syncStatus = SyncStatus.IsSyncing(0, 1)
        _notifyUpdateFilesUI.postValue(UpdateFilesUI.ShowSyncSnackBar)

        when (val syncResult = CoreModel.sync(config, this)) {
            is Ok -> _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyWithSnackbar(upToDateMsg))
            is Err -> _notifyUpdateFilesUI.postValue(UpdateFilesUI.NotifyError(syncResult.error.toLbError(resources)))
        }
    }
}

sealed class SyncStatus {
    object IsNotSyncing : SyncStatus()
    data class IsSyncing(var total: Int, var progress: Int) : SyncStatus()
}
