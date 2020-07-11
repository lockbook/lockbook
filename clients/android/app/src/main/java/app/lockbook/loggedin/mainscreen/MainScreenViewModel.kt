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

class MainScreenViewModel(private val path: String) : ViewModel(), ListFilesClickInterface {

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    private val json = Klaxon()
    private val _filesFolders = MutableLiveData<List<FileMetadata>>()
    private val _navigateToFileEditor = MutableLiveData<Document>()
    private val _navigateToPopUpInfo = MutableLiveData<FileMetadata>()
    private val _navigateToNewFileFolder = MutableLiveData<Boolean>()
    lateinit var parentFileMetadata: FileMetadata

    val filesFolders: LiveData<List<FileMetadata>>
        get() = _filesFolders

    val navigateToFileEditor: LiveData<Document>
        get() = _navigateToFileEditor

    val navigateToPopUpInfo: LiveData<FileMetadata>
        get() = _navigateToPopUpInfo

    val navigateToNewFileFolder: LiveData<Boolean>
        get() = _navigateToNewFileFolder

    fun startListFilesFolders() {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                getRootMetadata()
                getChildrenFileMetadata()
            }
        }
    }

    fun launchNewFileFolder() {
        _navigateToNewFileFolder.value = true
    }

    fun getRootMetadata() {
        getRoot()
    }

    private fun getRoot() {
        val root: FileMetadata? = json.parse(getRoot(path))

        if (root != null) {
            parentFileMetadata = root
        }

    }

    fun getChildrenFileMetadata() {
        getChildren(parentFileMetadata.id)

    }

    private fun getChildren(uuid: String) {
        val children: List<FileMetadata>? = json.parseArray(getChildren(path, uuid))

        if (children == null) {
            _filesFolders.postValue(listOf())
        } else {
            _filesFolders.postValue(children.filter {
                it.id != it.parent
            })
        }

    }

    fun getChildrenOfParentOfParentFileMetadata() {
        uiScope.launch {
            getChildren(parentFileMetadata.parent)
        }
    }

    private suspend fun getChildrenOfParent(uuid: String) {
        withContext(Dispatchers.IO) {
            val children: List<FileMetadata>? = json.parseArray(getChildren(path, uuid))

            if (children == null) {
                _filesFolders.postValue(listOf())
            } else {
                _filesFolders.postValue(children.filter {
                    it.id != it.parent
                })
                getParentOfParentFileMetadata()
            }
        }
    }

    private fun getFileDocument(fileUuid: String) {
        uiScope.launch {
            getFile(fileUuid)
        }
    }

    private suspend fun getFile(fileUuid: String) {
        withContext(Dispatchers.IO) {
            val file: Document? = json.parse(getFile(path, fileUuid))
            if (file != null) {
                _navigateToFileEditor.postValue(file)
            }
        }
    }

    fun getParentOfParentFileMetadata() {
        getParentOfParent()

    }

    private fun getParentOfParent() {
        val parent: FileMetadata? = json.parse(getFileMetadata(path, parentFileMetadata.parent))

        if (parent != null) {
            parentFileMetadata = parent
        }
    }

    override fun onItemClick(position: Int) {
        _filesFolders.value?.let {
            val item = it[position]

            if (item.file_type == FileType.Folder) {
                parentFileMetadata = item
                getChildrenFileMetadata()
            } else {
                getFileDocument(item.id)
            }
        }
    }

    override fun onLongClick(position: Int) {
        _filesFolders.value?.let {
            _navigateToPopUpInfo.value = it[position]
        }
    }

}