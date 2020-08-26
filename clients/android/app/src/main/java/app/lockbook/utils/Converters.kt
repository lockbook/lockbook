package app.lockbook.utils

import com.beust.klaxon.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import timber.log.Timber

val createAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        val errorResult = jv.obj?.get("Err")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (errorResult) {
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
            is JsonObject -> {
                val unexpectedError = errorResult.string("UnexpectedError")
                if (unexpectedError is String) {
                    return Err(CreateAccountError.UnexpectedError(unexpectedError))
                }
            }
        }

        return Err(CreateAccountError.UnexpectedError("Unable to parse CreateAccountResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val importAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        val errorResult = jv.obj?.get("Err")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (errorResult) {
            ImportError.AccountStringCorrupted::class.simpleName -> return Err(ImportError.AccountStringCorrupted)
            ImportError.AccountExistsAlready::class.simpleName -> return Err(ImportError.AccountExistsAlready)
            ImportError.AccountDoesNotExist::class.simpleName -> return Err(ImportError.AccountDoesNotExist)
            ImportError.UsernamePKMismatch::class.simpleName -> return Err(ImportError.UsernamePKMismatch)
            ImportError.CouldNotReachServer::class.simpleName -> return Err(ImportError.CouldNotReachServer)
            is JsonObject -> {
                val unexpectedError = errorResult.string("UnexpectedError")
                if (unexpectedError is String) {
                    return Err(ImportError.UnexpectedError(unexpectedError))
                }
            }
        }

        return Err(ImportError.UnexpectedError("Unable to parse ImportAccountResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val exportAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.string("Ok")

        val errorResult = jv.obj?.get("Err")

        if (okResult is String) {
            return Ok(okResult)
        }

        when (errorResult) {
            AccountExportError.NoAccount::class.simpleName -> return Err(AccountExportError.NoAccount)
            is JsonObject -> {
                val unexpectedError = errorResult.string("UnexpectedError")
                if (unexpectedError is String) {
                    return Err(AccountExportError.UnexpectedError(unexpectedError))
                }
            }
        }

        return Err(AccountExportError.UnexpectedError("Unable to parse AccountExportResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.obj("Ok")
        val errorResult = jv.obj?.get("Err")

        okResult?.let { jsonObject ->
            return Ok(Klaxon().parseFromJsonObject<Account>(jsonObject))
        }

        when (errorResult) {
            GetAccountError.NoAccount::class.simpleName -> return Err(GetAccountError.NoAccount)
            is JsonObject -> {
                val unexpectedError = errorResult.string("UnexpectedError")
                if (unexpectedError is String) {
                    return Err(GetAccountError.UnexpectedError(unexpectedError))
                }
            }
        }

        return Err(GetAccountError.UnexpectedError("Unable to parse GetAccountResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val setLastSyncedConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")
        val errorResult = jv.obj?.get("Err")

        if (okResult == true) {
            return Ok(Unit)
        }

        if (errorResult is JsonObject) {
            val unexpectedError = errorResult.string("UnexpectedError")
            if (unexpectedError is String) {
                return Err(SetLastSyncedError.UnexpectedError(unexpectedError))
            }
        }

        return Err(SetLastSyncedError.UnexpectedError("Unable to parse SetLastSyncedResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getRootConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.obj("Ok")
        val errorResult = jv.obj?.get("Err")

        okResult?.let { jsonObject ->
            return Ok(Klaxon().parseFromJsonObject<FileMetadata>(jsonObject))
        }

        when (errorResult) {
            GetRootError.NoRoot::class.simpleName -> return Err(GetRootError.NoRoot)
            is JsonObject -> {
                val unexpectedError = errorResult.string("UnexpectedError")
                if (unexpectedError is String) {
                    return Err(GetRootError.UnexpectedError(unexpectedError))
                }
            }
        }

        return Err(GetRootError.UnexpectedError("Unable to parse GetRootResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getChildrenConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.array<FileMetadata>("Ok")
        val errorResult = jv.obj?.get("Err")

        okResult?.let { jsonArray ->
            return Ok(Klaxon().parseFromJsonArray<FileMetadata>(jsonArray))
        }

        if (errorResult is JsonObject) {
            val unexpectedError = errorResult.string("UnexpectedError")
            if (unexpectedError is String) {
                return Err(GetChildrenError.UnexpectedError(unexpectedError))
            }
        }

        return Err(GetChildrenError.UnexpectedError("Unable to parse GetChildrenResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getFileByIdConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.obj("Ok")
        val errorResult = jv.obj?.get("Err")

        okResult?.let { jsonObject ->
            return Ok(Klaxon().parseFromJsonObject<FileMetadata>(jsonObject))
        }

        when (errorResult) {
            GetFileByIdError.NoFileWithThatId::class.simpleName -> Err(GetFileByIdError.NoFileWithThatId)
            is JsonObject -> {
                val unexpectedError = errorResult.string("UnexpectedError")
                if (unexpectedError is String) {
                    return Err(GetFileByIdError.UnexpectedError(unexpectedError))
                }
            }
        }

        return Err(GetFileByIdError.UnexpectedError("Unable to parse GetFileByIdResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val insertFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")
        val errorResult = jv.obj?.get("Err")

        if (okResult == true) {
            return Ok(Unit)
        }

        if (errorResult is JsonObject) {
            val unexpectedError = errorResult.string("UnexpectedError")
            if (unexpectedError is String) {
                return Err(InsertFileError.UnexpectedError(unexpectedError))
            }
        }

        return Err(InsertFileError.UnexpectedError("Unable to parse InsertFileResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val renameFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")
        val errorResult = jv.obj?.get("Err")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (errorResult) {
            RenameFileError.FileDoesNotExist::class.simpleName -> return Err(
                RenameFileError.FileDoesNotExist
            )
            RenameFileError.FileNameNotAvailable::class.simpleName -> return Err(
                RenameFileError.FileNameNotAvailable
            )
            RenameFileError.NewNameContainsSlash::class.simpleName -> return Err(RenameFileError.NewNameContainsSlash)
            is JsonObject -> {
                val unexpectedError = errorResult.string("UnexpectedError")
                if (unexpectedError is String) {
                    return Err(RenameFileError.UnexpectedError(unexpectedError))
                }
            }
        }

        return Err(RenameFileError.UnexpectedError("Unable to parse RenameFileResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val createFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.obj("Ok")
        val errorResult = jv.obj?.get("Err")

        okResult?.let { jsonObject ->
            return Ok(Klaxon().parseFromJsonObject<FileMetadata>(jsonObject))
        }

        when (errorResult) {
            CreateFileError.NoAccount::class.simpleName -> return Err(CreateFileError.NoAccount)
            CreateFileError.DocumentTreatedAsFolder::class.simpleName -> return Err(CreateFileError.DocumentTreatedAsFolder)
            CreateFileError.CouldNotFindAParent::class.simpleName -> return Err(CreateFileError.CouldNotFindAParent)
            CreateFileError.FileNameNotAvailable::class.simpleName -> return Err(CreateFileError.FileNameNotAvailable)
            CreateFileError.FileNameContainsSlash::class.simpleName -> return Err(CreateFileError.FileNameContainsSlash)
            is JsonObject -> {
                val unexpectedError = errorResult.string("UnexpectedError")
                if (unexpectedError is String) {
                    return Err(CreateFileError.UnexpectedError(unexpectedError))
                }
            }
        }

        return Err(CreateFileError.UnexpectedError("Unable to parse CreateFileResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val deleteFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        val errorResult = jv.obj?.get("Err")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (errorResult) {
            DeleteFileError.NoFileWithThatId::class.simpleName -> return Err(DeleteFileError.NoFileWithThatId)
            is JsonObject -> {
                val unexpectedError = errorResult.string("UnexpectedError")
                if (unexpectedError is String) {
                    return Err(DeleteFileError.UnexpectedError(unexpectedError))
                }
            }
        }

        return Err(DeleteFileError.UnexpectedError("Unable to parse DeleteFileResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val readDocumentConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.obj("Ok")
        val errorResult = jv.obj?.get("Err")

        okResult?.let { jsonObject ->
            return Ok(Klaxon().parseFromJsonObject<DecryptedValue>(jsonObject))
        }

        when (errorResult) {
            ReadDocumentError.TreatedFolderAsDocument::class.simpleName -> return Err(
                ReadDocumentError.TreatedFolderAsDocument
            )
            ReadDocumentError.NoAccount::class.simpleName -> return Err(ReadDocumentError.NoAccount)
            ReadDocumentError.FileDoesNotExist::class.simpleName -> return Err(ReadDocumentError.FileDoesNotExist)
            is JsonObject -> {
                val unexpectedError = errorResult.string("UnexpectedError")
                if (unexpectedError is String) {
                    return Err(ReadDocumentError.UnexpectedError(unexpectedError))
                }
            }
        }

        return Err(ReadDocumentError.UnexpectedError("Unable to parse ReadDocumentResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val writeDocumentConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")
        val errorResult = jv.obj?.get("Err")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (errorResult) {
            WriteToDocumentError.NoAccount::class.simpleName -> return Err(WriteToDocumentError.NoAccount)
            WriteToDocumentError.FileDoesNotExist::class.simpleName -> return Err(
                WriteToDocumentError.FileDoesNotExist
            )
            WriteToDocumentError.FolderTreatedAsDocument::class.simpleName -> return Err(
                WriteToDocumentError.FolderTreatedAsDocument
            )
            is JsonObject -> {
                val unexpectedError = errorResult.string("UnexpectedError")
                if (unexpectedError is String) {
                    return Err(WriteToDocumentError.UnexpectedError(unexpectedError))
                }
            }
        }

        return Err(WriteToDocumentError.UnexpectedError("Unable to parse WriteToDocumentResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val moveFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")
        val errorResult = jv.obj?.get("Err")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (errorResult) {
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
            is JsonObject -> {
                val unexpectedError = errorResult.string("UnexpectedError")
                if (unexpectedError is String) {
                    return Err(MoveFileError.UnexpectedError(unexpectedError))
                }
            }
        }

        return MoveFileError.UnexpectedError("Unable to parse MoveFileResult: ${jv.obj?.toJsonString()}")
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val syncAllConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        val errorResult = jv.obj?.get("Err")
        val executeWorkError = jv.obj?.array<FileMetadata>("ExecuteWorkError")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (errorResult) {
            SyncAllError.NoAccount::class.simpleName -> return Err(
                SyncAllError.NoAccount
            )
            SyncAllError.CouldNotReachServer::class.simpleName -> return Err(
                SyncAllError.CouldNotReachServer
            )
            is JsonObject -> {
                val unexpectedError = errorResult.string("UnexpectedError")
                if (unexpectedError is String) {
                    return Err(SyncAllError.UnexpectedError(unexpectedError))
                }
            }
        }

        executeWorkError?.let { jsonArray ->
            return Err(Klaxon().parseFromJsonArray<ExecuteWorkError>(jsonArray))
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

        val errorResult = jv.obj?.get("Err")

        if (okWorkUnits is JsonArray<WorkUnit> && okMostRecentUpdate is Long) {
            val workUnitResult = Klaxon().parseFromJsonArray<WorkUnit>(okWorkUnits)
            if (workUnitResult is List<WorkUnit>) {
                return Ok(WorkCalculated(workUnitResult, okMostRecentUpdate))
            }
        }

        when (errorResult) {
            CalculateWorkError.NoAccount::class.simpleName -> return Err(
                CalculateWorkError.NoAccount
            )
            CalculateWorkError.CouldNotReachServer::class.simpleName -> return Err(
                CalculateWorkError.CouldNotReachServer
            )
            is JsonObject -> {
                val unexpectedError = errorResult.string("UnexpectedError")
                if (unexpectedError is String) {
                    return Err(CalculateWorkError.UnexpectedError(unexpectedError))
                }
            }
        }

        return Err(CalculateWorkError.UnexpectedError("Unable to parse CalculateSyncWorkResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val executeSyncWorkConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        val errorResult = jv.obj?.get("Err")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (errorResult) {
            ExecuteWorkError.CouldNotReachServer::class.simpleName -> return Err(ExecuteWorkError.CouldNotReachServer)
            is JsonObject -> {
                val unexpectedError = errorResult.string("UnexpectedError")
                if (unexpectedError is String) {
                    return Err(ExecuteWorkError.UnexpectedError(unexpectedError))
                }
            }
        }

        return Err(ExecuteWorkError.UnexpectedError("Unable to parse ExecuteSyncWorkResult: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}
