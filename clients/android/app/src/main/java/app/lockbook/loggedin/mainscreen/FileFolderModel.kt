package app.lockbook.loggedin.mainscreen

import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import app.lockbook.core.*
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result

class FileFolderModel(private val path: String) {
    private val json = Klaxon()
    lateinit var parentFileMetadata: FileMetadata
    lateinit var lastDocumentAccessed: FileMetadata
    private val _unexpectedErrorOccurred = MutableLiveData<String>()

    val unexpectedErrorOccurred: LiveData<String>
        get() = _unexpectedErrorOccurred

    companion object {

        private const val SET_PARENT_TO_ROOT_ERROR: String =
            "Couldn't access root, please file a bug report in the settings."

        private const val GET_CHILDREN_OF_PARENT_ERROR: String =
            "Couldn't get access to the files and folders, please file a bug report in the settings."

        private const val GET_FILE_BY_ID_ERROR: String =
            "Couldn't get a file, please file a bug report in the settings."

        fun insertFileFolder(path: String, parentUuid: String, fileType: String, name: String) {
            val serializedFileFolder = createFile(path, parentUuid, fileType, name)

            val fileFolder: FileMetadata? = Klaxon().parse(serializedFileFolder)

            fileFolder?.let {
                insertFile(path, serializedFileFolder)
            }
        }
    }

    fun setParentToRoot() {
        val root: Result<FileMetadata, GetRootError>? = json.parse(getRoot(path))

        if (root != null) {
            when (root) {
                is Ok -> parentFileMetadata = root.value
                is Err -> _unexpectedErrorOccurred.value = SET_PARENT_TO_ROOT_ERROR
            }
        }
        _unexpectedErrorOccurred.value = SET_PARENT_TO_ROOT_ERROR
    }

    fun getChildrenOfParent(): List<FileMetadata> {
        val children: Result<List<FileMetadata>, GetChildrenError>? =
            json.parse(getChildren(path, parentFileMetadata.id))

        if (children != null) {
            return when (children) {
                is Ok -> children.value.filter { it.id != it.parent }
                is Err -> {
                    _unexpectedErrorOccurred.value = GET_CHILDREN_OF_PARENT_ERROR
                    listOf()
                }
            }
        }
        _unexpectedErrorOccurred.value = GET_CHILDREN_OF_PARENT_ERROR

        return listOf()
    }

    fun getSiblingsOfParent(): List<FileMetadata> {
        val children: Result<List<FileMetadata>, GetChildrenError>? =
            json.parse(getChildren(path, parentFileMetadata.parent))

        children?.let {
            return when (it) {
                is Ok -> {
                    val editedChildren =
                        it.value.filter { fileMetaData -> fileMetaData.id != fileMetaData.parent }
                    getParentOfParent()
                    editedChildren
                }
                is Err -> {
                    _unexpectedErrorOccurred.value = GET_CHILDREN_OF_PARENT_ERROR
                    listOf()
                }
            }

            return listOf()
        }

        return listOf()
    }

    private fun getParentOfParent() {
        val parent: Result<FileMetadata, GetFileByIdError>? = json.parse(getFileById(path, parentFileMetadata.parent))

        if (parent != null) {
            return when(parent) {
                is Ok -> parentFileMetadata = parent.value
                is Err -> _unexpectedErrorOccurred.value = GET_FILE_BY_ID_ERROR
            }
        }

        _unexpectedErrorOccurred.value = GET_FILE_BY_ID_ERROR
    }

    fun getDocumentContent(fileUuid: String): String {
        val document: DecryptedValue? = json.parse(readDocument(path, fileUuid))
        if (document != null) {
            return document.secret
        }

        return "" // definitely better way to handle this
    }

    fun writeContentToDocument(content: String) { // have a return type to be handled in case it doesnt work
        writeDocument(path, lastDocumentAccessed.id, json.toJsonString(DecryptedValue(content)))
    }

//    fun syncAll(): Int {
//        return sync(path)
//    }
//
//    fun getAllSyncWork() { // need to start using eithers
//        val tempAllSyncWork: WorkCalculated? = json.parse(calculateWork(path))
//
//        tempAllSyncWork?.let {
//            allSyncWork = it
//        }
//    }
//
//    fun doSyncWork(account: Account): Int {
//        val serializedAccount = json.toJsonString(account)
//        val serializedWork = json.toJsonString(allSyncWork.work_units[workNumber])
//
//        if (executeWork(path, serializedAccount, serializedWork) == 0 && workNumber != allSyncWork.work_units.size - 1) {
//            workNumber++
//            return workNumber
//        }
//
//        return workNumber
//    }

}