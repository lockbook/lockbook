package app.lockbook.model

import androidx.lifecycle.LiveData
import app.lockbook.util.*
import com.github.michaelbull.result.*
import com.github.michaelbull.result.Ok

class SyncModel {
    var syncStatus: SyncStatus = SyncStatus.NotSyncing

    private val _notifySyncStepInfo = SingleMutableLiveData<SyncStepInfo>()

    val notifySyncStepInfo: LiveData<SyncStepInfo>
        get() = _notifySyncStepInfo

    fun trySync(): Result<Unit, CoreError<out UiCoreError>> =
        if (syncStatus is SyncStatus.NotSyncing) {
            val syncResult = sync()
            syncStatus = SyncStatus.NotSyncing
            syncResult
        } else {
            Ok(Unit)
        }

    // used by core over ffi
    fun updateSyncProgressAndTotal(
        total: Int,
        progress: Int,
        msg: String?
    ) {
        val syncProgress = SyncStepInfo(progress, total, msg ?: "")
        val newStatus = SyncStatus.Syncing(syncProgress)
        syncStatus = newStatus

        _notifySyncStepInfo.postValue(syncProgress)
    }

    fun hasSyncWork(): Result<Boolean, CoreError<out UiCoreError>> {
        return CoreModel.calculateWork().map { workCalculated -> workCalculated.workUnits.isNotEmpty() }
    }

    private fun sync(): Result<Unit, CoreError<out UiCoreError>> {
        syncStatus = SyncStatus.StartingSync
        return CoreModel.syncAll(this)
    }
}

sealed class SyncStatus {
    object NotSyncing : SyncStatus()
    object StartingSync : SyncStatus()
    data class Syncing(var syncStepInfo: SyncStepInfo) : SyncStatus()
}

data class SyncStepInfo(
    var progress: Int,
    var total: Int,
    var msg: String
)
