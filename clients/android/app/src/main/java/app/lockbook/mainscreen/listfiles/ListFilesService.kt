package app.lockbook.mainscreen.listfiles

import androidx.lifecycle.MutableLiveData
import app.lockbook.utils.FileMetadata
import com.beust.klaxon.Klaxon
import kotlinx.coroutines.*

class ListFilesService(var path: String) {
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
            val maybeRootSerialized = app.lockbook.core.getRoot(path)
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
            val childrenSerialized = app.lockbook.core.getChildren(path, parentUuid)
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