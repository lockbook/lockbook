package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.util.DocumentCoverStore
import app.lockbook.util.DocumentPreviewLoader
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.LbError
import timber.log.Timber

internal class WorkspaceDocumentsViewModel(
    application: Application,
) : AndroidViewModel(application) {
    val previews = DocumentPreviewLoader(application, DocumentCoverStore(application))
    private val items = MutableLiveData<List<WorkspaceDocumentItem>>(emptyList())
    val documents: LiveData<List<WorkspaceDocumentItem>> = items

    init {
        viewModelScope.launch {
            combine(observePinnedFiles(application).distinctUntilChanged(), FileMetadataStore.snapshots.filterNotNull()) { pins, snapshot ->
                pins to snapshot
            }.collectLatest { (pins, snapshot) ->
                try {
                    val updated =
                        withContext(Dispatchers.IO) {
                            val files = snapshot.files
                            val cursors = previews.cursorPositions()
                            workspaceDocumentItems(pins, files).map { it.copy(cursor = cursors[it.file.id] ?: 0) }
                        }
                    val previous = items.value.orEmpty()
                    if (previous.size != updated.size ||
                        previous.zip(updated).any { (old, new) -> !old.sameContentAs(new) }
                    ) {
                        items.value = updated
                    }
                } catch (error: LbError) {
                    Timber.e(error, "Unable to refresh welcome documents.")
                }
            }
        }
    }
}
