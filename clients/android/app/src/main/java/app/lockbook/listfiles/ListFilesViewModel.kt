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

    val filesFolders: LiveData<List<ClientFileMetadata>>
        get() = _filesFolders

    fun getRootFilesFolders() {
        uiScope.launch {
            getRoot()
        }
    }

    private suspend fun getRoot() {
        withContext(Dispatchers.IO) {
            val maybeRootSerialized = getRoot(path.absolutePath)
            val root: List<ClientFileMetadata> = listOf((Klaxon().parse(maybeRootSerialized)!!)!!)
            _filesFolders.value = root
        }
    }

    fun getChildrenFilesFolders(parentUuid: String) {
        uiScope.launch {
            getChildren(parentUuid)
        }
    }

    private suspend fun getChildren(parentUuid: String) {
        withContext(Dispatchers.IO) {
            val childrenSerialized = getChildren(path.absolutePath, parentUuid)
            val children: List<ClientFileMetadata> = Klaxon().parse(childrenSerialized)!!
            _filesFolders.value = children
        }
    }

    fun getFileFilesFolders(fileUuid: String) {

    }

    init {
        getRootFilesFolders()
    }


}