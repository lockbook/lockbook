package app.lockbook.loggedin.listfiles

import android.app.Activity.RESULT_CANCELED
import android.content.Intent
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.utils.*
import app.lockbook.utils.ClickInterface
import app.lockbook.utils.RequestResultCodes.DELETE_RESULT_CODE
import app.lockbook.utils.RequestResultCodes.NEW_FILE_REQUEST_CODE
import app.lockbook.utils.RequestResultCodes.POP_UP_INFO_REQUEST_CODE
import app.lockbook.utils.RequestResultCodes.RENAME_RESULT_CODE
import app.lockbook.utils.RequestResultCodes.TEXT_EDITOR_REQUEST_CODE
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*

class ListFilesViewModel(path: String) :
    ViewModel(),
    ClickInterface {

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    private val coreModel = CoreModel(Config(path))

    private val _files = MutableLiveData<List<FileMetadata>>()
    private val _navigateToFileEditor = MutableLiveData<String>()
    private val _navigateToPopUpInfo = MutableLiveData<FileMetadata>()
    private val _navigateToNewFile = MutableLiveData<Unit>()
    private val _listFilesRefreshing = MutableLiveData<Boolean>()
    private val _errorHasOccurred = MutableLiveData<String>()

    val files: LiveData<List<FileMetadata>>
        get() = _files

    val navigateToFileEditor: LiveData<String>
        get() = _navigateToFileEditor

    val navigateToPopUpInfo: LiveData<FileMetadata>
        get() = _navigateToPopUpInfo

    val navigateToNewFile: LiveData<Unit>
        get() = _navigateToNewFile

    val listFilesRefreshing: LiveData<Boolean>
        get() = _listFilesRefreshing

    val errorHasOccurred: LiveData<String>
        get() = _errorHasOccurred

    fun startUpFiles() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                sync()
                startUpInRoot()
            }
        }
    }

    fun launchNewFileActivity() {
        _navigateToNewFile.value = Unit
    }

    fun quitOrNot(): Boolean {
        if (coreModel.parentFileMetadata.id == coreModel.parentFileMetadata.parent) {
            return false
        }
        upADirectory()

        return true
    }

    private fun upADirectory() {
        when (val getSiblingsOfParentResult = coreModel.getSiblingsOfParent()) {
            is Ok -> {
                when (val getParentOfParentResult = coreModel.getParentOfParent()) {
                    is Ok -> _files.postValue(getSiblingsOfParentResult.value)
                    is Err -> when (getParentOfParentResult.error) {
                        is GetFileByIdError.NoFileWithThatId -> _errorHasOccurred.postValue("Error! No file with that id!")
                        is GetFileByIdError.UnexpectedError -> _errorHasOccurred.postValue("An unexpected error has occurred!")
                    }
                }
            }
            is Err -> _errorHasOccurred.postValue("An unexpected error has occurred!")
        }
    }

    private fun refreshFiles() {
        when (val getChildrenResult = coreModel.getChildrenOfParent()) {
            is Ok -> _files.postValue(getChildrenResult.value)
            is Err -> _errorHasOccurred.postValue("An unexpected error has occurred!")
        }
    }

    private fun writeNewTextToDocument(content: String) {
        val writeToDocumentResult = coreModel.writeContentToDocument(content)
        if (writeToDocumentResult is Err) {
            when (writeToDocumentResult.error) {
                is WriteToDocumentError.FolderTreatedAsDocument -> _errorHasOccurred.postValue("Error! Folder is treated as document!")
                is WriteToDocumentError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                is WriteToDocumentError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is WriteToDocumentError.UnexpectedError -> _errorHasOccurred.postValue("An unexpected error has occurred!")
            }
        }
    }

    private fun createInsertRefreshFiles(name: String, fileType: String) {
        when (val createFileResult = coreModel.createFile(name, fileType)) {
            is Ok -> {
                val insertFileResult = coreModel.insertFile(createFileResult.value)
                if (insertFileResult is Err) {
                    _errorHasOccurred.postValue("An unexpected error has occurred!")
                }
                refreshFiles()
            }
            is Err -> when (createFileResult.error) {
                is CreateFileError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is CreateFileError.DocumentTreatedAsFolder -> _errorHasOccurred.postValue("Error! Document is treated as folder!")
                is CreateFileError.CouldNotFindAParent -> _errorHasOccurred.postValue("Error! Could not find file parent!")
                is CreateFileError.FileNameNotAvailable -> _errorHasOccurred.postValue("Error! File name not available!")
                is CreateFileError.FileNameContainsSlash -> _errorHasOccurred.postValue("Error! File contains a slash!")
                is CreateFileError.UnexpectedError -> _errorHasOccurred.postValue("An unexpected error has occurred!")
            }
        }
    }

    private fun renameRefreshFiles(id: String, newName: String) {
        when (val renameFileResult = coreModel.renameFile(id, newName)) {
            is Ok -> refreshFiles()
            is Err -> when (renameFileResult.error) {
                is RenameFileError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                is RenameFileError.NewNameContainsSlash -> _errorHasOccurred.postValue("Error! New name contains slash!")
                is RenameFileError.FileNameNotAvailable -> _errorHasOccurred.postValue("Error! File name not available!")
                is RenameFileError.UnexpectedError -> _errorHasOccurred.postValue("An unexpected error has occurred!")
            }
        }
    }

    private fun deleteRefreshFiles(id: String) {
        when (val deleteFileResult = coreModel.deleteFile(id)) {
            is Ok -> refreshFiles()
            is Err -> when (deleteFileResult.error) {
                is DeleteFileError.NoFileWithThatId -> _errorHasOccurred.postValue("Error! No file with that id!")
                is DeleteFileError.UnexpectedError -> _errorHasOccurred.postValue("An unexpected error has occurred!")
            }
        }
    }

    private fun handleReadDocument(fileMetadata: FileMetadata) {
        when (val documentResult = coreModel.getDocumentContent(fileMetadata.id)) {
            is Ok -> {
                _navigateToFileEditor.postValue(documentResult.value)
                coreModel.lastDocumentAccessed = fileMetadata
            }
            is Err -> when (documentResult.error) {
                is ReadDocumentError.TreatedFolderAsDocument -> _errorHasOccurred.postValue("Error! Folder treated as document!")
                is ReadDocumentError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is ReadDocumentError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                is ReadDocumentError.UnexpectedError -> _errorHasOccurred.postValue("An unexpected error has occurred!")
            }
        }
    }

    private fun intoFolder(fileMetadata: FileMetadata) {
        coreModel.parentFileMetadata = fileMetadata
        refreshFiles()
    }

    private fun sync() {
        val syncAllResult = coreModel.syncAllFiles()
        if (syncAllResult is Err) {
            when (syncAllResult.error) {
                is SyncAllError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is SyncAllError.CouldNotReachServer -> _errorHasOccurred.postValue("Error! Could not reach server!")
                is SyncAllError.UnexpectedError -> _errorHasOccurred.postValue("An unexpected error has occurred!")
            }
        }
    }

    private fun startUpInRoot() {
        when (val result = coreModel.setParentToRoot()) {
            is Ok -> refreshFiles()
            is Err -> when (result.error) {
                is GetRootError.NoRoot -> _errorHasOccurred.postValue("No root!")
                is GetRootError.UnexpectedError -> _errorHasOccurred.postValue("An unexpected error has occurred!")
            }
        }
    }

    fun handleActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                if (data is Intent) {
                    when (requestCode) {
                        NEW_FILE_REQUEST_CODE -> {
                            handleNewFileRequest(data)
                        }
                        TEXT_EDITOR_REQUEST_CODE -> {
                            handleTextEditorRequest(data)
                        }
                        POP_UP_INFO_REQUEST_CODE -> {
                            handlePopUpInfoRequest(resultCode, data)
                        }
                    }
                } else if (resultCode != RESULT_CANCELED) {
                    _errorHasOccurred.postValue("An unexpected error has occurred!")
                }
            }
        }
    }

    private fun handleNewFileRequest(data: Intent) {
        val name = data.getStringExtra("name")
        val fileType = data.getStringExtra("fileType")
        if (name != null && fileType != null) {
            createInsertRefreshFiles(name, fileType)
        } else {
            _errorHasOccurred.postValue("An unexpected error has occurred!")
        }
    }

    private fun handleTextEditorRequest(data: Intent) {
        val text = data.getStringExtra("text")
        if (text != null) {
            writeNewTextToDocument(text)
        } else {
            _errorHasOccurred.postValue("An unexpected error has occurred!")
        }
    }

    private fun handlePopUpInfoRequest(resultCode: Int, data: Intent) {
        if (resultCode == RENAME_RESULT_CODE) {
            val id = data.getStringExtra("id")
            val newName = data.getStringExtra("new_name")
            if (id != null && newName != null) {
                renameRefreshFiles(id, newName)
            } else {
                _errorHasOccurred.postValue("An unexpected error has occurred!")
            }
        } else if (resultCode == DELETE_RESULT_CODE) {
            val id = data.getStringExtra("id")
            if (id != null) {
                deleteRefreshFiles(id)
            }
        }
    }

    fun syncRefresh() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                sync()
                refreshFiles()
                _listFilesRefreshing.postValue(false)
            }
        }
    }

    override fun onItemClick(position: Int) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                _files.value?.let {
                    val fileMetadata = it[position]

                    if (fileMetadata.file_type == FileType.Folder) {
                        intoFolder(fileMetadata)
                    } else {
                        handleReadDocument(fileMetadata)
                    }
                }
            }
        }
    }

    override fun onLongClick(position: Int) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                _files.value?.let {
                    _navigateToPopUpInfo.postValue(it[position])
                }
            }
        }
    }
}
