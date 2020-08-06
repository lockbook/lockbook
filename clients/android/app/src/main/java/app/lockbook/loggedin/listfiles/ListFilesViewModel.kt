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
import timber.log.Timber

class ListFilesViewModel(path: String) :
    ViewModel(),
    ClickInterface {

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    private val coreModel = CoreModel(Config(path))

    private val _files = MutableLiveData<List<FileMetadata>>()
    private val _navigateToFileEditor = MutableLiveData<EditableFile>()
    private val _navigateToPopUpInfo = MutableLiveData<FileMetadata>()
    private val _navigateToNewFile = MutableLiveData<Unit>()
    private val _listFilesRefreshing = MutableLiveData<Boolean>()
    private val _errorHasOccurred = MutableLiveData<String>()

    val files: LiveData<List<FileMetadata>>
        get() = _files

    val navigateToFileEditor: LiveData<EditableFile>
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
                    is Ok -> sortFiles(getSiblingsOfParentResult.value)
                    is Err -> when (val error = getParentOfParentResult.error) {
                        is GetFileByIdError.NoFileWithThatId -> _errorHasOccurred.postValue("Error! No file with that id!")
                        is GetFileByIdError.UnexpectedError -> {
                            Timber.e("Unable to get the parent of the current path: ${error.error}")
                            _errorHasOccurred.postValue(
                                UNEXPECTED_ERROR_OCCURRED
                            )
                        }
                    }
                }
            }
            is Err -> {
                Timber.e("Unable to get siblings of the parent: ${getSiblingsOfParentResult.error}")
                _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
            }
        }
    }

    private fun refreshFiles() {
        when (val getChildrenResult = coreModel.getChildrenOfParent()) {
            is Ok -> sortFiles(getChildrenResult.value)
            is Err -> {
                Timber.e("Unable to get children: ${getChildrenResult.error}")
                _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
            }
        }
    }

    private fun writeNewTextToDocument(content: String) {
        Timber.i("HERE1")
        val writeToDocumentResult = coreModel.writeContentToDocument(content)
        Timber.i("HERE2")
        if (writeToDocumentResult is Err) {
            when (val error = writeToDocumentResult.error) {
                is WriteToDocumentError.FolderTreatedAsDocument -> _errorHasOccurred.postValue("Error! Folder is treated as document!")
                is WriteToDocumentError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                is WriteToDocumentError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is WriteToDocumentError.UnexpectedError -> {
                    Timber.e("Unable to write document changes: ${error.error}")
                    _errorHasOccurred.postValue(
                        UNEXPECTED_ERROR_OCCURRED
                    )
                }
            }
        }
    }

    private fun createInsertRefreshFiles(name: String, fileType: String) {
        when (val createFileResult = coreModel.createFile(name, fileType)) {
            is Ok -> {
                val insertFileResult = coreModel.insertFile(createFileResult.value)
                if (insertFileResult is Err) {
                    Timber.e("Unable to insert a newly created file: ${insertFileResult.error}")
                    _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
                }
                refreshFiles()
            }
            is Err -> when (val error = createFileResult.error) {
                is CreateFileError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is CreateFileError.DocumentTreatedAsFolder -> _errorHasOccurred.postValue("Error! Document is treated as folder!")
                is CreateFileError.CouldNotFindAParent -> _errorHasOccurred.postValue("Error! Could not find file parent!")
                is CreateFileError.FileNameNotAvailable -> _errorHasOccurred.postValue("Error! File name not available!")
                is CreateFileError.FileNameContainsSlash -> _errorHasOccurred.postValue("Error! File contains a slash!")
                is CreateFileError.UnexpectedError -> {
                    Timber.e("Unable to create a file: ${error.error}")
                    _errorHasOccurred.postValue(
                        UNEXPECTED_ERROR_OCCURRED
                    )
                }
            }
        }
    }

    private fun renameRefreshFiles(id: String, newName: String) {
        when (val renameFileResult = coreModel.renameFile(id, newName)) {
            is Ok -> refreshFiles()
            is Err -> when (val error = renameFileResult.error) {
                is RenameFileError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                is RenameFileError.NewNameContainsSlash -> _errorHasOccurred.postValue("Error! New name contains slash!")
                is RenameFileError.FileNameNotAvailable -> _errorHasOccurred.postValue("Error! File name not available!")
                is RenameFileError.UnexpectedError -> {
                    Timber.e("Unable to rename file: ${error.error}")
                    _errorHasOccurred.postValue(
                        UNEXPECTED_ERROR_OCCURRED
                    )
                }
            }
        }
    }

    private fun deleteRefreshFiles(id: String) {
        when (val deleteFileResult = coreModel.deleteFile(id)) {
            is Ok -> refreshFiles()
            is Err -> when (val error = deleteFileResult.error) {
                is DeleteFileError.NoFileWithThatId -> _errorHasOccurred.postValue("Error! No file with that id!")
                is DeleteFileError.UnexpectedError -> {
                    Timber.e("Unable to delete file: ${error.error}")
                    _errorHasOccurred.postValue(
                        UNEXPECTED_ERROR_OCCURRED
                    )
                }
            }
        }
    }

    private fun sortFiles(files: List<FileMetadata>) {
        val sortedFiles = files.sortedBy { fileMetadata ->
            fileMetadata.name
        }
        if (sortedFiles == files) {
            _files.postValue(
                files.sortedByDescending { fileMetadata ->
                    fileMetadata.name
                }
            )
        } else {
            _files.postValue(sortedFiles)
        }
    }

    private fun handleReadDocument(fileMetadata: FileMetadata) {
        when (val documentResult = coreModel.getDocumentContent(fileMetadata.id)) {
            is Ok -> {
                _navigateToFileEditor.postValue(EditableFile(fileMetadata.name, documentResult.value))
                coreModel.lastDocumentAccessed = fileMetadata
            }
            is Err -> when (val error = documentResult.error) {
                is ReadDocumentError.TreatedFolderAsDocument -> _errorHasOccurred.postValue("Error! Folder treated as document!")
                is ReadDocumentError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is ReadDocumentError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                is ReadDocumentError.UnexpectedError -> {
                    Timber.e("Unable to get content of file: ${error.error}")
                    _errorHasOccurred.postValue(
                        UNEXPECTED_ERROR_OCCURRED
                    )
                }
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
            when (val error = syncAllResult.error) {
                is SyncAllError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is SyncAllError.CouldNotReachServer -> _errorHasOccurred.postValue("Error! Could not reach server!")
                is SyncAllError.ExecuteWorkError -> { // more will be done about this since it can send a wide variety of errors
                    _errorHasOccurred.postValue("Unable to sync work.")
                }
                is SyncAllError.UnexpectedError -> {
                    Timber.e("Unable to sync all files: ${error.error}")
                    _errorHasOccurred.postValue(
                        UNEXPECTED_ERROR_OCCURRED
                    )
                }
            }
        }
    }

    private fun startUpInRoot() {
        when (val result = coreModel.setParentToRoot()) {
            is Ok -> refreshFiles()
            is Err -> when (val error = result.error) {
                is GetRootError.NoRoot -> _errorHasOccurred.postValue("No root!")
                is GetRootError.UnexpectedError -> {
                    Timber.e("Unable to set parent to root: ${error.error}")
                    _errorHasOccurred.postValue(
                        UNEXPECTED_ERROR_OCCURRED
                    )
                }
            }
        }
    }

    fun handleActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                if (data is Intent) {
                    when (requestCode) {
                        NEW_FILE_REQUEST_CODE -> handleNewFileRequest(data)
                        TEXT_EDITOR_REQUEST_CODE -> handleTextEditorRequest(data)
                        POP_UP_INFO_REQUEST_CODE -> handlePopUpInfoRequest(resultCode, data)
                    }
                } else if (resultCode != RESULT_CANCELED) {
                    Timber.e("Unable to recognize resultCode.")
                    _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
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
            Timber.e("Name or fileType is null.")
            _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
        }
    }

    private fun handleTextEditorRequest(data: Intent) {
        val contents = data.getStringExtra("contents")
        if (contents != null) {
            writeNewTextToDocument(contents)
        } else {
            Timber.e("contents is null.")
            _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
        }
    }

    private fun handlePopUpInfoRequest(resultCode: Int, data: Intent) {
        val id = data.getStringExtra("id")
        if (id is String) {
            when (resultCode) {
                RENAME_RESULT_CODE -> {
                    val newName = data.getStringExtra("new_name")
                    if (newName != null) {
                        renameRefreshFiles(id, newName)
                    } else {
                        Timber.e("newName is null.")
                        _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
                    }
                }
                DELETE_RESULT_CODE -> deleteRefreshFiles(id)
                else -> {
                    Timber.e("Unrecognized result code.")
                    _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
                }
            }
        } else {
            Timber.e("id is null.")
            _errorHasOccurred.postValue(UNEXPECTED_ERROR_OCCURRED)
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

    fun onSortPressed() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                val files = _files.value
                if (files is List<FileMetadata>) {
                    sortFiles(files)
                }
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
