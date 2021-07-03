package app.lockbook.model

import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.lockbook.App.Companion.config
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*

class MoveFileViewModel :
    ViewModel(),
    RegularClickInterface {
    private lateinit var currentParent: ClientFileMetadata
    lateinit var ids: Array<String>
    lateinit var names: Array<String>

    private val _files = MutableLiveData<List<ClientFileMetadata>>()
    private val _closeDialog = MutableLiveData<Unit>()
    private val _notifyError = SingleMutableLiveData<LbError>()
    private val _unexpectedErrorHasOccurred = SingleMutableLiveData<String>()

    val files: LiveData<List<ClientFileMetadata>>
        get() = _files

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
                is Err -> _notifyError.postValue(rootResult.error.toLbError())
            }.exhaustive
        }
    }

    fun moveFilesToFolder() {
        viewModelScope.launch(Dispatchers.IO) {
            var hasErrorOccurred = false
            for (id in ids) {
                val moveFileResult = moveFileIfSuccessful(id)
                if (!moveFileResult) {
                    hasErrorOccurred = !moveFileResult
                    break
                }
            }

            if (!hasErrorOccurred) {
                _closeDialog.postValue(Unit)
            }
        }
    }

    private fun moveFileIfSuccessful(id: String): Boolean {
        return when (val moveFileResult = CoreModel.moveFile(config, id, currentParent.id)) {
            is Ok -> true
            is Err -> {
                _notifyError.postValue(moveFileResult.error.toLbError())
                false
            }
        }.exhaustive
    }

    private fun refreshOverFolder() {
        when (val getChildrenResult = CoreModel.getChildren(config, currentParent.id)) {
            is Ok -> {
                val tempFiles = getChildrenResult.value.filter { fileMetadata -> fileMetadata.fileType == FileType.Folder && !ids.contains(fileMetadata.id) }.toMutableList()
                tempFiles.add(0, ClientFileMetadata(name = "..", parent = "The parent file is ${currentParent.name}"))
                _files.postValue(tempFiles)
            }
            is Err -> when (val error = getChildrenResult.error) {
                is GetChildrenError.Unexpected -> {
                    _unexpectedErrorHasOccurred.postValue(error.error)
                }
            }
        }.exhaustive
    }

    private fun setParentAsParent() {
        when (val getFileById = CoreModel.getFileById(config, currentParent.parent)) {
            is Ok -> currentParent = getFileById.value
            is Err -> _notifyError.postValue(getFileById.error.toLbError())
        }.exhaustive
    }

    override fun onItemClick(position: Int) {
        viewModelScope.launch(Dispatchers.IO) {
            _files.value?.let { files ->
                if (position == 0) {
                    setParentAsParent()
                    refreshOverFolder()
                } else {
                    currentParent = files[position]
                    refreshOverFolder()
                }
            }
        }
    }
}
