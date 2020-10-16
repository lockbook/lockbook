package app.lockbook.utils

import app.lockbook.core.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Result
import timber.log.Timber

object CoreModel {

    const val API_URL = "http://qa.lockbook.app:8000"

    fun setUpInitLogger(path: String): Result<Unit, CoreError> {
        val initLoggerResult: Result<Unit, CoreError>? =
            Klaxon().converter(initLoggerConverter)
                .parse(initLogger(path))

        if (initLoggerResult != null) {
            return initLoggerResult
        }

        return Err(CoreError.Unexpected("initLoggerConverter was unable to be called!"))
    }

    fun generateAccount(config: Config, account: String): Result<Unit, CoreError> {
        val createAccountResult: Result<Unit, CoreError>? =
            Klaxon().converter(createAccountConverter)
                .parse(createAccount(Klaxon().toJsonString(config), account, API_URL))

        if (createAccountResult != null) {
            return createAccountResult
        }

        return Err(CoreError.Unexpected("createAccountConverter was unable to be called!"))
    }

    fun importAccount(config: Config, account: String): Result<Unit, CoreError> {
        val importResult: Result<Unit, CoreError>? =
            Klaxon().converter(importAccountConverter)
                .parse(importAccount(Klaxon().toJsonString(config), account))

        if (importResult != null) {
            return importResult
        }

        return Err(CoreError.Unexpected("importAccountConverter was unable to be called!"))
    }

    fun exportAccount(config: Config): Result<String, CoreError> {
        val exportResult: Result<String, CoreError>? =
            Klaxon().converter(exportAccountConverter)
                .parse(exportAccount(Klaxon().toJsonString(config)))

        if (exportResult != null) {
            return exportResult
        }

        return Err(CoreError.Unexpected("exportAccountConverter was unable to be called!"))
    }

    fun syncAllFiles(config: Config): Result<Unit, CoreError> {
        val syncResult: Result<Unit, CoreError>? =
            Klaxon().converter(syncAllConverter).parse(syncAll(Klaxon().toJsonString(config)))

        if (syncResult != null) {
            return syncResult
        }

        return Err(CoreError.Unexpected("syncAllConverter was unable to be called!"))
    }

    fun writeContentToDocument(
        config: Config,
        id: String,
        content: String
    ): Result<Unit, CoreError> {
        val writeResult: Result<Unit, CoreError>? =
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

        return Err(CoreError.Unexpected("writeDocument was unable to be called!"))
    }

    fun getRoot(config: Config): Result<FileMetadata, CoreError> {
        val getRootResult: Result<FileMetadata, CoreError>? =
            Klaxon().converter(getRootConverter).parse(getRoot(Klaxon().toJsonString(config)))

        if (getRootResult != null) {
            return getRootResult
        }

        return Err(CoreError.Unexpected("getRootConverter was unable to be called!"))
    }

    fun getAccount(config: Config): Result<Account, CoreError> {
        val getAccountResult: Result<Account, CoreError>? =
            Klaxon().converter(getAccountConverter)
                .parse(getAccount(Klaxon().toJsonString(config)))

        if (getAccountResult != null) {
            return getAccountResult
        }

        return Err(CoreError.Unexpected("getChildrenConverter was unable to be called!"))
    }

    fun setLastSynced(
        config: Config,
        lastSyncedDuration: Long
    ): Result<Unit, CoreError> {
        val setLastSyncedResult: Result<Unit, CoreError>? =
            Klaxon().converter(setLastSyncedConverter)
                .parse(setLastSynced(Klaxon().toJsonString(config), lastSyncedDuration))

        if (setLastSyncedResult != null) {
            return setLastSyncedResult
        }

        return Err(CoreError.Unexpected("setLastSyncedConverter was unable to be called!"))
    }

    fun getChildren(
        config: Config,
        parentId: String
    ): Result<List<FileMetadata>, CoreError> {
        val getChildrenResult: Result<List<FileMetadata>, CoreError>? =
            Klaxon().converter(getChildrenConverter)
                .parse(getChildren(Klaxon().toJsonString(config), parentId))

        if (getChildrenResult != null) {
            return getChildrenResult
        }

        return Err(CoreError.Unexpected("getChildrenConverter was unable to be called!"))
    }

    fun getFileById(
        config: Config,
        fileId: String
    ): Result<FileMetadata, CoreError> {
        val getFileByIdResult: Result<FileMetadata, CoreError>? =
            Klaxon().converter(
                getFileByIdConverter
            ).parse(getFileById(Klaxon().toJsonString(config), fileId))

        if (getFileByIdResult != null) {
            return getFileByIdResult
        }

        return Err(CoreError.Unexpected("getFileByIdConverter was unable to be called!"))
    }

    fun getDocumentContent(
        config: Config,
        fileId: String
    ): Result<DecryptedValue, CoreError> {
        val getDocumentResult: Result<DecryptedValue, CoreError>? =
            Klaxon().converter(readDocumentConverter)
                .parse(readDocument(Klaxon().toJsonString(config), fileId))

        if (getDocumentResult != null) {
            return getDocumentResult
        }

        return Err(CoreError.Unexpected("readDocumentConverter was unable to be called!"))
    }

    fun createFile(
        config: Config,
        parentId: String,
        name: String,
        fileType: String
    ): Result<FileMetadata, CoreError> {
        val createFileResult: Result<FileMetadata, CoreError>? =
            Klaxon().converter(createFileConverter)
                .parse(createFile(Klaxon().toJsonString(config), name, parentId, fileType))

        if (createFileResult != null) {
            return createFileResult
        }

        return Err(CoreError.Unexpected("createFileConverter was unable to be called!"))
    }

    fun insertFile(
        config: Config,
        fileMetadata: FileMetadata
    ): Result<Unit, CoreError> {
        val insertResult: Result<Unit, CoreError>? =
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

        return Err(CoreError.Unexpected("insertFileConverter was unable to be called!"))
    }

    fun deleteFile(
        config: Config,
        id: String
    ): Result<Unit, CoreError> {
        val deleteFile: Result<Unit, CoreError>? =
            Klaxon().converter(deleteFileConverter)
                .parse(deleteFile(Klaxon().toJsonString(config), id))

        if (deleteFile != null) {
            return deleteFile
        }

        return Err(CoreError.Unexpected("deleteFileConverter was unable to be called!"))
    }

    fun renameFile(
        config: Config,
        id: String,
        name: String
    ): Result<Unit, CoreError> {
        val renameResult: Result<Unit, CoreError>? =
            Klaxon().converter(renameFileConverter)
                .parse(renameFile(Klaxon().toJsonString(config), id, name))

        if (renameResult != null) {
            return renameResult
        }

        return Err(CoreError.Unexpected("renameFileConverter was unable to be called!"))
    }

    fun moveFile(
        config: Config,
        id: String,
        parentId: String
    ): Result<Unit, CoreError> {
        val moveResult: Result<Unit, CoreError>? =
            Klaxon().converter(moveFileConverter)
                .parse(moveFile(Klaxon().toJsonString(config), id, parentId))

        if (moveResult != null) {
            return moveResult
        }

        return Err(CoreError.Unexpected("moveFileConverter was unable to be called!"))
    }

    fun calculateFileSyncWork(config: Config): Result<WorkCalculated, CoreError> {
        val calculateSyncWorkResult: Result<WorkCalculated, CoreError>? =
            Klaxon().converter(calculateSyncWorkConverter)
                .parse(calculateSyncWork(Klaxon().toJsonString(config)))

        if (calculateSyncWorkResult != null) {
            return calculateSyncWorkResult
        }

        return Err(CoreError.Unexpected("calculateSyncWorkConverter was unable to be called!"))
    }

    fun executeFileSyncWork(
        config: Config,
        account: Account,
        workUnit: WorkUnit
    ): Result<Unit, CoreError> {
        Timber.e("${Klaxon().toJsonString(workUnit)}, ${config.writeable_path}")
        val executeSyncWorkResult: Result<Unit, CoreError>? =
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

        return Err(CoreError.Unexpected("executeSyncWorkConverter was unable to be called!"))
    }
}
