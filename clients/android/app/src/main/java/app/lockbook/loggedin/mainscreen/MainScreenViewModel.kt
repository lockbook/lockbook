package app.lockbook.loggedin.mainscreen

import android.app.Activity.RESULT_OK
import android.content.Intent
import android.util.Log
import android.widget.Toast
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.loggedin.listfiles.FilesFoldersClickInterface
import app.lockbook.loggedin.newfilefolder.NewFileFolderActivity
import app.lockbook.loggedin.popupinfo.PopUpInfoActivity
import app.lockbook.loggedin.texteditor.TextEditorActivity
import app.lockbook.utils.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*

class MainScreenViewModel(path: String) : ViewModel(), FilesFoldersClickInterface {

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    private val _filesFolders = MutableLiveData<List<FileMetadata>>()
    private val _navigateToFileEditor = MutableLiveData<String>()
    private val _navigateToPopUpInfo = MutableLiveData<FileMetadata>()
    private val _navigateToNewFileFolder = MutableLiveData<Boolean>()
    private val _errorHasOccurred = MutableLiveData<String>()
    private val fileFolderModel = FileFolderModel(Config(path))

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

    companion object {
        private const val SET_PARENT_TO_ROOT_ERROR =
            "Couldn't retrieve root, please file a bug report."
        private const val GET_PARENT_OF_PARENT_ERROR =
            "Couldn't get parent of parent, please file a bug report." // needs something more user friendly
        private const val GET_SIBLINGS_OF_PARENT_ERROR =
            "Couldn't retrieve the upper directory, please file a bug report."
        private const val REFRESH_CHILDREN_ERROR =
            "Couldn't refresh the files on screen, please file a bug report."
        private const val WRITE_NEW_TEXT_TO_DOCUMENT_ERROR =
            "Couldn't save your changes, please file a bug report."
        private const val ACCESS_DOCUMENT_ERROR =
            "Couldn't access the document, please file a bug report."
        private const val CREATE_FILE_ERROR =
            "Couldn't create the file, please file a bug report."
        private const val INSERT_FILE_ERROR =
            "Couldn't add file to DB, please file a bug report."
        private const val NEW_FILE_VIEW_ERROR =
            "Couldn't create a new file based on view, please file a bug report."
        private const val TEXT_EDITOR_VIEW_ERROR =
            "Couldn't make changes based on view, please file a bug report."
        private const val RENAME_VIEW_ERROR =
            "Couldn't rename the file based on view, please file a bug report."
    }

    fun startListFilesFolders() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                fileFolderModel.syncAllFiles()
                if (fileFolderModel.setParentToRoot() is Ok) {
                    refreshFiles()
                } else {
                    _errorHasOccurred.postValue(SET_PARENT_TO_ROOT_ERROR)
                }
            }
        }
    }

    fun launchNewFileFolder() {
        _navigateToNewFileFolder.value = true
    }

    private fun upADirectory() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                when (val siblings = fileFolderModel.getSiblingsOfParent()) {
                    is Ok -> {
                        when (fileFolderModel.getParentOfParent()) {
                            is Ok -> _filesFolders.postValue(siblings.value)
                            is Err -> _errorHasOccurred.postValue(GET_PARENT_OF_PARENT_ERROR)
                        }

                    }
                    is Err -> _errorHasOccurred.postValue(GET_SIBLINGS_OF_PARENT_ERROR)
                }
            }
        }
    }

    fun quitOrNot(): Boolean {
        if (fileFolderModel.parentFileMetadata.id == fileFolderModel.parentFileMetadata.parent) {
            return false
        }
        upADirectory()

        return true
    }

    private fun refreshFiles() {
        when (val children = fileFolderModel.getChildrenOfParent()) {
            is Ok -> _filesFolders.postValue(children.value)
            is Err -> _errorHasOccurred.postValue(REFRESH_CHILDREN_ERROR)
        }
    }

    private fun writeNewTextToDocument(content: String) {
        val writeResult = fileFolderModel.writeContentToDocument(content)
        if (writeResult is Err) {
            _errorHasOccurred.postValue(WRITE_NEW_TEXT_TO_DOCUMENT_ERROR)
        }
    }

    private fun createInsertFile(name: String, fileType: String) {
        when (val createFileResult = fileFolderModel.createFile(name, fileType)) {
            is Ok -> {
                val insertFileResult = fileFolderModel.insertFile(createFileResult.value)
                if (insertFileResult is Err) {
                    _errorHasOccurred.postValue(INSERT_FILE_ERROR)
                }
            }
            is Err -> _errorHasOccurred.postValue(CREATE_FILE_ERROR)
        }
    }

    private fun renameRefreshFile(id: String, newName: String) {
        fileFolderModel.renameFile(id, newName)
        refreshFiles()
    }

    //
//    fun syncInBackground() { // syncs in the background
//        uiScope.launch {
//            withContext(Dispatchers.IO) {
//                fileFolderModel.syncAll()
//            }
//        }
//    }
//
//    fun syncNextWork(): Int { // returns the number of work it is on
//        return fileFolderModel.doSyncWork(account)
//    }
//
//    fun startSyncWork(): Int { // returns the amount to complete
//        fileFolderModel.getAllSyncWork()
//        return fileFolderModel.allSyncWork.work_units.size
//    }
//

    fun handleActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                if (data is Intent && resultCode == RESULT_OK) {
                    when (requestCode) {
                        MainScreenFragment.NEW_FILE_REQUEST_CODE -> {
                            createInsertFile(
                                data.getStringExtra("name"),
                                data.getStringExtra("fileType")
                            )
                            refreshFiles()
                        }
                        MainScreenFragment.TEXT_EDITOR_REQUEST_CODE -> {
                            writeNewTextToDocument(data.getStringExtra("text"))
                        }
                        MainScreenFragment.POP_UP_INFO_REQUEST_CODE -> {
                            renameRefreshFile(
                                data.getStringExtra("id"),
                                data.getStringExtra("new_name")
                            )
                        }
                    }
                } else if (resultCode == RESULT_OK) {
                    when (requestCode) {
                        MainScreenFragment.NEW_FILE_REQUEST_CODE -> _errorHasOccurred.postValue(
                            NEW_FILE_VIEW_ERROR
                        )
                        MainScreenFragment.TEXT_EDITOR_REQUEST_CODE -> _errorHasOccurred.postValue(
                            TEXT_EDITOR_VIEW_ERROR
                        )
                        MainScreenFragment.POP_UP_INFO_REQUEST_CODE -> _errorHasOccurred.postValue(
                            RENAME_VIEW_ERROR
                        )

                    }
                }
            }
        }
    }

    override fun onItemClick(position: Int) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                _filesFolders.value?.let {
                    val item = it[position]

                    if (item.file_type == FileType.Folder) {
                        fileFolderModel.parentFileMetadata = item
                        refreshFiles()
                    } else {
                        when (val documentResult = fileFolderModel.getDocumentContent(item.id)) {
                            is Ok -> {
                                _navigateToFileEditor.postValue(documentResult.value)
                                fileFolderModel.lastDocumentAccessed = item
                            }
                            is Err -> _errorHasOccurred.postValue(ACCESS_DOCUMENT_ERROR)
                        }
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