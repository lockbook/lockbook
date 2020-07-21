package app.lockbook.loggedin.mainscreen

import app.lockbook.core.*
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result

class FileFolderModel(config: Config) {
    private val config = Klaxon().toJsonString(config)
    lateinit var parentFileMetadata: FileMetadata
    lateinit var lastDocumentAccessed: FileMetadata

    fun setParentToRoot(): Result<Unit, GetRootError> {
        val root: Result<FileMetadata, GetRootError>? =
            Klaxon().converter(getRootConverter).parse(getRoot(config))

        root?.let { rootResult ->
            return when (rootResult) {
                is Ok -> {
                    parentFileMetadata = rootResult.value
                    Ok(Unit)
                }
                is Err -> Err(rootResult.error)
            }
        }

        return Err(GetRootError.UnexpectedError("Unable to parse getRoot json!"))
    }

    fun getChildrenOfParent(): Result<List<FileMetadata>, GetChildrenError> {
        val children: Result<List<FileMetadata>, GetChildrenError>? =
            Klaxon().converter(getChildrenConverter).parse(getChildren(config, parentFileMetadata.id))

        children?.let { childrenResult ->
            return when (childrenResult) {
                is Ok -> Ok(childrenResult.value.filter { fileMetadata -> fileMetadata.id != fileMetadata.parent })
                is Err -> Err(childrenResult.error)
            }
        }

        return Err(GetChildrenError.UnexpectedError("Unable to parse getChildren json!"))
    }

    fun getSiblingsOfParent(): Result<List<FileMetadata>, GetChildrenError> {
        val children: Result<List<FileMetadata>, GetChildrenError>? =
            Klaxon().converter(getChildrenConverter)
                .parse(getChildren(config, parentFileMetadata.parent))

        children?.let { childrenResult ->
            return when (childrenResult) {
                is Ok -> {
                    val editedChildren =
                        childrenResult.value.filter { fileMetaData -> fileMetaData.id != fileMetaData.parent }
                    Ok(editedChildren)
                }
                is Err -> Err(childrenResult.error)
            }
        }

        return Err(GetChildrenError.UnexpectedError("Unable to parse getChildren json!"))
    }

    fun getParentOfParent(): Result<Unit, GetFileByIdError> {
        val parent: Result<FileMetadata, GetFileByIdError>? =
            Klaxon().converter(
                getFileByIdConverter
            ).parse(getFileById(config, parentFileMetadata.parent))

        parent?.let { parentResult ->
            return when (parentResult) {
                is Ok -> {
                    parentFileMetadata = parentResult.value
                    Ok(Unit)
                }
                is Err -> Err(parentResult.error)
            }
        }
        return Err(GetFileByIdError.UnexpectedError("Unable to parse getFileById json!"))
    }

    fun getDocumentContent(fileUuid: String): Result<String, ReadDocumentError> { // return result instead
        val document: Result<DecryptedValue, ReadDocumentError>? =
            Klaxon().converter(readDocumentConverter).parse(readDocument(config, fileUuid))

        document?.let { documentResult ->
            return when (documentResult) {
                is Ok -> Ok(documentResult.value.secret)
                is Err -> Err(documentResult.error)
            }
        }

        return Err(ReadDocumentError.UnexpectedError("Unable to parse readDocument json!"))
    }

    fun writeContentToDocument(content: String): Result<Unit, WriteToDocumentError> {
        val writeResult: Result<Unit, WriteToDocumentError>? =
            Klaxon().converter(writeDocumentConverter).parse(
                writeDocument(
                    config,
                    lastDocumentAccessed.id,
                    Klaxon().toJsonString(DecryptedValue(content))
                )
            )

        writeResult?.let {
            return writeResult
        }

        return Err(WriteToDocumentError.UnexpectedError("Unable to parse writeDocument json!"))
    }

    fun createFile(
        name: String,
        fileType: String
    ): Result<FileMetadata, CreateFileError> {
        val createFileResult: Result<FileMetadata, CreateFileError>? =
            Klaxon().converter(createFileConverter).parse(createFile(config, name, parentFileMetadata.id, fileType))

        createFileResult?.let {
            return createFileResult
        }

        return Err(CreateFileError.UnexpectedError("Unable to parse createFile json!"))
    }

    fun insertFile(
        fileMetadata: FileMetadata
    ): Result<Unit, InsertFileError> {
        val insertResult: Result<Unit, InsertFileError>? =
            Klaxon().converter(insertFileConverter).parse(insertFile(config, Klaxon().toJsonString(fileMetadata)))
        insertResult?.let {
            return insertResult
        }

        return Err(InsertFileError.UnexpectedError("Unable to parse insertFile json!"))
    }

    fun renameFile(
        id: String,
        name: String
    ): Result<Unit, RenameFileError> {
        val renameResult: Result<Unit, RenameFileError>? =
            Klaxon().converter(renameFileConverter).parse(renameFile(config, id, name))

        renameResult?.let {
            return renameResult
        }

        return Err(RenameFileError.UnexpectedError("Unable to parse renameFile json!"))
    }

    fun syncAllFiles(): Result<Unit, SyncAllError> {
        val syncResult: Result<Unit, SyncAllError>? =
            Klaxon().converter(syncAllConverter).parse(syncAll(config))

        syncResult?.let {
            return syncResult
        }

        return Err(SyncAllError.UnexpectedError("Unable to parse syncAll json!"))
    }

    fun calculateFileSyncWork(): Result<WorkCalculated, CalculateWorkError> {
        val calculateSyncWorkResult: Result<WorkCalculated, CalculateWorkError>? =
            Klaxon().converter(calculateSyncWorkConverter).parse(calculateSyncWork(config))

        calculateSyncWorkResult?.let {
            return calculateSyncWorkResult
        }

        return Err(CalculateWorkError.UnexpectedError("Unable to parse calculateSyncWork json!"))
    }

    fun executeFileSyncWork(): Result<Unit, ExecuteWorkError> {
        val executeSyncWorkResult: Result<Unit, ExecuteWorkError>? =
            Klaxon().converter(calculateSyncWorkConverter).parse(calculateSyncWork(config))

        executeSyncWorkResult?.let {
            return executeSyncWorkResult
        }

        return Err(ExecuteWorkError.UnexpectedError("Unable to parse executeSyncWork json!"))
    }



}