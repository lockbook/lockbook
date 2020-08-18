package app.lockbook.utils

import app.lockbook.core.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result

class CoreModel(config: Config) {
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

        return Err(GetRootError.UnexpectedError("getRootConverter was unable to be called!"))
    }

    fun getAccount(): Result<Account, GetAccountError> {
        val account: Result<Account, GetAccountError>? =
            Klaxon().converter(getAccountConverter).parse(getAccount(config))

        account?.let { accountResult ->
            return when (accountResult) {
                is Ok -> Ok(accountResult.value)
                is Err -> Err(accountResult.error)
            }
        }

        return Err(GetAccountError.UnexpectedError("getChildrenConverter was unable to be called!"))
    }

    fun setLastSynced(lastSyncedDuration: Long): Result<Unit, SetLastSyncedError> {
        val lastSynced: Result<Unit, SetLastSyncedError>? =
            Klaxon().converter(setLastSyncedConverter).parse(setLastSynced(config, lastSyncedDuration))

        lastSynced?.let { lastSyncedResult ->
            return when (lastSyncedResult) {
                is Ok -> Ok(lastSyncedResult.value)
                is Err -> Err(lastSyncedResult.error)
            }
        }

        return Err(SetLastSyncedError.UnexpectedError("setLastSyncedConverter was unable to be called!"))
    }

    fun getChildrenOfParent(): Result<List<FileMetadata>, GetChildrenError> {
        val children: Result<List<FileMetadata>, GetChildrenError>? =
            Klaxon().converter(getChildrenConverter)
                .parse(getChildren(config, parentFileMetadata.id))

        children?.let { childrenResult ->
            return when (childrenResult) {
                is Ok -> Ok(childrenResult.value.filter { fileMetadata -> fileMetadata.id != fileMetadata.parent && !fileMetadata.deleted })
                is Err -> Err(childrenResult.error)
            }
        }

        return Err(GetChildrenError.UnexpectedError("getChildrenConverter was unable to be called!"))
    }

    fun getSiblingsOfParent(): Result<List<FileMetadata>, GetChildrenError> {
        val children: Result<List<FileMetadata>, GetChildrenError>? =
            Klaxon().converter(getChildrenConverter)
                .parse(getChildren(config, parentFileMetadata.parent))

        children?.let { childrenResult ->
            return when (childrenResult) {
                is Ok -> {
                    val editedChildren =
                        childrenResult.value.filter { fileMetadata -> fileMetadata.id != fileMetadata.parent && !fileMetadata.deleted }
                    Ok(editedChildren)
                }
                is Err -> Err(childrenResult.error)
            }
        }

        return Err(GetChildrenError.UnexpectedError("getChildrenConverter was unable to be called!"))
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
        return Err(GetFileByIdError.UnexpectedError("getFileByIdConverter was unable to be called!"))
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

        return Err(ReadDocumentError.UnexpectedError("readDocumentConverter was unable to be called!"))
    }

    fun createFile(
        name: String,
        fileType: String
    ): Result<FileMetadata, CreateFileError> {
        val createFileResult: Result<FileMetadata, CreateFileError>? =
            Klaxon().converter(createFileConverter)
                .parse(createFile(config, name, parentFileMetadata.id, fileType))

        createFileResult?.let {
            return createFileResult
        }

        return Err(CreateFileError.UnexpectedError("createFileConverter was unable to be called!"))
    }

    fun insertFile(
        fileMetadata: FileMetadata
    ): Result<Unit, InsertFileError> {
        val insertResult: Result<Unit, InsertFileError>? =
            Klaxon().converter(insertFileConverter)
                .parse(insertFile(config, Klaxon().toJsonString(fileMetadata)))

        insertResult?.let {
            return insertResult
        }

        return Err(InsertFileError.UnexpectedError("insertFileConverter was unable to be called!"))
    }

    fun deleteFile(
        id: String
    ): Result<Unit, DeleteFileError> {
        val deleteFile: Result<Unit, DeleteFileError>? =
            Klaxon().converter(deleteFileConverter).parse(deleteFile(config, id))

        deleteFile?.let {
            return deleteFile
        }

        return Err(DeleteFileError.UnexpectedError("deleteFileConverter was unable to be called!"))
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

        return Err(RenameFileError.UnexpectedError("renameFileConverter was unable to be called!"))
    }

    fun moveFile(
        id: String,
        parentId: String
    ): Result<Unit, MoveFileError> {
        val moveResult: Result<Unit, MoveFileError>? =
            Klaxon().converter(moveFileConverter).parse(moveFile(config, id, parentId))

        moveResult?.let {
            return moveResult
        }

        return Err(MoveFileError.UnexpectedError("moveFileConverter was unable to be called!"))
    }

    fun calculateFileSyncWork(): Result<WorkCalculated, CalculateWorkError> {
        val calculateSyncWorkResult: Result<WorkCalculated, CalculateWorkError>? =
            Klaxon().converter(calculateSyncWorkConverter).parse(calculateSyncWork(config))

        calculateSyncWorkResult?.let {
            return calculateSyncWorkResult
        }

        return Err(CalculateWorkError.UnexpectedError("calculateSyncWorkConverter was unable to be called!"))
    }

    fun executeFileSyncWork(account: Account, workUnit: WorkUnit): Result<Unit, ExecuteWorkError> {
        val executeSyncWorkResult: Result<Unit, ExecuteWorkError>? =
            Klaxon().converter(executeSyncWorkConverter).parse(
                executeSyncWork(
                    config,
                    Klaxon().toJsonString(account),
                    Klaxon().toJsonString(workUnit)
                )
            )

        executeSyncWorkResult?.let {
            return executeSyncWorkResult
        }

        return Err(ExecuteWorkError.UnexpectedError("executeSyncWorkConverter was unable to be called!"))
    }

    companion object {
        fun generateAccount(config: Config, account: String): Result<Unit, CreateAccountError> {
            val createResult: Result<Unit, CreateAccountError>? =
                Klaxon().converter(createAccountConverter)
                    .parse(createAccount(Klaxon().toJsonString(config), account))

            createResult?.let {
                return createResult
            }

            return Err(CreateAccountError.UnexpectedError("createAccountConverter was unable to be called!"))
        }

        fun importAccount(config: Config, account: String): Result<Unit, ImportError> {
            val importResult: Result<Unit, ImportError>? =
                Klaxon().converter(importAccountConverter)
                    .parse(importAccount(Klaxon().toJsonString(config), account))

            importResult?.let {
                return importResult
            }

            return Err(ImportError.UnexpectedError("importAccountConverter was unable to be called!"))
        }

        fun exportAccount(config: Config): Result<String, AccountExportError> {
            val exportResult: Result<String, AccountExportError>? =
                Klaxon().converter(exportAccountConverter)
                    .parse(exportAccount(Klaxon().toJsonString(config)))

            exportResult?.let {
                return exportResult
            }

            return Err(AccountExportError.UnexpectedError("exportAccountConverter was unable to be called!"))
        }

        fun syncAllFiles(config: Config): Result<Unit, SyncAllError> {
            val syncResult: Result<Unit, SyncAllError>? =
                Klaxon().converter(syncAllConverter).parse(syncAll(Klaxon().toJsonString(config)))

            syncResult?.let {
                return syncResult
            }

            return Err(SyncAllError.UnexpectedError("syncAllConverter was unable to be called!"))
        }

        fun writeContentToDocument(
            config: Config,
            id: String,
            content: String
        ): Result<Unit, WriteToDocumentError> {
            val writeResult: Result<Unit, WriteToDocumentError>? =
                Klaxon().converter(writeDocumentConverter).parse(
                    writeDocument(
                        Klaxon().toJsonString(config),
                        id,
                        Klaxon().toJsonString(DecryptedValue(content))
                    )
                )

            writeResult?.let {
                return writeResult
            }

            return Err(WriteToDocumentError.UnexpectedError("writeDocument was unable to be called!"))
        }
    }
}
