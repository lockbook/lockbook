package app.lockbook.loggedin.mainscreen

import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.utils.FileMetadata
import app.lockbook.core.getChildren
import app.lockbook.core.getRoot
import app.lockbook.core.getFile
import app.lockbook.loggedin.listfiles.ListFilesClickInterface
import app.lockbook.utils.Document
import com.beust.klaxon.Klaxon
import kotlinx.coroutines.*
import app.lockbook.utils.FileType

class MainScreenViewModel(private val path: String) : ViewModel(), ListFilesClickInterface {

    private var viewModelJob = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + viewModelJob)
    private val json = Klaxon()
    private val _filesFolders = MutableLiveData<List<FileMetadata>>()
    private val _navigateToFileEditor = MutableLiveData<Document>()
    private val _navigateToPopUpInfo = MutableLiveData<FileMetadata>()

    val filesFolders: LiveData<List<FileMetadata>>
        get() = _filesFolders

    val navigateToFileEditor: LiveData<Document>
        get() = _navigateToFileEditor

    val navigateToPopUpInfo: LiveData<FileMetadata>
        get() = _navigateToPopUpInfo

    fun getRootMetadata() {
        uiScope.launch {
            getRoot()
        }
    }

    private suspend fun getRoot() {
        withContext(Dispatchers.IO) {
            val maybeRootSerialized = getRoot(path)
            val root: FileMetadata? = json.parse(maybeRootSerialized)
            if(root == null) {
                _filesFolders.postValue(listOf())
            } else {
                _filesFolders.postValue(listOf(root))
            }
        }
    }

    private suspend fun getChildren(parentUuid: String) {
        withContext(Dispatchers.IO) {
            val childrenSerialized = getChildren(path, parentUuid)
            val children: List<FileMetadata>? = json.parse(childrenSerialized)
            if(children == null) {
                _filesFolders.postValue(listOf())
            } else {
                _filesFolders.postValue(children)
            }
        }
    }

    private suspend fun getFile(fileUuid: String) {
        withContext(Dispatchers.IO) {
            val fileSerialized = getFile(path, fileUuid)
            val file: Document? = json.parse(fileSerialized)
            _navigateToFileEditor.postValue(file!!)
        }
    }

    override fun onItemClick(position: Int) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                val item = _filesFolders.value!![position]
                if(item.file_type == FileType.Folder) {
                    getChildren(item.id)
                } else {
                    getFile(item.id)
                }
            }
        }
    }

    override fun onLongClick(position: Int) {
        val item = _filesFolders.value!![position]
        _navigateToPopUpInfo.value = item
    }

//    fun getFileMetadata(fileUuid: String) {
//        uiScope.launch {
//            getFile(fileUuid)
//        }
//    }
//
//    private suspend fun getFileMetadata(fileUuid: String) {
//        withContext(Dispatchers.IO) {
//            val fileSerialized = getFile(path.absolutePath, fileUuid)
//            print("File Data: $fileSerialized")
//            val file: ClientFileMetadata = json.parse(fileSerialized)
//            if(file == null) {
//
//            }
//        }
//    }

}