package app.lockbook.model

import androidx.lifecycle.LiveData
import app.lockbook.App.Companion.config
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result

class SyncModel() {
    var syncStatus: SyncStatus = SyncStatus.NotSyncing

    private val _notifySyncProgress = SingleMutableLiveData<SyncProgress>()

    val notifySyncProgress: LiveData<SyncProgress>
        get() = _notifySyncProgress

    fun trySync(): Result<Unit, CoreError> =
        if (syncStatus is SyncStatus.NotSyncing) {
            val syncResult = sync()
            syncStatus = SyncStatus.NotSyncing
            syncResult
        } else {
            Ok(Unit)
        }

    fun updateSyncProgressAndTotal(
        total: Int,
        progress: Int,
        isPushing: Boolean,
        fileName: String?
    ) { // used by core over ffi
        val syncAction = when {
            isPushing && fileName != null -> {
                SyncMessage.PushingDocument(fileName)
            }
            isPushing && fileName == null -> {
                SyncMessage.PushingMetadata
            }
            !isPushing && fileName != null -> {
                SyncMessage.PullingDocument(fileName)
            }
            else -> {
                SyncMessage.PullingMetadata
            }
        }

        val syncProgress = SyncProgress(progress, total, syncAction)
        val newStatus = SyncStatus.Syncing(syncProgress)
        syncStatus = newStatus

        _notifySyncProgress.postValue(syncProgress)
    }

    private fun sync(): Result<Unit, CoreError> {
        when (val workCalculatedResult = CoreModel.calculateWork(config)) {
            is Ok -> if (workCalculatedResult.value.workUnits.isEmpty()) {
                return Ok(Unit)
            }
            is Err -> {
                return Err(workCalculatedResult.error)
            }
        }

        syncStatus = SyncStatus.StartingSync
        return CoreModel.sync(config, this)
    }
}

sealed class SyncStatus {
    object NotSyncing : SyncStatus()
    object StartingSync : SyncStatus()
    data class Syncing(var syncProgress: SyncProgress) : SyncStatus()
}

data class SyncProgress(
    var progress: Int,
    var total: Int,
    var action: SyncMessage
)

sealed class SyncMessage {
    object PullingMetadata : SyncMessage()
    object PushingMetadata : SyncMessage()
    data class PullingDocument(val fileName: String) : SyncMessage()
    data class PushingDocument(val fileName: String) : SyncMessage()

    override fun toString(): String = when (val syncMessage = this) {
        is PullingDocument -> "Pulling ${syncMessage.fileName}."
        is PushingDocument -> "Pushing ${syncMessage.fileName}."
        PullingMetadata -> "Pulling files."
        PushingMetadata -> "Pushing files."
    }
}
