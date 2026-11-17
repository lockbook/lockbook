package app.lockbook.model

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import net.lockbook.Lb
import net.lockbook.LbEvent
import timber.log.Timber

/** The application's single native event receiver. Metadata refresh requests are coalesced by the store. */
object LbEvents {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val updates = MutableSharedFlow<LbEvent>(extraBufferCapacity = 32)
    val events = updates.asSharedFlow()

    fun start() {
        Timber.tag("UsernameDebug").d("native event receiver starting")
        scope.launch {
            while (isActive) {
                val event = Lb.subscribe(Lb.eventsReceiver) ?: continue
                if (event.metadataChanged || event.pendingSharesChanged || event.ipcChangesApplied || event.syncFinished) {
                    Timber.tag("UsernameDebug").d(
                        "native event metadata=%s pendingShares=%s ipc=%s syncFinished=%s documentWritten=%s",
                        event.metadataChanged,
                        event.pendingSharesChanged,
                        event.ipcChangesApplied,
                        event.syncFinished,
                        event.documentWritten,
                    )
                }
                if (event.metadataChanged || event.documentWritten || event.pendingSharesChanged || event.ipcChangesApplied) {
                    FileMetadataStore.requestRefresh()
                }
                updates.emit(event)
            }
        }
    }
}
