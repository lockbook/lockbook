package app.lockbook.loggedin.mainscreen

import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.loggedin.listfiles.FilesFoldersClickInterface
import app.lockbook.utils.FileMetadata
import app.lockbook.utils.FileType
import kotlinx.coroutines.*

class MainScreenViewModel(val path: String) : ViewModel(), FilesFoldersClickInterface {

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    private val _filesFolders = MutableLiveData<List<FileMetadata>>()
    private val _navigateToFileEditor = MutableLiveData<String>()
    private val _navigateToPopUpInfo = MutableLiveData<FileMetadata>()
    private val _navigateToNewFileFolder = MutableLiveData<Boolean>()
    val fileFolderModel = FileFolderModel(path)

    val filesFolders: LiveData<List<FileMetadata>>
        get() = _filesFolders

    val navigateToFileEditor: LiveData<String>
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

    fun writeNewTextToDocument(content: String) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                fileFolderModel.writeContentToDocument(content)
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
                        _navigateToFileEditor.postValue(fileFolderModel.getDocumentContent(item.id))
                        fileFolderModel.lastDocumentAccessed = item
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