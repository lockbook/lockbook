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

        private const val GET_DOCUMENT_CONTENT_ERROR: String =
            "Couldn't get the contents of the document, please file a bug report in the settings."

        fun insertFileFolder(path: String, parentUuid: String, fileType: String, name: String) { // how do I get out the error
            val json = Klaxon()
            val fileFolder: Result<FileMetadata, CreateFileError>? = json.parse(createFile(path, parentUuid, fileType, name))

            fileFolder?.let {
                if(it is Ok<FileMetadata>) {
                    insertFile(path, json.toJsonString(it.value))
                }
            }
        }
    }

    fun setParentToRoot() {
        val root: Result<FileMetadata, GetRootError>? = json.parse(getRoot(path))

        root?.let {
            when (it) {
                is Ok -> parentFileMetadata = it.value
                is Err -> _unexpectedErrorOccurred.value = SET_PARENT_TO_ROOT_ERROR
            }
        }
        _unexpectedErrorOccurred.value = SET_PARENT_TO_ROOT_ERROR
    }

    fun getChildrenOfParent(): List<FileMetadata> {
        val children: Result<List<FileMetadata>, GetChildrenError>? =
            json.parse(getChildren(path, parentFileMetadata.id))

        if (children != null) {
            when (children) {
                is Ok -> children.value.filter { it.id != it.parent }
                is Err -> _unexpectedErrorOccurred.value = GET_CHILDREN_OF_PARENT_ERROR
            }
        }
        _unexpectedErrorOccurred.value = GET_CHILDREN_OF_PARENT_ERROR

        return listOf()
    }

    fun getSiblingsOfParent(): List<FileMetadata> {
        val children: Result<List<FileMetadata>, GetChildrenError>? =
            json.parse(getChildren(path, parentFileMetadata.parent))

        children?.let {
            when(it) {
                is Ok -> {
                    val editedChildren =
                        it.value.filter { fileMetaData -> fileMetaData.id != fileMetaData.parent }
                    getParentOfParent()
                    editedChildren
                }
                is Err -> _unexpectedErrorOccurred.value = GET_CHILDREN_OF_PARENT_ERROR
            }
        }
        _unexpectedErrorOccurred.value = GET_CHILDREN_OF_PARENT_ERROR

        return listOf()
    }

    private fun getParentOfParent() {
        val parent: Result<FileMetadata, GetFileByIdError>? =
            json.parse(getFileById(path, parentFileMetadata.parent))

        if (parent != null) {
            when (parent) {
                is Ok -> parentFileMetadata = parent.value
                is Err -> _unexpectedErrorOccurred.value = GET_FILE_BY_ID_ERROR
            }
        }

        _unexpectedErrorOccurred.value = GET_FILE_BY_ID_ERROR
    }

    fun getDocumentContent(fileUuid: String): String {
        val document: Result<DecryptedValue, ReadDocumentError>? =
            json.parse(readDocument(path, fileUuid))
        if (document != null) {
            when (document) {
                is Ok -> return document.value.secret
                is Err -> _unexpectedErrorOccurred.value = GET_DOCUMENT_CONTENT_ERROR
            }
        }
        _unexpectedErrorOccurred.value = GET_DOCUMENT_CONTENT_ERROR

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