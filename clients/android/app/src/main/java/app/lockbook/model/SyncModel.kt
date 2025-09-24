package app.lockbook.model

import androidx.lifecycle.LiveData
import app.lockbook.util.*
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.SyncProgress

class SyncModel : SyncProgress {
    var syncStatus: SyncStatus = SyncStatus.NotSyncing

    private val _notifySyncStepInfo = SingleMutableLiveData<SyncStepInfo>()

    val notifySyncStepInfo: LiveData<SyncStepInfo>
        get() = _notifySyncStepInfo

    val _notifySyncDone = SingleMutableLiveData<NotifySyncDone>()
    val notifySyncDone: LiveData<NotifySyncDone>
        get() = _notifySyncDone

    fun trySync() {
        if (syncStatus is SyncStatus.NotSyncing) {
            syncStatus = SyncStatus.StartingSync
            Lb.sync(this)

            syncStatus = SyncStatus.NotSyncing
        }
    }

    override fun updateSyncProgressAndTotal(
        total: Int,
        progress: Int,
        msg: String?
    ) {
        val syncProgress = SyncStepInfo(progress, total, msg ?: "")
        val newStatus = SyncStatus.Syncing(syncProgress)
        syncStatus = newStatus
        _notifySyncStepInfo.postValue(syncProgress)
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

sealed class NotifySyncDone {
    data class NotifyError(val error: LbError) : NotifySyncDone()
    object FinishedSync : NotifySyncDone()
}
