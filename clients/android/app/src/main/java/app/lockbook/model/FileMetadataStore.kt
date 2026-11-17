package app.lockbook.model

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import net.lockbook.File
import net.lockbook.Lb
import net.lockbook.LbError

sealed interface MetadataLoadState {
    data object Loading : MetadataLoadState

    data object Ready : MetadataLoadState

    data class Failed(
        val error: LbError,
    ) : MetadataLoadState
}

private data class FolderLocation(
    val parent: String,
    val name: String,
)

/** One published metadata snapshot; folder navigation belongs to individual consumers. */
class FileMetadataSnapshot(
    val root: File,
    val files: List<File>,
    val pendingShares: List<File>,
) {
    val idsAndFiles: Map<String, File> = (files + pendingShares).associateBy { it.id }

    private val folderLocations =
        idsAndFiles.values
            .filter { it.type == File.FileType.Folder }
            .associate { it.id to FolderLocation(it.parent, it.name) }
    private val childrenByParent = idsAndFiles.values.filter { it.id != it.parent }.groupBy { it.parent }

    fun sameFoldersAs(other: FileMetadataSnapshot): Boolean = root.id == other.root.id && folderLocations == other.folderLocations

    fun children(id: String): List<File> = FileModel.sortFiles(childrenByParent[id].orEmpty())
}

object FileMetadataStore {
    private val lock = Any()
    private var generation = 0L
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val requests = Channel<Unit>(Channel.CONFLATED)
    private val refreshWaiters = mutableListOf<CompletableDeferred<FileMetadataSnapshot>>()
    private val state = MutableStateFlow<FileMetadataSnapshot?>(null)
    val snapshots = state.asStateFlow()
    private val loading = MutableStateFlow<MetadataLoadState>(MetadataLoadState.Loading)
    val loadState = loading.asStateFlow()
    val folderSnapshots = snapshots.filterNotNull().distinctUntilChanged(FileMetadataSnapshot::sameFoldersAs)

    init {
        scope.launch {
            var nextRefreshAt = 0L
            for (request in requests) {
                // Refresh immediately after idle; coalesce subsequent writes without starving updates.
                val remaining = nextRefreshAt - System.nanoTime() / 1_000_000
                if (state.value != null && remaining > 0) delay(remaining)
                requests.tryReceive()
                refresh()
                nextRefreshAt = System.nanoTime() / 1_000_000 + 80
            }
        }
    }

    // Synchronous lookups only read the cache. Loading always runs in the worker above.
    fun current(): FileMetadataSnapshot =
        state.value ?: throw LbError().apply {
            kind = LbError.LbEC.TryAgain
            msg = "File metadata is still loading."
            trace = ""
        }

    fun requestRefresh() {
        synchronized(lock) {
            if (state.value == null) loading.value = MetadataLoadState.Loading
            requests.trySend(Unit)
        }
    }

    suspend fun awaitLoaded(): FileMetadataSnapshot {
        state.value?.let { return it }
        requestRefresh()
        return when (val result = loadState.first { it != MetadataLoadState.Loading }) {
            is MetadataLoadState.Failed -> throw result.error
            else -> current()
        }
    }

    /** Wait for a refresh that starts after this request, even if metadata is already cached. */
    suspend fun refreshAndAwait(): FileMetadataSnapshot {
        val waiter = CompletableDeferred<FileMetadataSnapshot>()
        synchronized(lock) {
            refreshWaiters += waiter
            if (state.value == null) loading.value = MetadataLoadState.Loading
            requests.trySend(Unit)
        }
        return try {
            waiter.await()
        } finally {
            synchronized(lock) { refreshWaiters.remove(waiter) }
        }
    }

    private fun refresh() {
        val (version, waiters) =
            synchronized(lock) {
                generation to refreshWaiters.toList().also { refreshWaiters.clear() }
            }
        try {
            val previous = state.value?.idsAndFiles.orEmpty()

            fun retainUnchanged(file: File): File = previous[file.id]?.takeIf { it.sameMetadataAs(file) } ?: file
            val listed = Lb.listMetadatas().map(::retainUnchanged)
            val root = retainUnchanged(Lb.getRoot())
            val files = if (listed.any { it.id == root.id }) listed else listed + root
            val shares = Lb.getPendingShareFiles().map(::retainUnchanged).sortedBy { it.id }
            val snapshot = FileMetadataSnapshot(root, files, shares)
            synchronized(lock) {
                if (version == generation) {
                    state.value = snapshot
                    loading.value = MetadataLoadState.Ready
                    waiters.forEach { it.complete(snapshot) }
                } else {
                    waiters.forEach { it.cancel() }
                }
            }
        } catch (error: LbError) {
            synchronized(lock) {
                if (version == generation) {
                    if (error.kind == LbError.LbEC.AccountNonexistent || error.kind == LbError.LbEC.RootNonexistent) state.value = null
                    loading.value = MetadataLoadState.Failed(error)
                    waiters.forEach { it.completeExceptionally(error) }
                } else {
                    waiters.forEach { it.cancel() }
                }
            }
        }
    }

    fun requireFile(id: String): File =
        current().idsAndFiles[id] ?: throw LbError().apply {
            kind = LbError.LbEC.FileNonexistent
            msg = "This file no longer exists."
            trace = ""
        }

    fun clear() {
        synchronized(lock) {
            generation++
            requests.tryReceive()
            refreshWaiters.forEach { it.cancel() }
            refreshWaiters.clear()
            state.value = null
            loading.value = MetadataLoadState.Loading
        }
    }
}

/** Core's Java metadata objects have reference equality; reuse unchanged entries across snapshots. */
private fun File.sameMetadataAs(other: File): Boolean =
    id == other.id && parent == other.parent && name == other.name && type == other.type &&
        lastModified == other.lastModified && lastModifiedBy == other.lastModifiedBy &&
        shares.size == other.shares.size &&
        shares.indices.all { index ->
            val left = shares[index]
            val right = other.shares[index]
            left.mode == right.mode && left.sharedBy == right.sharedBy && left.sharedWith == right.sharedWith
        }
