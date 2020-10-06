package app.lockbook.utils

import com.beust.klaxon.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import timber.log.Timber

val initLoggerConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        if (okResult == true) {
            return Ok(Unit)
        }

        val unexpectedResult = jv.obj?.obj("Err")?.get("Unexpected")

        if (unexpectedResult is String) {
            return Err(InitLoggerError.Unexpected(unexpectedResult))
        }

        return Err(InitLoggerError.Unexpected("Unable to parse InitLoggerError: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getStateConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {

        Timber.e("${jv.obj?.string("Ok")}, ${State.Empty::class.simpleName}, ${State.Empty.name}")
        when(jv.obj?.string("Ok")) {
            State.MigrationRequired.name -> return Ok(State.MigrationRequired)
            State.StateRequiresClearing.name -> return Ok(State.StateRequiresClearing)
            State.ReadyToUse.name -> return Ok(State.ReadyToUse)
            State.Empty.name -> return Ok(State.Empty)
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(GetStateError.UnexpectedError(unexpectedResult))
        }

        return Err(GetStateError.UnexpectedError("Unable to parse GetStateResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val migrateDBConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        if (okResult == true) {
            return Ok(Unit)
        }

        if(jv.obj?.get("Err") == MigrationError.StateRequiresCleaning::class.simpleName) {
            return Err(MigrationError.StateRequiresCleaning)
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(MigrationError.UnexpectedError(unexpectedResult))
        }

        return Err(MigrationError.UnexpectedError("Unable to parse MigrateDBResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val createAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (jv.obj?.get("Err")) {
            CreateAccountError.CouldNotReachServer::class.simpleName -> return Err(
                CreateAccountError.CouldNotReachServer
            )
            CreateAccountError.InvalidUsername::class.simpleName -> return Err(
                CreateAccountError.InvalidUsername
            )
            CreateAccountError.UsernameTaken::class.simpleName -> return Err(CreateAccountError.UsernameTaken)
            CreateAccountError.AccountExistsAlready::class.simpleName -> return Err(
                CreateAccountError.AccountExistsAlready
            )
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(CreateAccountError.UnexpectedError(unexpectedResult))
        }

        return Err(CreateAccountError.UnexpectedError("Unable to parse CreateAccountResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val importAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (jv.obj?.get("Err")) {
            ImportError.AccountStringCorrupted::class.simpleName -> return Err(ImportError.AccountStringCorrupted)
            ImportError.AccountExistsAlready::class.simpleName -> return Err(ImportError.AccountExistsAlready)
            ImportError.AccountDoesNotExist::class.simpleName -> return Err(ImportError.AccountDoesNotExist)
            ImportError.UsernamePKMismatch::class.simpleName -> return Err(ImportError.UsernamePKMismatch)
            ImportError.CouldNotReachServer::class.simpleName -> return Err(ImportError.CouldNotReachServer)
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(ImportError.UnexpectedError(unexpectedResult))
        }

        return Err(ImportError.UnexpectedError("Unable to parse ImportAccountResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val exportAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.string("Ok")

        if (okResult is String) {
            return Ok(okResult)
        }

        val errorResult = jv.obj?.get("Err")

        if (errorResult == AccountExportError.NoAccount::class.simpleName) {
            return Err(AccountExportError.NoAccount)
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(AccountExportError.UnexpectedError(unexpectedResult))
        }

        return Err(AccountExportError.UnexpectedError("Unable to parse AccountExportResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.obj("Ok")

        if (okResult is JsonObject) {
            return Ok(Klaxon().parseFromJsonObject<Account>(okResult))
        }

        val errorResult = jv.obj?.get("Err")

        if (errorResult == GetAccountError.NoAccount::class.simpleName) {
            return Err(GetAccountError.NoAccount)
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(GetAccountError.UnexpectedError(unexpectedResult))
        }

        return Err(GetAccountError.UnexpectedError("Unable to parse GetAccountResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val setLastSyncedConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        if (okResult == true) {
            return Ok(Unit)
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(SetLastSyncedError.UnexpectedError(unexpectedResult))
        }

        return Err(SetLastSyncedError.UnexpectedError("Unable to parse SetLastSyncedResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getRootConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.obj("Ok")

        if (okResult is JsonObject) {
            return Ok(Klaxon().parseFromJsonObject<FileMetadata>(okResult))
        }

        val errorResult = jv.obj?.get("Err")

        if (errorResult == GetRootError.NoRoot::class.simpleName) {
            return Err(GetRootError.NoRoot)
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(GetRootError.UnexpectedError(unexpectedResult))
        }

        return Err(GetRootError.UnexpectedError("Unable to parse GetRootResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getChildrenConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.array<FileMetadata>("Ok")

        if (okResult != null) {
            return Ok(Klaxon().parseFromJsonArray<FileMetadata>(okResult))
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(GetChildrenError.UnexpectedError(unexpectedResult))
        }

        return Err(GetChildrenError.UnexpectedError("Unable to parse GetChildrenResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getFileByIdConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.obj("Ok")

        if (okResult is JsonObject) {
            return Ok(Klaxon().parseFromJsonObject<FileMetadata>(okResult))
        }

        if (jv.obj?.get("Err") == GetFileByIdError.NoFileWithThatId::class.simpleName) {
            return Err(GetFileByIdError.NoFileWithThatId)
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(GetFileByIdError.UnexpectedError(unexpectedResult))
        }

        return Err(GetFileByIdError.UnexpectedError("Unable to parse GetFileByIdResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val insertFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        if (okResult == true) {
            return Ok(Unit)
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(InsertFileError.UnexpectedError(unexpectedResult))
        }

        return Err(InsertFileError.UnexpectedError("Unable to parse InsertFileResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val renameFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (jv.obj?.get("Err")) {
            RenameFileError.FileDoesNotExist::class.simpleName -> return Err(
                RenameFileError.FileDoesNotExist
            )
            RenameFileError.FileNameNotAvailable::class.simpleName -> return Err(
                RenameFileError.FileNameNotAvailable
            )
            RenameFileError.NewNameEmpty::class.simpleName -> return Err(
                RenameFileError.NewNameEmpty
            )
            RenameFileError.CannotRenameRoot::class.simpleName -> return Err(
                RenameFileError.CannotRenameRoot
            )
            RenameFileError.NewNameContainsSlash::class.simpleName -> return Err(RenameFileError.NewNameContainsSlash)
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(RenameFileError.UnexpectedError(unexpectedResult))
        }

        return Err(RenameFileError.UnexpectedError("Unable to parse RenameFileResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val createFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.obj("Ok")

        if (okResult is JsonObject) {
            return Ok(Klaxon().parseFromJsonObject<FileMetadata>(okResult))
        }

        when (jv.obj?.get("Err")) {
            CreateFileError.NoAccount::class.simpleName -> return Err(CreateFileError.NoAccount)
            CreateFileError.DocumentTreatedAsFolder::class.simpleName -> return Err(CreateFileError.DocumentTreatedAsFolder)
            CreateFileError.CouldNotFindAParent::class.simpleName -> return Err(CreateFileError.CouldNotFindAParent)
            CreateFileError.FileNameNotAvailable::class.simpleName -> return Err(CreateFileError.FileNameNotAvailable)
            CreateFileError.FileNameContainsSlash::class.simpleName -> return Err(CreateFileError.FileNameContainsSlash)
            CreateFileError.FileNameEmpty::class.simpleName -> return Err(CreateFileError.FileNameEmpty)
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(CreateFileError.UnexpectedError(unexpectedResult))
        }

        return Err(CreateFileError.UnexpectedError("Unable to parse CreateFileResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val deleteFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        if (okResult == true) {
            return Ok(Unit)
        }

        if (jv.obj?.get("Err") == DeleteFileError.NoFileWithThatId::class.simpleName) {
            return Err(DeleteFileError.NoFileWithThatId)
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(SetLastSyncedError.UnexpectedError(unexpectedResult))
        }

        return Err(SetLastSyncedError.UnexpectedError("Unable to parse DeleteFileResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val readDocumentConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.obj("Ok")

        if (okResult is JsonObject) {
            return Ok(Klaxon().parseFromJsonObject<DecryptedValue>(okResult))
        }

        when (jv.obj?.get("Err")) {
            ReadDocumentError.TreatedFolderAsDocument::class.simpleName -> return Err(
                ReadDocumentError.TreatedFolderAsDocument
            )
            ReadDocumentError.NoAccount::class.simpleName -> return Err(ReadDocumentError.NoAccount)
            ReadDocumentError.FileDoesNotExist::class.simpleName -> return Err(ReadDocumentError.FileDoesNotExist)
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(ReadDocumentError.UnexpectedError(unexpectedResult))
        }

        return Err(ReadDocumentError.UnexpectedError("Unable to parse ReadDocumentResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val writeDocumentConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (jv.obj?.get("Err")) {
            WriteToDocumentError.NoAccount::class.simpleName -> return Err(WriteToDocumentError.NoAccount)
            WriteToDocumentError.FileDoesNotExist::class.simpleName -> return Err(
                WriteToDocumentError.FileDoesNotExist
            )
            WriteToDocumentError.FolderTreatedAsDocument::class.simpleName -> return Err(
                WriteToDocumentError.FolderTreatedAsDocument
            )
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(WriteToDocumentError.UnexpectedError(unexpectedResult))
        }

        return Err(WriteToDocumentError.UnexpectedError("Unable to parse WriteToDocumentResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val moveFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (jv.obj?.get("Err")) {
            MoveFileError.NoAccount::class.simpleName -> return Err(
                MoveFileError.NoAccount
            )
            MoveFileError.FileDoesNotExist::class.simpleName -> return Err(
                MoveFileError.FileDoesNotExist
            )
            MoveFileError.DocumentTreatedAsFolder::class.simpleName -> return Err(
                MoveFileError.DocumentTreatedAsFolder
            )
            MoveFileError.TargetParentDoesNotExist::class.simpleName -> return Err(
                MoveFileError.TargetParentDoesNotExist
            )
            MoveFileError.TargetParentHasChildNamedThat::class.simpleName -> return Err(
                MoveFileError.TargetParentHasChildNamedThat
            )
            MoveFileError.CannotMoveRoot::class.simpleName -> return Err(
                MoveFileError.CannotMoveRoot
            )
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(MoveFileError.UnexpectedError(unexpectedResult))
        }

        return Err(MoveFileError.UnexpectedError("Unable to parse MoveFileResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val syncAllConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (val errorResult = jv.obj?.get("Err")) {
            SyncAllError.NoAccount::class.simpleName -> return Err(
                SyncAllError.NoAccount
            )
            SyncAllError.CouldNotReachServer::class.simpleName -> return Err(
                SyncAllError.CouldNotReachServer
            )
            is JsonObject -> {
                val innerJsonArray = errorResult.array<ExecuteWorkError>("ExecuteWorkError")
                if (innerJsonArray is JsonArray<ExecuteWorkError>) {
                    val executeWorkError = Klaxon().converter(executeSyncWorkErrorsConverter)
                        .parseFromJsonArray<ExecuteWorkError>(innerJsonArray)
                    if (executeWorkError != null) {
                        return Err(SyncAllError.ExecuteWorkError(executeWorkError))
                    }
                }
            }
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(SyncAllError.UnexpectedError(unexpectedResult))
        }

        return Err(SyncAllError.UnexpectedError("Unable to parse SyncAllResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val calculateSyncWorkConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okWorkUnits = jv.obj?.obj("Ok")?.array<WorkUnit>("work_units")
        val okMostRecentUpdate = jv.obj?.obj("Ok")?.long("most_recent_update_from_server")

        if (okWorkUnits is JsonArray<WorkUnit> && okMostRecentUpdate is Long) {
            val workUnitResult = Klaxon().parseFromJsonArray<WorkUnit>(okWorkUnits)
            if (workUnitResult is List<WorkUnit>) {
                return Ok(WorkCalculated(workUnitResult, okMostRecentUpdate))
            }
        }

        when (jv.obj?.get("Err")) {
            CalculateWorkError.NoAccount::class.simpleName -> return Err(
                CalculateWorkError.NoAccount
            )
            CalculateWorkError.CouldNotReachServer::class.simpleName -> return Err(
                CalculateWorkError.CouldNotReachServer
            )
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(CalculateWorkError.UnexpectedError(unexpectedResult))
        }

        return Err(CalculateWorkError.UnexpectedError("Unable to parse CalculateSyncWorkResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val executeSyncWorkConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        if (okResult == true) {
            return Ok(Unit)
        }

        if (jv.obj?.get("Err") == ExecuteWorkError.CouldNotReachServer::class.simpleName) {
            return Err(ExecuteWorkError.CouldNotReachServer)
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return Err(ExecuteWorkError.UnexpectedError(unexpectedResult))
        }

        return Err(ExecuteWorkError.UnexpectedError("Unable to parse ExecuteSyncWorkResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val executeSyncWorkErrorsConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {

        if (jv.string == ExecuteWorkError.CouldNotReachServer::class.simpleName) {
            return ExecuteWorkError.CouldNotReachServer
        }

        val unexpectedResult = jv.obj?.get("UnexpectedError")

        if (unexpectedResult is String) {
            return ExecuteWorkError.UnexpectedError(unexpectedResult)
        }

        return ExecuteWorkError.UnexpectedError("Unable to parse SyncAll.ExecuteSyncWorkErrors: ${jv.obj?.toJsonString()}")
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}
