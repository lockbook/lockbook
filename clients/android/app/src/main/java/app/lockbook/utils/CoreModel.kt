package app.lockbook.utils

import app.lockbook.core.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Result

object CoreModel {
    fun setUpInitLogger(path: String): Result<Unit, InitLoggerError> {
        val initLoggerResult: Result<Unit, InitLoggerError>? =
            Klaxon().converter(initLoggerConverter)
                .parse(initLogger(path))

        if (initLoggerResult != null) {
            return initLoggerResult
        }

        return Err(InitLoggerError.Unexpected("initLoggerConverter was unable to be called!"))
    }

    fun generateAccount(config: Config, account: String): Result<Unit, CreateAccountError> {
        val createAccountResult: Result<Unit, CreateAccountError>? =
            Klaxon().converter(createAccountConverter)
                .parse(createAccount(Klaxon().toJsonString(config), account))

        if (createAccountResult != null) {
            return createAccountResult
        }

        return Err(CreateAccountError.UnexpectedError("createAccountConverter was unable to be called!"))
    }

    fun importAccount(config: Config, account: String): Result<Unit, ImportError> {
        val importResult: Result<Unit, ImportError>? =
            Klaxon().converter(importAccountConverter)
                .parse(importAccount(Klaxon().toJsonString(config), account))

        if (importResult != null) {
            return importResult
        }

        return Err(ImportError.UnexpectedError("importAccountConverter was unable to be called!"))
    }

    fun exportAccount(config: Config): Result<String, AccountExportError> {
        val exportResult: Result<String, AccountExportError>? =
            Klaxon().converter(exportAccountConverter)
                .parse(exportAccount(Klaxon().toJsonString(config)))

        if (exportResult != null) {
            return exportResult
        }

        return Err(AccountExportError.UnexpectedError("exportAccountConverter was unable to be called!"))
    }

    fun syncAllFiles(config: Config): Result<Unit, SyncAllError> {
        val syncResult: Result<Unit, SyncAllError>? =
            Klaxon().converter(syncAllConverter).parse(syncAll(Klaxon().toJsonString(config)))

        if (syncResult != null) {
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

        if (writeResult != null) {
            return writeResult
        }

        return Err(WriteToDocumentError.UnexpectedError("writeDocument was unable to be called!"))
    }

    fun getRoot(config: Config): Result<FileMetadata, GetRootError> {
        val getRootResult: Result<FileMetadata, GetRootError>? =
            Klaxon().converter(getRootConverter).parse(getRoot(Klaxon().toJsonString(config)))

        if (getRootResult != null) {
            return getRootResult
        }

        return Err(GetRootError.UnexpectedError("getRootConverter was unable to be called!"))
    }

    fun getAccount(config: Config): Result<Account, GetAccountError> {
        val getAccountResult: Result<Account, GetAccountError>? =
            Klaxon().converter(getAccountConverter)
                .parse(getAccount(Klaxon().toJsonString(config)))

        if (getAccountResult != null) {
            return getAccountResult
        }

        return Err(GetAccountError.UnexpectedError("getChildrenConverter was unable to be called!"))
    }

    fun setLastSynced(
        config: Config,
        lastSyncedDuration: Long
    ): Result<Unit, SetLastSyncedError> {
        val setLastSyncedResult: Result<Unit, SetLastSyncedError>? =
            Klaxon().converter(setLastSyncedConverter)
                .parse(setLastSynced(Klaxon().toJsonString(config), lastSyncedDuration))

        if (setLastSyncedResult != null) {
            return setLastSyncedResult
        }

        return Err(SetLastSyncedError.UnexpectedError("setLastSyncedConverter was unable to be called!"))
    }

    fun getChildren(
        config: Config,
        parentId: String
    ): Result<List<FileMetadata>, GetChildrenError> {
        val getChildrenResult: Result<List<FileMetadata>, GetChildrenError>? =
            Klaxon().converter(getChildrenConverter)
                .parse(getChildren(Klaxon().toJsonString(config), parentId))

        if (getChildrenResult != null) {
            return getChildrenResult
        }

        return Err(GetChildrenError.UnexpectedError("getChildrenConverter was unable to be called!"))
    }

    fun getFileById(
        config: Config,
        fileId: String
    ): Result<FileMetadata, GetFileByIdError> {
        val getFileByIdResult: Result<FileMetadata, GetFileByIdError>? =
            Klaxon().converter(
                getFileByIdConverter
            ).parse(getFileById(Klaxon().toJsonString(config), fileId))

        if (getFileByIdResult != null) {
            return getFileByIdResult
        }

        return Err(GetFileByIdError.UnexpectedError("getFileByIdConverter was unable to be called!"))
    }

    fun getDocumentContent(
        config: Config,
        fileId: String
    ): Result<DecryptedValue, ReadDocumentError> {
        val getDocumentResult: Result<DecryptedValue, ReadDocumentError>? =
            Klaxon().converter(readDocumentConverter)
                .parse(readDocument(Klaxon().toJsonString(config), fileId))

        if (getDocumentResult != null) {
            return getDocumentResult
        }

        return Err(ReadDocumentError.UnexpectedError("readDocumentConverter was unable to be called!"))
    }

    fun createFile(
        config: Config,
        parentId: String,
        name: String,
        fileType: String
    ): Result<FileMetadata, CreateFileError> {
        val createFileResult: Result<FileMetadata, CreateFileError>? =
            Klaxon().converter(createFileConverter)
                .parse(createFile(Klaxon().toJsonString(config), name, parentId, fileType))

        if (createFileResult != null) {
            return createFileResult
        }

        return Err(CreateFileError.UnexpectedError("createFileConverter was unable to be called!"))
    }

    fun insertFile(
        config: Config,
        fileMetadata: FileMetadata
    ): Result<Unit, InsertFileError> {
        val insertResult: Result<Unit, InsertFileError>? =
            Klaxon().converter(insertFileConverter)
                .parse(
                    insertFile(
                        Klaxon().toJsonString(config),
                        Klaxon().toJsonString(fileMetadata)
                    )
                )

        if (insertResult != null) {
            return insertResult
        }

        return Err(InsertFileError.UnexpectedError("insertFileConverter was unable to be called!"))
    }

    fun deleteFile(
        config: Config,
        id: String
    ): Result<Unit, DeleteFileError> {
        val deleteFile: Result<Unit, DeleteFileError>? =
            Klaxon().converter(deleteFileConverter)
                .parse(deleteFile(Klaxon().toJsonString(config), id))

        if (deleteFile != null) {
            return deleteFile
        }

        return Err(DeleteFileError.UnexpectedError("deleteFileConverter was unable to be called!"))
    }

    fun renameFile(
        config: Config,
        id: String,
        name: String
    ): Result<Unit, RenameFileError> {
        val renameResult: Result<Unit, RenameFileError>? =
            Klaxon().converter(renameFileConverter)
                .parse(renameFile(Klaxon().toJsonString(config), id, name))

        if (renameResult != null) {
            return renameResult
        }

        return Err(RenameFileError.UnexpectedError("renameFileConverter was unable to be called!"))
    }

    fun moveFile(
        config: Config,
        id: String,
        parentId: String
    ): Result<Unit, MoveFileError> {
        val moveResult: Result<Unit, MoveFileError>? =
            Klaxon().converter(moveFileConverter)
                .parse(moveFile(Klaxon().toJsonString(config), id, parentId))

        if (moveResult != null) {
            return moveResult
        }

        return Err(MoveFileError.UnexpectedError("moveFileConverter was unable to be called!"))
    }

    fun calculateFileSyncWork(config: Config): Result<WorkCalculated, CalculateWorkError> {
        val calculateSyncWorkResult: Result<WorkCalculated, CalculateWorkError>? =
            Klaxon().converter(calculateSyncWorkConverter)
                .parse(calculateSyncWork(Klaxon().toJsonString(config)))

        if (calculateSyncWorkResult != null) {
            return calculateSyncWorkResult
        }

        return Err(CalculateWorkError.UnexpectedError("calculateSyncWorkConverter was unable to be called!"))
    }

    fun executeFileSyncWork(
        config: Config,
        account: Account,
        workUnit: WorkUnit
    ): Result<Unit, ExecuteWorkError> {
        val executeSyncWorkResult: Result<Unit, ExecuteWorkError>? =
            Klaxon().converter(executeSyncWorkConverter).parse(
                executeSyncWork(
                    Klaxon().toJsonString(config),
                    Klaxon().toJsonString(account),
                    Klaxon().toJsonString(workUnit)
                )
            )

        if (executeSyncWorkResult != null) {
            return executeSyncWorkResult
        }

        return Err(ExecuteWorkError.UnexpectedError("executeSyncWorkConverter was unable to be called!"))
    }
}
