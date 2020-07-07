package app.lockbook.listfiles

import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.FileMetadata
import app.lockbook.core.getChildren
import app.lockbook.core.getRoot
import com.beust.klaxon.Klaxon
import kotlinx.coroutines.*

class ListFilesViewModel(var path: String) : ViewModel() {

    private var viewModelJob = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + viewModelJob)
    private val json = Klaxon()
    val filesFolders = MutableLiveData<List<FileMetadata>>()

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
                filesFolders.postValue(listOf())
            } else {

                filesFolders.postValue(listOf(root))
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
            val childrenSerialized = getChildren(path, parentUuid)
            print("Children Data: $childrenSerialized")
            val children: List<FileMetadata>? = json.parse(childrenSerialized)
            if(children == null) {
                filesFolders.postValue(listOf())
            } else {
                filesFolders.postValue(children)
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

}