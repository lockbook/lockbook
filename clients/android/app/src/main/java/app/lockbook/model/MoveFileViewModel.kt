package app.lockbook.model

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*

class MoveFileViewModel(path: String, application: Application) :
    AndroidViewModel(application),
    RegularClickInterface {

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    private val config = Config(path)
    lateinit var currentParentId: String

    private val _files = MutableLiveData<List<FileMetadata>>()
    private val _errorHasOccurred = SingleMutableLiveData<String>()
    private val _unexpectedErrorHasOccurred = SingleMutableLiveData<String>()

    val files: LiveData<List<FileMetadata>>
        get() = _files

    val errorHasOccurred: LiveData<String>
        get() = _errorHasOccurred

    val unexpectedErrorHasOccurred: LiveData<String>
        get() = _unexpectedErrorHasOccurred

    fun startInRoot() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                when (val rootResult = CoreModel.getRoot(config)) {
                    is Ok -> {
                        currentParentId = rootResult.value.id
                        refreshOverFolder(rootResult.value.id)
                    }
                    is Err -> when (val error = rootResult.error) {
                        is GetRootError.NoRoot -> _errorHasOccurred.postValue("Error! No root!")
                        is GetRootError.Unexpected -> _unexpectedErrorHasOccurred.postValue(error.error)
                    }
                }.exhaustive
            }
        }
    }

    fun moveFilesToFolder(ids: Array<String>) {
        for(id in ids) {
            moveFileRefresh(id)
        }
    }

    private fun moveFileRefresh(id: String) {
        when(val moveFileResult = CoreModel.moveFile(config, id, currentParentId)) {
            is Ok -> {}
            is Err -> when(val error = moveFileResult.error) {
                MoveFileError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                MoveFileError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                MoveFileError.DocumentTreatedAsFolder -> _errorHasOccurred.postValue("Error! Document treated as folder!")
                MoveFileError.TargetParentDoesNotExist -> _errorHasOccurred.postValue("Error! The parent file does not exist!")
                MoveFileError.TargetParentHasChildNamedThat -> _errorHasOccurred.postValue("Error! The parent file has a child called that already!")
                MoveFileError.CannotMoveRoot -> _errorHasOccurred.postValue("Error! You cannot move to root!")
                is MoveFileError.Unexpected -> _unexpectedErrorHasOccurred.postValue(error.error)
            }
        }.exhaustive
    }

    private fun refreshOverFolder(parentId: String) {
        when (val getChildrenResult = CoreModel.getChildren(config, parentId)) {
            is Ok -> _files.postValue(getChildrenResult.value.filter { fileMetadata -> fileMetadata.fileType == FileType.Folder })
            is Err -> when (val error = getChildrenResult.error) {
                is GetChildrenError.Unexpected -> {
                    _unexpectedErrorHasOccurred.postValue(error.error)
                }
            }
        }.exhaustive
    }

    override fun onItemClick(position: Int) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                _files.value?.let { files ->
                    currentParentId = files[position].id
                    refreshOverFolder(files[position].id)
                }
            }
        }
    }
}
