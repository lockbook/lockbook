package app.lockbook.listfiles

import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.ClientFileMetadata
import app.lockbook.core.getChildren
import app.lockbook.core.getRoot
import com.beust.klaxon.Klaxon
import kotlinx.coroutines.*
import java.io.File

class ListFilesViewModel(var path: File) : ViewModel() {

    private var viewModelJob = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + viewModelJob)
    private val _filesFolders = MutableLiveData<List<ClientFileMetadata>>()
    private val json = Klaxon()

    val filesFolders: LiveData<List<ClientFileMetadata>>
        get() = _filesFolders

    fun getRootMetadata() {
        uiScope.launch {
            getRoot()
        }
    }

    private suspend fun getRoot() {
        withContext(Dispatchers.IO) {
            val maybeRootSerialized = getRoot(path.absolutePath)
            print("Root Data: $maybeRootSerialized")
            val root: ClientFileMetadata? = json.parse(maybeRootSerialized)
            if(root == null) {
                _filesFolders.value = listOf()
            } else {
                _filesFolders.value = listOf(root)
            }
        }
    }

    fun getChildrenMetadata(parentUuid: String) {
        uiScope.launch {
            getChildren(parentUuid)
        }
    }

    private suspend fun getChildren(parentUuid: String) {
        withContext(Dispatchers.IO) {
            val childrenSerialized = getChildren(path.absolutePath, parentUuid)
            print("Children Data: $childrenSerialized")
            val children: List<ClientFileMetadata>? = json.parse(childrenSerialized)
            if(children == null) {
                _filesFolders.value = listOf()
            } else {
                _filesFolders.value = children
            }
        }
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

    init {
        getRootMetadata()
    }

}