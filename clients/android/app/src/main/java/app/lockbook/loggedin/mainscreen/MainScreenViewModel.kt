package app.lockbook.loggedin.mainscreen

import android.view.View
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import app.lockbook.core.getChildren
import app.lockbook.core.getFile
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
    var parentUuid: String = ""
    var visibleFAB = View.GONE


    val filesFolders: LiveData<List<FileMetadata>>
        get() = _filesFolders

    val navigateToFileEditor: LiveData<Document>
        get() = _navigateToFileEditor

    val navigateToPopUpInfo: LiveData<FileMetadata>
        get() = _navigateToPopUpInfo

    val navigateToNewFileFolder: LiveData<Boolean>
        get() = _navigateToNewFileFolder

    fun launchNewFileFolder() {
        _navigateToNewFileFolder.value = true
    }

    fun getRootMetadata() {
        uiScope.launch {
            getRoot()
        }
    }

    private suspend fun getRoot() {
        withContext(Dispatchers.IO) {
            val root: FileMetadata? = json.parse(getRoot(path))

            if (root == null) {
                _filesFolders.postValue(listOf())
            } else _filesFolders.postValue(listOf(root))
        }
    }

    private fun getChildrenMetadata(parentUuid: String) {
        uiScope.launch {
            getChildren(parentUuid)
        }
    }

    private suspend fun getChildren(parentUuid: String) {
        withContext(Dispatchers.IO) {
            val children: List<FileMetadata>? = json.parse(getChildren(path, parentUuid))

            if (children == null) {
                _filesFolders.postValue(listOf())
            } else {
                _filesFolders.postValue(children)
            }
        }
    }

    fun getFileDocument(fileUuid: String) {
        uiScope.launch {
            getFile(fileUuid)
        }
    }

    private suspend fun getFile(fileUuid: String) {
        withContext(Dispatchers.IO) {
            val file: Document? = json.parse(getFile(path, fileUuid))
            if(file != null) {
                _navigateToFileEditor.postValue(file)
            }
        }
    }

    override fun onItemClick(position: Int) {
            _filesFolders.value?.let {
                val item = it[position]
                parentUuid = item.id
                visibleFAB = View.VISIBLE

                if (item.file_type == FileType.Folder) {
                    getChildrenMetadata(item.id)
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