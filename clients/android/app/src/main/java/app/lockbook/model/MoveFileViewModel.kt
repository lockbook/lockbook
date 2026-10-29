@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.util.*
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import net.lockbook.File
import net.lockbook.File.FileType
import net.lockbook.Lb
import net.lockbook.LbError

class MoveFileViewModel(
    application: Application,
    private val startId: String?,
    private val sourceFileId: String? = null,
) : AndroidViewModel(application) {
    lateinit var currentParent: File
    var ids: List<String> = emptyList()
        set(value) {
            field = value
            if (::currentParent.isInitialized) refreshOverFolder()
        }

    var files = emptyDataSourceTyped<File>()

    private val _ready = MutableLiveData(false)
    val ready: LiveData<Boolean> = _ready
    private val _closeDialog = MutableLiveData<Unit>()
    private val _notifyError = SingleMutableLiveData<LbError>()

    val closeDialog: LiveData<Unit>
        get() = _closeDialog

    val notifyError: LiveData<LbError>
        get() = _notifyError

    companion object {
        const val PARENT_ID = "PARENT"
    }

    init {
        viewModelScope.launch(Dispatchers.IO) {
            startWithCurrentParent()
            FileMetadataStore.folderSnapshots.collect { snapshot ->
                if (::currentParent.isInitialized) refreshOverFolder()
            }
        }
    }

    private suspend fun startWithCurrentParent() {
        try {
            val snapshot = FileMetadataStore.awaitLoaded()
            val parentId = startId ?: sourceFileId?.let { FileMetadataStore.requireFile(it).parent }
            currentParent = parentId?.let(FileMetadataStore::requireFile) ?: snapshot.root
            refreshOverFolder()
            _ready.postValue(true)
        } catch (err: LbError) {
            _notifyError.postValue(err)
        }
    }

    fun moveFilesToCurrentFolder() {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                for (id in ids) {
                    Lb.moveFile(id, currentParent.id)
                }
            } catch (err: LbError) {
                _notifyError.postValue(err)
                return@launch
            }

            _closeDialog.postValue(Unit)
        }
    }

    fun refreshOverFolder() {
        try {
            val snapshot = FileMetadataStore.current()
            currentParent = snapshot.idsAndFiles[currentParent.id]?.takeIf { it.type == FileType.Folder } ?: snapshot.root
            val tempFiles =
                snapshot
                    .children(currentParent.id)
                    .filter { file ->
                        file.type == FileType.Folder && !ids.contains(file.id)
                    }.toMutableList()

            if (!currentParent.isRoot) {
                val parent = File()
                parent.id = PARENT_ID
                parent.type = FileType.Folder
                parent.name = "..."
                tempFiles.add(0, parent)
            }

            viewModelScope.launch(Dispatchers.Main) {
                files.set(FileModel.sortFiles(tempFiles))
            }
        } catch (err: LbError) {
            _notifyError.postValue(err)
        }
    }

    private fun setParentAsParent() {
        try {
            currentParent = FileMetadataStore.requireFile(currentParent.parent)
        } catch (err: LbError) {
            _notifyError.postValue(err)
        }
    }

    fun onItemClick(item: File) {
        viewModelScope.launch(Dispatchers.IO) {
            when (item.id) {
                PARENT_ID -> {
                    setParentAsParent()
                    refreshOverFolder()
                }

                else -> {
                    currentParent = item
                    refreshOverFolder()
                }
            }
        }
    }
}
