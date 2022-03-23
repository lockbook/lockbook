package app.lockbook.model

import android.app.Application
import androidx.lifecycle.*
import app.lockbook.App.Companion.config
import app.lockbook.getRes
import app.lockbook.util.*
import com.afollestad.recyclical.datasource.emptyDataSourceTyped
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*

class MoveFileViewModel(application: Application) :
    AndroidViewModel(application) {
    private lateinit var currentParent: DecryptedFileMetadata
    lateinit var ids: Array<String>

    var files = emptyDataSourceTyped<DecryptedFileMetadata>()

    private val _closeDialog = MutableLiveData<Unit>()
    private val _notifyError = SingleMutableLiveData<LbError>()
    private val _unexpectedErrorHasOccurred = SingleMutableLiveData<String>()

    val closeDialog: LiveData<Unit>
        get() = _closeDialog

    val notifyError: LiveData<LbError>
        get() = _notifyError

    init {
        viewModelScope.launch(Dispatchers.IO) {
            startInRoot()
        }
    }

    private fun startInRoot() {
        viewModelScope.launch(Dispatchers.IO) {
            when (val rootResult = CoreModel.getRoot(config)) {
                is Ok -> {
                    currentParent = rootResult.value
                    refreshOverFolder()
                }
                is Err -> _notifyError.postValue(rootResult.error.toLbError(getRes()))
            }.exhaustive
        }
    }

    fun moveFilesToFolder(ids: Array<String>) {
        viewModelScope.launch(Dispatchers.IO) {
            for (id in ids) {
                when (val moveFileResult = CoreModel.moveFile(config, id, currentParent.id)) {
                    is Ok -> {
                    }
                    is Err -> {
                        _notifyError.postValue(moveFileResult.error.toLbError(getRes()))
                        _closeDialog.postValue(Unit)
                        return@launch
                    }
                }
            }

            _closeDialog.postValue(Unit)
        }
    }

    private fun refreshOverFolder() {
        when (val getChildrenResult = CoreModel.getChildren(config, currentParent.id)) {
            is Ok -> {
                val tempFiles = getChildrenResult.value.filter { fileMetadata ->
                    fileMetadata.fileType == FileType.Folder && !ids.contains(fileMetadata.id)
                }.toMutableList()
                tempFiles.add(
                    0,
                    DecryptedFileMetadata(
                        id = "PARENT",
                        decryptedName = "..",
                        parent = "The parent file is ${currentParent.decryptedName}"
                    )
                )

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
        when (val getFileById = CoreModel.getFileById(config, currentParent.parent)) {
            is Ok -> currentParent = getFileById.value
            is Err -> _notifyError.postValue(getFileById.error.toLbError(getRes()))
        }.exhaustive
    }

    fun onItemClick(item: DecryptedFileMetadata) {
        viewModelScope.launch(Dispatchers.IO) {
            when (item.id) {
                "PARENT" -> {
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
