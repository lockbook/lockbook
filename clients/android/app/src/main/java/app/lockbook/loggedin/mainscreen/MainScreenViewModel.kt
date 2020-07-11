package app.lockbook.loggedin.mainscreen

import android.util.Log
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.core.getChildren
import app.lockbook.core.getFile
import app.lockbook.core.getFileMetadata
import app.lockbook.core.getRoot
import app.lockbook.loggedin.listfiles.ListFilesClickInterface
import app.lockbook.utils.Document
import app.lockbook.utils.FileMetadata
import app.lockbook.utils.FileType
import com.beust.klaxon.Klaxon
import kotlinx.coroutines.*

class MainScreenViewModel(path: String) : ViewModel(), ListFilesClickInterface {

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    private val _filesFolders = MutableLiveData<List<FileMetadata>>()
    private val _navigateToFileEditor = MutableLiveData<Document>()
    private val _navigateToPopUpInfo = MutableLiveData<FileMetadata>()
    private val _navigateToNewFileFolder = MutableLiveData<Boolean>()
    val fileFolderModel = FileFolderModel(path)

    val filesFolders: LiveData<List<FileMetadata>>
        get() = _filesFolders

    val navigateToFileEditor: LiveData<Document>
        get() = _navigateToFileEditor

    val navigateToPopUpInfo: LiveData<FileMetadata>
        get() = _navigateToPopUpInfo

    val navigateToNewFileFolder: LiveData<Boolean>
        get() = _navigateToNewFileFolder

    fun startListFilesFolders() {
        fileFolderModel.setParentToRoot()
        _filesFolders.postValue(fileFolderModel.getChildrenOfParent())
    }

    fun launchNewFileFolder() {
        _navigateToNewFileFolder.value = true
    }

    fun upADirectory() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                _filesFolders.postValue(fileFolderModel.getSiblingsOfParent())
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

    fun refreshFilesFolderList() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                _filesFolders.postValue(fileFolderModel.getChildrenOfParent())
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
                        _filesFolders.postValue(fileFolderModel.getChildrenOfParent())
                    } else {
                        _navigateToFileEditor.postValue(fileFolderModel.getFileDocument(item.id))
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