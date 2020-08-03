package app.lockbook.utils

import com.beust.klaxon.Converter
import com.beust.klaxon.JsonValue
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok

val createAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        val basicError = jv.obj?.get("Err")
        val unexpectedError = jv.obj?.get("UnexpectedError")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (basicError) {
            CreateAccountError.CouldNotReachServer::class.simpleName -> return Err(
                CreateAccountError.CouldNotReachServer
            )
            CreateAccountError.InvalidUsername::class.simpleName -> return Err(
                CreateAccountError.InvalidUsername
            )
            CreateAccountError.UsernameTaken::class.simpleName -> return Err(CreateAccountError.UsernameTaken)
            CreateAccountError.AccountExistsAlready::class.simpleName -> return Err(CreateAccountError.AccountExistsAlready)
        }

        if (unexpectedError is String) {
            return Err(CreateAccountError.UnexpectedError(unexpectedError))
        }

        return Err(CreateAccountError.UnexpectedError("Unable to parse CreateAccountResult!"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val importAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        val basicError = jv.obj?.get("Err")
        val unexpectedError = jv.obj?.get("UnexpectedError")

        if (okResult == true) {
            return Ok(Unit)
        }

        when(basicError) {
            ImportError.AccountStringCorrupted::class.simpleName -> return Err(ImportError.AccountStringCorrupted)
            ImportError.AccountExistsAlready::class.simpleName -> return Err(ImportError.AccountExistsAlready)
            ImportError.AccountDoesNotExist::class.simpleName -> return Err(ImportError.AccountDoesNotExist)
            ImportError.UsernamePKMismatch::class.simpleName -> return Err(ImportError.UsernamePKMismatch)
            ImportError.CouldNotReachServer::class.simpleName -> return Err(ImportError.CouldNotReachServer)
        }

        if (unexpectedError is String) {
            return Err(ImportError.UnexpectedError(unexpectedError))
        }

        return Err(ImportError.UnexpectedError("Unable to parse ImportAccountResult!"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val exportAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.string("Ok")

        val basicError = jv.obj?.get("Err")
        val unexpectedError = jv.obj?.get("UnexpectedError")

        if (okResult is String) {
            return Ok(okResult)
        }

        if (basicError == AccountExportError.NoAccount::class.simpleName) {
            return Err(AccountExportError.NoAccount)
        }

        if (unexpectedError is String) {
            return Err(AccountExportError.UnexpectedError(unexpectedError))
        }

        return Err(AccountExportError.UnexpectedError("Unable to parse AccountExportResult!"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getRootConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.obj("Ok")

        val basicError = jv.obj?.get("Err")
        val unexpectedError = jv.obj?.get("UnexpectedError")

        okResult?.let { jsonObject ->
            return Ok(Klaxon().parseFromJsonObject<FileMetadata>(jsonObject))
        }

        if (basicError == GetRootError.NoRoot::class.simpleName) {
            return Err(GetRootError.NoRoot)
        }

        if (unexpectedError is String) {
            return Err(GetRootError.UnexpectedError(unexpectedError))
        }

        return Err(GetRootError.UnexpectedError("Unable to parse GetRootResult!"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getChildrenConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.array<FileMetadata>("Ok")

        val unexpectedError = jv.obj?.get("UnexpectedError")

        okResult?.let { jsonArray ->
            return Ok(Klaxon().parseFromJsonArray<FileMetadata>(jsonArray))
        }

        if (unexpectedError is String) {
            return Err(GetChildrenError.UnexpectedError(unexpectedError))
        }

        return Err(GetChildrenError.UnexpectedError("Unable to parse GetChildrenResult!"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getFileByIdConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.obj("Ok")

        val basicError = jv.obj?.get("Err")
        val unexpectedError = jv.obj?.get("UnexpectedError")

        okResult?.let { jsonObject ->
            return Ok(Klaxon().parseFromJsonObject<FileMetadata>(jsonObject))
        }

        if (basicError == GetFileByIdError.NoFileWithThatId::class.simpleName) {
            return Err(GetFileByIdError.NoFileWithThatId)
        }

        if (unexpectedError is String) {
            return Err(GetFileByIdError.UnexpectedError(unexpectedError))
        }

        return Err(GetFileByIdError.UnexpectedError("Unable to parse GetFileByIdResult!"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val insertFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        val unexpectedError = jv.obj?.get("UnexpectedError")

        if (okResult == true) {
            return Ok(Unit)
        }

        if (unexpectedError is String) {
            return Err(InsertFileError.UnexpectedError(unexpectedError))
        }

        return Err(InsertFileError.UnexpectedError("Unable to parse InsertFileResult!"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val renameFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        val basicError = jv.obj?.get("Err")
        val unexpectedError = jv.obj?.get("UnexpectedError")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (basicError) {
            RenameFileError.FileDoesNotExist::class.simpleName -> return Err(
                RenameFileError.FileDoesNotExist
            )
            RenameFileError.FileNameNotAvailable::class.simpleName -> return Err(
                RenameFileError.FileNameNotAvailable
            )
            RenameFileError.NewNameContainsSlash::class.simpleName -> return Err(RenameFileError.NewNameContainsSlash)
        }

        if (unexpectedError is String) {
            return Err(RenameFileError.UnexpectedError(unexpectedError))
        }

        return Err(RenameFileError.UnexpectedError("Unable to parse RenameFileResult!"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val createFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.obj("Ok")

        val basicError = jv.obj?.get("Err")
        val unexpectedError = jv.obj?.get("UnexpectedError")

        okResult?.let { jsonObject ->
            return Ok(Klaxon().parseFromJsonObject<FileMetadata>(jsonObject))
        }

        when (basicError) {
            CreateFileError.NoAccount::class.simpleName -> return Err(CreateFileError.NoAccount)
            CreateFileError.DocumentTreatedAsFolder::class.simpleName -> return Err(CreateFileError.DocumentTreatedAsFolder)
            CreateFileError.CouldNotFindAParent::class.simpleName -> return Err(CreateFileError.CouldNotFindAParent)
            CreateFileError.FileNameNotAvailable::class.simpleName -> return Err(CreateFileError.FileNameNotAvailable)
            CreateFileError.FileNameContainsSlash::class.simpleName -> return Err(CreateFileError.FileNameContainsSlash)
        }

        if (unexpectedError is String) {
            return Err(CreateFileError.UnexpectedError(unexpectedError))
        }

        return Err(CreateFileError.UnexpectedError("Unable to parse CreateFileResult!"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val deleteFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        val basicError = jv.obj?.get("Err")
        val unexpectedError = jv.obj?.get("UnexpectedError")

        if (okResult == true) {
            return Ok(Unit)
        }

        if (basicError == DeleteFileError.NoFileWithThatId::class.simpleName) {
            return Err(DeleteFileError.NoFileWithThatId)
        }

        if (unexpectedError is String) {
            return Err(DeleteFileError.UnexpectedError(unexpectedError))
        }

        return Err(DeleteFileError.UnexpectedError("Unable to parse DeleteFileResult!"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val readDocumentConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.obj("Ok")

        val basicError = jv.obj?.get("Err")
        val unexpectedError = jv.obj?.get("UnexpectedError")

        okResult?.let { jsonObject ->
            return Ok(Klaxon().parseFromJsonObject<DecryptedValue>(jsonObject))
        }

        when (basicError) {
            ReadDocumentError.TreatedFolderAsDocument::class.simpleName -> return Err(
                ReadDocumentError.TreatedFolderAsDocument
            )
            ReadDocumentError.NoAccount::class.simpleName -> return Err(ReadDocumentError.NoAccount)
            ReadDocumentError.FileDoesNotExist::class.simpleName -> return Err(ReadDocumentError.FileDoesNotExist)
        }

        if (unexpectedError is String) {
            return Err(ReadDocumentError.UnexpectedError(unexpectedError))
        }

        return Err(ReadDocumentError.UnexpectedError("Unable to parse ReadDocumentResult!"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val writeDocumentConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        val basicError = jv.obj?.get("Err")
        val unexpectedError = jv.obj?.get("UnexpectedError")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (basicError) {
            WriteToDocumentError.NoAccount::class.simpleName -> return Err(WriteToDocumentError.NoAccount)
            WriteToDocumentError.FileDoesNotExist::class.simpleName -> return Err(
                WriteToDocumentError.FileDoesNotExist
            )
            WriteToDocumentError.FolderTreatedAsDocument::class.simpleName -> return Err(
                WriteToDocumentError.FolderTreatedAsDocument
            )
        }

        if (unexpectedError is String) {
            return Err(WriteToDocumentError.UnexpectedError(unexpectedError))
        }

        return Err(WriteToDocumentError.UnexpectedError("Unable to parse WriteToDocumentResult!"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val moveFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        val basicError = jv.obj?.get("Err")
        val unexpectedError = jv.obj?.get("UnexpectedError")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (basicError) {
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
        }

        if (unexpectedError is String) {
            return Err(MoveFileError.UnexpectedError(unexpectedError))
        }

        return MoveFileError.UnexpectedError("Unable to parse MoveFileResult!")
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val syncAllConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        val basicError = jv.obj?.get("Err")
        val unexpectedError = jv.obj?.get("UnexpectedError")

        if (okResult == true) {
            return Ok(Unit)
        }

        when (basicError) {
            SyncAllError.NoAccount::class.simpleName -> return Err(
                SyncAllError.NoAccount
            )
            SyncAllError.CouldNotReachServer::class.simpleName -> return Err(
                SyncAllError.CouldNotReachServer
            )
        }

        if (unexpectedError is String) {
            return Err(SyncAllError.UnexpectedError(unexpectedError))
        }

        return Err(SyncAllError.UnexpectedError("Unable to parse SyncAllResult!"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val calculateSyncWorkConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.obj("Ok")

        val basicError = jv.obj?.get("Err")
        val unexpectedError = jv.obj?.get("UnexpectedError")

        okResult?.let { jsonObject ->
            return Ok(Klaxon().parseFromJsonObject<WorkCalculated>(jsonObject))
        }

        when (basicError) {
            CalculateWorkError.NoAccount::class.simpleName -> return Err(
                CalculateWorkError.NoAccount
            )
            CalculateWorkError.CouldNotReachServer::class.simpleName -> return Err(
                CalculateWorkError.CouldNotReachServer
            )
        }

        if (unexpectedError is String) {
            return Err(CalculateWorkError.UnexpectedError(unexpectedError))
        }

        return Err(CalculateWorkError.UnexpectedError("Unable to parse CalculateSyncWorkResult!"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val executeSyncWorkConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val okResult = jv.obj?.containsKey("Ok")

        val basicError = jv.obj?.get("Err")
        val unexpectedError = jv.obj?.get("UnexpectedError")

        if (okResult == true) {
            return Ok(Unit)
        }

        if (basicError == ExecuteWorkError.CouldNotReachServer::class.simpleName) {
            return Err(ExecuteWorkError.CouldNotReachServer)
        }

        if (unexpectedError is String) {
            return Err(ExecuteWorkError.UnexpectedError(unexpectedError))
        }

        return ExecuteWorkError.UnexpectedError("Unable to parse ExecuteSyncWorkResult!")
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}
