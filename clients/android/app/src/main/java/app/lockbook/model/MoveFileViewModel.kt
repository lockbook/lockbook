package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import app.lockbook.util.*
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class MoveFileViewModel(application: Application, val startId: String) :
    AndroidViewModel(application) {
    private lateinit var currentParent: File
    lateinit var ids: List<String>

    var files = emptyDataSourceTyped<File>()

    private val _closeDialog = MutableLiveData<Unit>()
    private val _notifyError = SingleMutableLiveData<LbError>()
    private val _unexpectedErrorHasOccurred = SingleMutableLiveData<String>()

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
        }
    }

    private fun startWithCurrentParent() {
        when (val getFileByIdResult = CoreModel.getFileById(startId)) {
            is Ok -> {
                currentParent = getFileByIdResult.value
                refreshOverFolder()
            }
            is Err -> _notifyError.postValue(getFileByIdResult.error.toLbError(getRes()))
        }.exhaustive
    }

    fun moveFilesToCurrentFolder() {
        viewModelScope.launch(Dispatchers.IO) {
            for (id in ids) {
                val moveFileResult = CoreModel.moveFile(id, currentParent.id)

                if (moveFileResult is Err) {
                    _notifyError.postValue(moveFileResult.error.toLbError(getRes()))
                    return@launch
                }
            }

            _closeDialog.postValue(Unit)
        }
    }

    private fun refreshOverFolder() {
        when (val getChildrenResult = CoreModel.getChildren(currentParent.id)) {
            is Ok -> {
                val tempFiles = getChildrenResult.value.filter { fileMetadata ->
                    fileMetadata.fileType == FileType.Folder && !ids.contains(fileMetadata.id)
                }.toMutableList()

                if (!currentParent.isRoot()) {
                    tempFiles.add(
                        0,
                        File(
                            id = PARENT_ID,
                            fileType = FileType.Folder,
                            name = "...",
                        )
                    )
                }

                viewModelScope.launch(Dispatchers.Main) {
                    files.set(tempFiles)
                }
            }
            is Err -> when (val error = getChildrenResult.error) {
                is CoreError.UiError -> _unexpectedErrorHasOccurred.postValue(basicErrorString(getRes()))
                is CoreError.Unexpected -> _unexpectedErrorHasOccurred.postValue(error.content)
            }.exhaustive
        }
    }

    private fun setParentAsParent() {
        when (val getFileById = CoreModel.getFileById(currentParent.parent)) {
            is Ok -> currentParent = getFileById.value
            is Err -> _notifyError.postValue(getFileById.error.toLbError(getRes()))
        }.exhaustive
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
