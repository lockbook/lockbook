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

/** The application's single native event receiver. Metadata refresh requests are coalesced by the store. */
object LbEvents {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val updates = MutableSharedFlow<LbEvent>(extraBufferCapacity = 32)
    val events = updates.asSharedFlow()

    fun start() {
        scope.launch {
            while (isActive) {
                val event = Lb.subscribe(Lb.eventsReceiver) ?: continue
                if (event.metadataChanged || event.documentWritten || event.pendingSharesChanged || event.ipcChangesApplied) {
                    FileMetadataStore.requestRefresh()
                }
                updates.emit(event)
            }
        }
    }
}
