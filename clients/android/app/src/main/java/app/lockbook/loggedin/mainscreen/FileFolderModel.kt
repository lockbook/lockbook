package app.lockbook.loggedin.mainscreen

import app.lockbook.core.*
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result

class FileFolderModel(config: Config) {
    private val json = Klaxon()
    private val config = json.toJsonString(config)
    lateinit var parentFileMetadata: FileMetadata
    lateinit var lastDocumentAccessed: FileMetadata

    fun setParentToRoot(): Result<Unit, GetRootError> {
        val root: Result<FileMetadata, GetRootError>? = json.parse(getRoot(config))

        root?.let { rootResult ->
            return when (rootResult) {
                is Ok -> {
                    parentFileMetadata = rootResult.value
                    Ok(Unit)
                }
                is Err -> {
                    Err(rootResult.error)
                }
            }
        }

        return Err<GetRootError>(GetRootError.UnexpectedError("Unable to parse getRoot json!"))
    }

    fun getChildrenOfParent(): Result<List<FileMetadata>, GetChildrenError> { // return result instead
        val children: Result<List<FileMetadata>, GetChildrenError>? =
            json.parse(getChildren(config, parentFileMetadata.id))

        children?.let { childrenResult ->
            return when (childrenResult) {
                is Ok -> Ok(childrenResult.value.filter { fileMetadata -> fileMetadata.id != fileMetadata.parent })
                is Err -> Err(childrenResult.error)
            }
        }

        return Err<GetChildrenError>(GetChildrenError.UnexpectedError("Unable to parse getChildren json!"))
    }

    fun getSiblingsOfParent(): Result<List<FileMetadata>, GetChildrenError> { // return result instead
        val children: Result<List<FileMetadata>, GetChildrenError>? =
            json.parse(getChildren(config, parentFileMetadata.parent))

        children?.let { childrenResult ->
            return when (childrenResult) {
                is Ok -> {
                    val editedChildren =
                        childrenResult.value.filter { fileMetaData -> fileMetaData.id != fileMetaData.parent }
                    Ok(editedChildren)
                }
                is Err -> {
                    Err(childrenResult.error)
                }
            }
        }

        return Err<GetChildrenError>(GetChildrenError.UnexpectedError("Unable to parse getChildren json!"))
    }

    fun getParentOfParent(): Result<Unit, GetFileByIdError> {
        val parent: Result<FileMetadata, GetFileByIdError>? =
            json.parse(getFileById(config, parentFileMetadata.parent))

        parent?.let { parentResult ->
            return when (parentResult) {
                is Ok -> {
                    parentFileMetadata = parentResult.value
                    Ok(Unit)
                }
                is Err -> Err(parentResult.error)
            }
        }
        return Err<GetFileByIdError>(GetFileByIdError.UnexpectedError("Unable to parse getFileById json!"))
    }

    fun getDocumentContent(fileUuid: String): Result<String, ReadDocumentError> { // return result instead
        val document: Result<DecryptedValue, ReadDocumentError>? =
            json.parse(readDocument(config, fileUuid))

        document?.let { documentResult ->
            return when (documentResult) {
                is Ok -> Ok(documentResult.value.secret)
                is Err -> Err(documentResult.error)
            }
        }

        return Err<ReadDocumentError>(ReadDocumentError.UnexpectedError("Unable to parse readDocument json!"))
    }

    fun writeContentToDocument(content: String): Result<Unit, WriteToDocumentError> { // have a return type to be handled in case it doesnt work
        val write: Result<Unit, WriteToDocumentError>? = json.parse(
            writeDocument(
                config,
                lastDocumentAccessed.id,
                json.toJsonString(DecryptedValue(content))
            )
        )

        write?.let { writeResult ->
            return when (writeResult) {
                is Ok -> Ok(Unit)
                is Err -> Err(writeResult.error)
            }
        }

        return Err<WriteToDocumentError>(WriteToDocumentError.UnexpectedError("Unable to parse writeDocumentResult json!"))
    }

    fun createFile(
        name: String,
        fileType: String
    ): Result<FileMetadata, CreateFileError> {
        val file: Result<FileMetadata, CreateFileError>? =
            json.parse(createFile(config, parentFileMetadata.id, fileType, name))

        file?.let { createFileResult ->
            return when (createFileResult) {
                is Ok -> Ok(createFileResult.value)
                is Err -> Err(createFileResult.error)
            }
        }

        return Err<CreateFileError>(CreateFileError.UnexpectedError("Unable to parse createFile json!"))
    }

    fun insertFile(
        fileMetadata: FileMetadata
    ): Result<Unit, InsertFileError> {
        val insert: Result<Unit, InsertFileError>? =
            json.parse(insertFile(config, json.toJsonString(fileMetadata)))

        insert?.let { insertResult ->
            return when (insertResult) {
                is Ok -> Ok(insertResult.value)
                is Err -> Err(insertResult.error)
            }
        }

        return Err<InsertFileError>(InsertFileError.UnexpectedError("Unable to parse insertFile json!"))
    }

    fun renameFile(
        id: String,
        name: String
    ): Result<Unit, RenameFileError> {
        val rename: Result<Unit, RenameFileError>? =
            json.parse(renameFile(config, id, name))

        rename?.let { renameResult ->
            return when(renameResult) {
                is Ok -> Ok(renameResult.value)
                is Err -> Err(renameResult.error)
            }
        }

        return Err<RenameFileError>(RenameFileError.UnexpectedError("Unable to parse renameFile json"))
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