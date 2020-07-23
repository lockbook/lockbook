package app.lockbook.loggedin.mainscreen

import android.app.Activity.RESULT_OK
import android.content.Intent
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.loggedin.listfiles.ClickInterface
import app.lockbook.utils.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*

class MainScreenViewModel(path: String): ViewModel(), ClickInterface {

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    private val fileFolderModel = FileFolderModel(Config(path))

    private val _filesFolders = MutableLiveData<List<FileMetadata>>()
    private val _navigateToFileEditor = MutableLiveData<String>()
    private val _navigateToPopUpInfo = MutableLiveData<FileMetadata>()
    private val _navigateToNewFileFolder = MutableLiveData<Boolean>()
    private val _errorHasOccurred = MutableLiveData<String>()

    val filesFolders: LiveData<List<FileMetadata>>
        get() = _filesFolders

    val navigateToFileEditor: LiveData<String>
        get() = _navigateToFileEditor

    val navigateToPopUpInfo: LiveData<FileMetadata>
        get() = _navigateToPopUpInfo

    val navigateToNewFileFolder: LiveData<Boolean>
        get() = _navigateToNewFileFolder

    val errorHasOccurred: LiveData<String>
        get() = _errorHasOccurred

    fun startListFilesFolders() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                sync()
                startUpInRoot()
            }
        }
    }

    fun launchNewFileFolder() {
        _navigateToNewFileFolder.value = true
    }

    fun quitOrNot(): Boolean {
        if (fileFolderModel.parentFileMetadata.id == fileFolderModel.parentFileMetadata.parent) {
            return false
        }
        upADirectory()

        return true
    }

    private fun upADirectory() {
        when (val result = fileFolderModel.getSiblingsOfParent()) {
            is Ok -> {
                when (val innerResult = fileFolderModel.getParentOfParent()) {
                    is Ok -> _filesFolders.postValue(result.value)
                    is Err -> when (innerResult.error) {
                        is GetFileByIdError.NoFileWithThatId -> _errorHasOccurred.postValue("Error! No file with that id!")
                        is GetFileByIdError.UnexpectedError -> _errorHasOccurred.postValue("An unexpected error has occurred!")
                    }
                }

            }
            is Err -> _errorHasOccurred.postValue("An unexpected error has occurred!")
        }
    }

    private fun refreshFiles() {
        when (val children = fileFolderModel.getChildrenOfParent()) {
            is Ok -> _filesFolders.postValue(children.value)
            is Err -> _errorHasOccurred.postValue("An unexpected error has occurred!")
        }
    }

    private fun writeNewTextToDocument(content: String) {
        val writeResult = fileFolderModel.writeContentToDocument(content)
        if (writeResult is Err) {
            when (writeResult.error) {
                is WriteToDocumentError.FolderTreatedAsDocument -> _errorHasOccurred.postValue("Error! Folder is treated as document!")
                is WriteToDocumentError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                is WriteToDocumentError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is WriteToDocumentError.UnexpectedError -> _errorHasOccurred.postValue("An unexpected error has occurred!")
            }
        }
    }

    private fun createInsertRefreshFile(name: String, fileType: String) {
        when (val createFileResult = fileFolderModel.createFile(name, fileType)) {
            is Ok -> {
                val insertFileResult = fileFolderModel.insertFile(createFileResult.value)
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

    private fun renameRefreshFile(id: String, newName: String) {
        when (val renameFileResult = fileFolderModel.renameFile(id, newName)) {
            is Ok -> refreshFiles()
            is Err -> when (renameFileResult.error) {
                is RenameFileError.FileDoesNotExist -> _errorHasOccurred.postValue("Error! File does not exist!")
                is RenameFileError.NewNameContainsSlash -> _errorHasOccurred.postValue("Error! New name contains slash!")
                is RenameFileError.FileNameNotAvailable -> _errorHasOccurred.postValue("Error! File name not available!")
                is RenameFileError.UnexpectedError -> _errorHasOccurred.postValue("An unexpected error has occurred!")
            }
        }
    }

    private fun handleReadDocument(fileMetadata: FileMetadata) {
        when (val documentResult = fileFolderModel.getDocumentContent(fileMetadata.id)) {
            is Ok -> {
                _navigateToFileEditor.postValue(documentResult.value)
                fileFolderModel.lastDocumentAccessed = fileMetadata
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
        fileFolderModel.parentFileMetadata = fileMetadata
        refreshFiles()
    }

    private fun sync() {
        val syncAllResult = fileFolderModel.syncAllFiles()
        if (syncAllResult is Err) {
            when (syncAllResult.error) {
                is SyncAllError.NoAccount -> _errorHasOccurred.postValue("Error! No account!")
                is SyncAllError.CouldNotReachServer -> _errorHasOccurred.postValue("Error! Could not reach server!")
                is SyncAllError.UnexpectedError -> _errorHasOccurred.postValue("An unexpected error has occurred!")
            }
        }
    }

    private fun startUpInRoot() {
        when (val result = fileFolderModel.setParentToRoot()) {
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
                if (data is Intent && resultCode == RESULT_OK) {
                    when (requestCode) {
                        MainScreenFragment.NEW_FILE_REQUEST_CODE -> {
                            createInsertRefreshFile(data.getStringExtra("name"), data.getStringExtra("fileType"))
                        }
                        MainScreenFragment.TEXT_EDITOR_REQUEST_CODE -> {
                            writeNewTextToDocument(data.getStringExtra("text"))
                        }
                        MainScreenFragment.POP_UP_INFO_REQUEST_CODE -> {
                            renameRefreshFile(data.getStringExtra("id"), data.getStringExtra("new_name"))
                        }
                    }
                } else if (resultCode == RESULT_OK) {
                    _errorHasOccurred.postValue("An unexpected error has occurred!")
                }
            }
        }
    }

    override fun onItemClick(position: Int) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                _filesFolders.value?.let {
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
                _filesFolders.value?.let {
                    _navigateToPopUpInfo.postValue(it[position])
                }
            }
        }
    }
}