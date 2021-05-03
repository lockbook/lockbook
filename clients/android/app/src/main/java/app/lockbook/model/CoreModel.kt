package app.lockbook.model

import app.lockbook.core.*
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Result

object CoreModel {

    private const val QA_API_URL = "http://qa.lockbook.app:8000"
    private const val PROD_API_URL = "http://api.lockbook.app:8000"
    fun getAPIURL(): String = System.getenv("API_URL") ?: QA_API_URL

    fun setUpInitLogger(path: String): Result<Unit, InitLoggerError> {
        val initLoggerResult: Result<Unit, InitLoggerError>? =
            Klaxon().converter(initLoggerConverter)
                .parse(initLogger(path))

        if (initLoggerResult != null) {
            return initLoggerResult
        }

        return Err(InitLoggerError.Unexpected("initLoggerConverter was unable to be called!"))
    }

    fun getUsage(config: Config): Result<List<FileUsage>, GetUsageError> {
        val getUsageResult: Result<List<FileUsage>, GetUsageError>? =
            Klaxon().converter(getUsageConverter)
                .parse(getUsage(Klaxon().toJsonString(config)))

        if (getUsageResult != null) {
            return getUsageResult
        }

        return Err(GetUsageError.Unexpected("getUsageConverter was unable to be called!"))
    }

    fun getDBState(config: Config): Result<State, GetStateError> {
        val getStateResult: Result<State, GetStateError>? =
            Klaxon().converter(getStateConverter)
                .parse(getDBState(Klaxon().toJsonString(config)))

        if (getStateResult != null) {
            return getStateResult
        }

        return Err(GetStateError.Unexpected("getStateConverter was unable to be called!"))
    }

    fun migrateDB(config: Config): Result<Unit, MigrationError> {
        val migrateDBResult: Result<Unit, MigrationError>? =
            Klaxon().converter(migrateDBConverter)
                .parse(migrateDB(Klaxon().toJsonString(config)))

        if (migrateDBResult != null) {
            return migrateDBResult
        }

        return Err(MigrationError.Unexpected("migrateDBConverter was unable to be called!"))
    }

    fun generateAccount(config: Config, account: String): Result<Unit, CreateAccountError> {
        val createAccountResult: Result<Unit, CreateAccountError>? =
            Klaxon().converter(createAccountConverter)
                .parse(createAccount(Klaxon().toJsonString(config), account, getAPIURL()))

        if (createAccountResult != null) {
            return createAccountResult
        }

        return Err(CreateAccountError.Unexpected("createAccountConverter was unable to be called!"))
    }

    fun importAccount(config: Config, account: String): Result<Unit, ImportError> {
        val importResult: Result<Unit, ImportError>? =
            Klaxon().converter(importAccountConverter)
                .parse(importAccount(Klaxon().toJsonString(config), account))

        if (importResult != null) {
            return importResult
        }

        return Err(ImportError.Unexpected("importAccountConverter was unable to be called!"))
    }

    fun exportAccount(config: Config): Result<String, AccountExportError> {
        val exportResult: Result<String, AccountExportError>? =
            Klaxon().converter(exportAccountConverter)
                .parse(exportAccount(Klaxon().toJsonString(config)))

        if (exportResult != null) {
            return exportResult
        }

        return Err(AccountExportError.Unexpected("exportAccountConverter was unable to be called!"))
    }

    fun sync(config: Config, syncModel: SyncModel?): Result<Unit, SyncAllError> {
        val syncResult: Result<Unit, SyncAllError>? = if (syncModel != null) {
            Klaxon().converter(syncConverter).parse(syncAll(Klaxon().toJsonString(config), syncModel))
        } else {
            Klaxon().converter(syncConverter).parse(backgroundSync(Klaxon().toJsonString(config)))
        }

        if (syncResult != null) {
            return syncResult
        }

        return Err(SyncAllError.Unexpected("syncConverter was unable to be called!"))
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
                    content
                )
            )

        if (writeResult != null) {
            return writeResult
        }

        return Err(WriteToDocumentError.Unexpected("writeDocument was unable to be called!"))
    }

    fun getRoot(config: Config): Result<FileMetadata, GetRootError> {
        val getRootResult: Result<FileMetadata, GetRootError>? =
            Klaxon().converter(getRootConverter).parse(getRoot(Klaxon().toJsonString(config)))

        if (getRootResult != null) {
            return getRootResult
        }

        return Err(GetRootError.Unexpected("getRootConverter was unable to be called!"))
    }

    fun getAccount(config: Config): Result<Account, GetAccountError> {
        val getAccountResult: Result<Account, GetAccountError>? =
            Klaxon().converter(getAccountConverter)
                .parse(getAccount(Klaxon().toJsonString(config)))

        if (getAccountResult != null) {
            return getAccountResult
        }

        return Err(GetAccountError.Unexpected("getChildrenConverter was unable to be called!"))
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

        return Err(SetLastSyncedError.Unexpected("setLastSyncedConverter was unable to be called!"))
    }

    fun convertToHumanDuration(
        metadataVersion: Long
    ): String = app.lockbook.core.convertToHumanDuration(metadataVersion)

    fun getLastSyncedHumanString(
        config: Config,
    ): Result<String, GetLastSynced> {
        val getLastSyncedHumanString: Result<String, GetLastSynced>? =
            Klaxon().converter(getLastSyncedHumanStringConverter)
                .parse(getLastSyncedHumanString(Klaxon().toJsonString(config)))

        if (getLastSyncedHumanString != null) {
            return getLastSyncedHumanString
        }

        return Err(GetLastSynced.Unexpected("getLastSyncedHumanString was unable to be called!"))
    }

    fun getUsageHumanString(
        config: Config,
        exact: Boolean
    ): Result<String, GetUsageError> {
        val getUsageHumanString: Result<String, GetUsageError>? =
            Klaxon().converter(getUsageHumanStringConverter)
                .parse(getUsageHumanString(Klaxon().toJsonString(config), exact))

        if (getUsageHumanString != null) {
            return getUsageHumanString
        }

        return Err(GetUsageError.Unexpected("getUsageHumanStringConverter was unable to be called!"))
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

        return Err(GetChildrenError.Unexpected("getChildrenConverter was unable to be called!"))
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

        return Err(GetFileByIdError.Unexpected("getFileByIdConverter was unable to be called!"))
    }

    fun getDocumentContent(
        config: Config,
        fileId: String
    ): Result<String, ReadDocumentError> {
        val getDocumentResult: Result<String, ReadDocumentError>? =
            Klaxon().converter(readDocumentConverter)
                .parse(readDocument(Klaxon().toJsonString(config), fileId))

        if (getDocumentResult != null) {
            return getDocumentResult
        }

        return Err(ReadDocumentError.Unexpected("readDocumentConverter was unable to be called!"))
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

        return Err(CreateFileError.Unexpected("createFileConverter was unable to be called!"))
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

        return Err(InsertFileError.Unexpected("insertFileConverter was unable to be called!"))
    }

    fun deleteFile(
        config: Config,
        id: String
    ): Result<Unit, FileDeleteError> {
        val fileDelete: Result<Unit, FileDeleteError>? =
            Klaxon().converter(deleteFileConverter)
                .parse(deleteFile(Klaxon().toJsonString(config), id))

        if (fileDelete != null) {
            return fileDelete
        }

        return Err(FileDeleteError.Unexpected("deleteFileConverter was unable to be called!"))
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

        return Err(RenameFileError.Unexpected("renameFileConverter was unable to be called!"))
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

        return Err(MoveFileError.Unexpected("moveFileConverter was unable to be called!"))
    }

    fun calculateWork(config: Config): Result<WorkCalculated, CalculateWorkError> {
        val calculateWorkResult: Result<WorkCalculated, CalculateWorkError>? =
            Klaxon().converter(calculateWorkConverter)
                .parse(calculateWork(Klaxon().toJsonString(config)))

        if (calculateWorkResult != null) {
            return calculateWorkResult
        }

        return Err(CalculateWorkError.Unexpected("calculateSyncWorkConverter was unable to be called!"))
    }

    fun executeWork(
        config: Config,
        account: Account,
        workUnit: WorkUnit
    ): Result<Unit, ExecuteWorkError> {
        val executeSyncWorkResult: Result<Unit, ExecuteWorkError>? =
            Klaxon().converter(executeWorkConverter).parse(
                executeWork(
                    Klaxon().toJsonString(config),
                    Klaxon().toJsonString(account),
                    Klaxon().toJsonString(workUnit)
                )
            )

        if (executeSyncWorkResult != null) {
            return executeSyncWorkResult
        }

        return Err(ExecuteWorkError.Unexpected("executeSyncWorkConverter was unable to be called!"))
    }
}
