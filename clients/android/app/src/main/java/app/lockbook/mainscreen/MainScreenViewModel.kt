package app.lockbook.mainscreen

import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.utils.FileMetadata
import app.lockbook.core.getChildren
import app.lockbook.core.getRoot
import app.lockbook.mainscreen.listfiles.ListFilesClickInterface
import com.beust.klaxon.Klaxon
import kotlinx.coroutines.*
import app.lockbook.utils.FileType

class MainScreenViewModel(var path: String) : ViewModel(), ListFilesClickInterface {

    private var viewModelJob = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + viewModelJob)
    private val json = Klaxon()
    private val _filesFolders = MutableLiveData<List<FileMetadata>>()
    private val _navigateToFileEditor = MutableLiveData<FileMetadata>()

    val filesFolders: LiveData<List<FileMetadata>>
        get() = _filesFolders

    val navigateToFileEditor: LiveData<FileMetadata>
        get() = _navigateToFileEditor

    fun getRootMetadata() {
        uiScope.launch {
            getRoot()
        }
    }

    private suspend fun getRoot() {
        withContext(Dispatchers.IO) {
            val maybeRootSerialized = getRoot(path)
            print("Root Data: $maybeRootSerialized")
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
            print("Children Data: $childrenSerialized")
            val children: List<FileMetadata>? = json.parse(childrenSerialized)
            if(children == null) {
                _filesFolders.postValue(listOf())
            } else {
                _filesFolders.postValue(children)
            }
        }
    }

    override fun onItemClick(position: Int) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                val item = _filesFolders.value!![position]
                if(item.file_type == FileType.Folder) {
                    getChildren(item.id)
                } else {
                    _navigateToFileEditor.postValue(item)
                }
            }
        }
    }

    override fun onLongClick(position: Int) {
        TODO("Not yet implemented")
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