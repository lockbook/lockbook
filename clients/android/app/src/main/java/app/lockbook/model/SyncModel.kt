package app.lockbook.model

import androidx.lifecycle.LiveData
import androidx.lifecycle.ViewModel
import app.lockbook.util.*
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.SyncProgress

class SyncRepository private constructor() : SyncProgress {
    var syncStatus: SyncStatus = SyncStatus.NotSyncing
        private set

    private val _notifySyncStepInfo = SingleMutableLiveData<SyncStepInfo>()
    val notifySyncStepInfo: LiveData<SyncStepInfo> = _notifySyncStepInfo

    private val _notifySyncDone = SingleMutableLiveData<NotifySyncDone>()
    val notifySyncDone: LiveData<NotifySyncDone> = _notifySyncDone

    fun trySync() {
        if (syncStatus is SyncStatus.NotSyncing) {
            syncStatus = SyncStatus.StartingSync

            try {
                _notifySyncStepInfo.postValue(SyncStepInfo(-1, 100, "loading"))
                Lb.sync(this)
                _notifySyncDone.postValue(NotifySyncDone.FinishedSync)
            } catch (error: LbError) {
                _notifySyncDone.postValue(NotifySyncDone.NotifyError(error))
            } finally {
                syncStatus = SyncStatus.NotSyncing
            }
        }
    }

    override fun updateSyncProgressAndTotal(
        total: Int,
        progress: Int,
        msg: String?
    ) {
        val syncProgress = SyncStepInfo(progress, total, msg ?: "")
        syncStatus = SyncStatus.Syncing(syncProgress)
        _notifySyncStepInfo.postValue(syncProgress)
    }

    companion object {
        @Volatile
        private var INSTANCE: SyncRepository? = null

        fun getInstance(): SyncRepository {
            return INSTANCE ?: synchronized(this) {
                INSTANCE ?: SyncRepository().also { INSTANCE = it }
            }
        }
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
