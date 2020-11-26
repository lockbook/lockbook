package app.lockbook.util

import com.beust.klaxon.Converter
import com.beust.klaxon.JsonValue
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok

const val okTag = "Ok"
const val errTag = "Err"
const val unexpectedTag = "Unexpected"
const val uiErrorTag = "UiError"
const val unmatchedTag = "couldn't match outermost tag to anything. Json: "
const val unmatchedErrorTag = "couldn't match error tag to anything: "
const val unmatchedUiError = "couldn't match a type of UiError: "
const val unmatchedOkEnum = "couldn't match a type of Enum: "
const val unableToGetUiError = "couldn't get UiError type from content. Json: "
const val unableToGetUnexpectedError = "couldn't get UnexpectedError message from content. Json: "
const val unableToGetOk = "couldn't get Ok tag content. Json: "

val initLoggerConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(InitLoggerError.Unexpected(error))
                } else {
                    Err(InitLoggerError.Unexpected("initLoggerConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(InitLoggerError.Unexpected("initLoggerConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(InitLoggerError.Unexpected("initLoggerConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getUsageConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> {
            val ok = jv.obj?.array<FileUsage>("content")
            if (ok != null) {
                Ok(Klaxon().parseFromJsonArray<FileUsage>(ok))
            } else {
                Err(GetUsageError.Unexpected("getUsageConverter $unableToGetOk ${jv.obj?.toJsonString()}"))
            }
        }
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            GetUsageError.ClientUpdateRequired::class.simpleName -> GetUsageError.ClientUpdateRequired
                            GetUsageError.CouldNotReachServer::class.simpleName -> GetUsageError.CouldNotReachServer
                            GetUsageError.NoAccount::class.simpleName -> GetUsageError.NoAccount
                            else -> GetUsageError.Unexpected("getUsageConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(GetUsageError.Unexpected("getUsageConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(GetUsageError.Unexpected(error))
                } else {
                    Err(GetUsageError.Unexpected("getUsageConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(GetUsageError.Unexpected("getUsageConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(GetUsageError.Unexpected("getUsageConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getStateConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> {
            val ok = jv.obj?.string("content")
            if (ok != null) {
                Ok(
                    when (ok) {
                        State.ReadyToUse.name -> State.ReadyToUse
                        State.Empty.name -> State.Empty
                        State.MigrationRequired.name -> State.MigrationRequired
                        State.StateRequiresClearing.name -> State.StateRequiresClearing
                        else -> GetStateError.Unexpected("getStateConverter $unmatchedOkEnum $ok")
                    }
                )
            } else {
                Err(GetStateError.Unexpected("getStateConverter $unableToGetOk ${jv.obj?.toJsonString()}"))
            }
        }
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(GetStateError.Unexpected(error))
                } else {
                    Err(GetStateError.Unexpected("getStateConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(GetStateError.Unexpected("getStateConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(GetStateError.Unexpected("getStateConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val migrateDBConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            MigrationError.StateRequiresCleaning::class.simpleName -> MigrationError.StateRequiresCleaning
                            else -> MigrationError.Unexpected("migrateDBConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(MigrationError.Unexpected("migrateDBConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(MigrationError.Unexpected(error))
                } else {
                    Err(MigrationError.Unexpected("migrateDBConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(MigrationError.Unexpected("migrateDBConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(MigrationError.Unexpected("migrateDBConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val createAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            CreateAccountError.UsernameTaken::class.simpleName -> CreateAccountError.UsernameTaken
                            CreateAccountError.InvalidUsername::class.simpleName -> CreateAccountError.InvalidUsername
                            CreateAccountError.CouldNotReachServer::class.simpleName -> CreateAccountError.CouldNotReachServer
                            CreateAccountError.AccountExistsAlready::class.simpleName -> CreateAccountError.AccountExistsAlready
                            CreateAccountError.ClientUpdateRequired::class.simpleName -> CreateAccountError.ClientUpdateRequired
                            else -> CreateAccountError.Unexpected("createAccountConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(CreateAccountError.Unexpected("createAccountConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(CreateAccountError.Unexpected(error))
                } else {
                    Err(CreateAccountError.Unexpected("createAccountConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(CreateAccountError.Unexpected("createAccountConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(CreateAccountError.Unexpected("createAccountConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val importAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            ImportError.AccountStringCorrupted::class.simpleName -> ImportError.AccountStringCorrupted
                            ImportError.AccountExistsAlready::class.simpleName -> ImportError.AccountExistsAlready
                            ImportError.AccountDoesNotExist::class.simpleName -> ImportError.AccountDoesNotExist
                            ImportError.UsernamePKMismatch::class.simpleName -> ImportError.UsernamePKMismatch
                            ImportError.CouldNotReachServer::class.simpleName -> ImportError.CouldNotReachServer
                            ImportError.ClientUpdateRequired::class.simpleName -> ImportError.ClientUpdateRequired
                            else -> ImportError.Unexpected("importAccountConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(ImportError.Unexpected("importAccountConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(ImportError.Unexpected(error))
                } else {
                    Err(ImportError.Unexpected("importAccountConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(ImportError.Unexpected("importAccountConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(ImportError.Unexpected("importAccountConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val exportAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> {
            val ok = jv.obj?.string("content")
            if (ok != null) {
                Ok(ok)
            } else {
                Err(AccountExportError.Unexpected("exportAccountConverter $unableToGetOk ${jv.obj?.toJsonString()}"))
            }
        }
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            AccountExportError.NoAccount::class.simpleName -> AccountExportError.NoAccount
                            else -> AccountExportError.Unexpected("exportAccountConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(AccountExportError.Unexpected("exportAccountConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(AccountExportError.Unexpected(error))
                } else {
                    Err(AccountExportError.Unexpected("exportAccountConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(AccountExportError.Unexpected("exportAccountConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(AccountExportError.Unexpected("exportAccountConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> {
            val ok = jv.obj?.obj("content")
            if (ok != null) {
                Ok(Klaxon().parseFromJsonObject<Account>(ok))
            } else {
                Err(GetAccountError.Unexpected("getAccountConverter $unableToGetOk ${jv.obj?.toJsonString()}"))
            }
        }
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            GetAccountError.NoAccount::class.simpleName -> GetAccountError.NoAccount
                            else -> GetAccountError.Unexpected("getAccountConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(GetAccountError.Unexpected("getAccountConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(GetAccountError.Unexpected(error))
                } else {
                    Err(GetAccountError.Unexpected("getAccountConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(GetAccountError.Unexpected("getAccountConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(GetAccountError.Unexpected("getAccountConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val setLastSyncedConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(SetLastSyncedError.Unexpected(error))
                } else {
                    Err(SetLastSyncedError.Unexpected("setLastSyncedConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(SetLastSyncedError.Unexpected("setLastSyncedConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(SetLastSyncedError.Unexpected("setLastSyncedConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getRootConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> {
            val ok = jv.obj?.obj("content")
            if (ok != null) {
                Ok(Klaxon().parseFromJsonObject<FileMetadata>(ok))
            } else {
                Err(GetRootError.Unexpected("getRootConverter $unableToGetOk ${jv.obj?.toJsonString()}"))
            }
        }
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            GetRootError.NoRoot::class.simpleName -> GetRootError.NoRoot
                            else -> GetRootError.Unexpected("getRootConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(GetRootError.Unexpected("getRootConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(GetRootError.Unexpected(error))
                } else {
                    Err(GetRootError.Unexpected("getRootConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(GetRootError.Unexpected("getRootConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(GetRootError.Unexpected("getRootConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getChildrenConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> {
            val ok = jv.obj?.array<FileMetadata>("content")
            if (ok != null) {
                Ok(Klaxon().parseFromJsonArray<FileMetadata>(ok))
            } else {
                Err(GetChildrenError.Unexpected("getChildrenConverter $unableToGetOk ${jv.obj?.toJsonString()}"))
            }
        }
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(GetChildrenError.Unexpected(error))
                } else {
                    Err(GetChildrenError.Unexpected("getChildrenConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(GetChildrenError.Unexpected("getChildrenConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(GetChildrenError.Unexpected("getChildrenConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getFileByIdConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> {
            val ok = jv.obj?.obj("content")
            if (ok != null) {
                Ok(Klaxon().parseFromJsonObject<FileMetadata>(ok))
            } else {
                Err(GetFileByIdError.Unexpected("getFileByIdConverter $unableToGetOk ${jv.obj?.toJsonString()}"))
            }
        }
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            GetFileByIdError.NoFileWithThatId::class.simpleName -> GetFileByIdError.NoFileWithThatId
                            else -> GetFileByIdError.Unexpected("getFileByIdConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(GetFileByIdError.Unexpected("getFileByIdConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(GetFileByIdError.Unexpected(error))
                } else {
                    Err(GetFileByIdError.Unexpected("getFileByIdConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(GetFileByIdError.Unexpected("getFileByIdConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(GetFileByIdError.Unexpected("getFileByIdConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val insertFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(InsertFileError.Unexpected(error))
                } else {
                    Err(InsertFileError.Unexpected("insertFileConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(InsertFileError.Unexpected("insertFileConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(InsertFileError.Unexpected("insertFileConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val renameFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            RenameFileError.FileDoesNotExist::class.simpleName -> RenameFileError.FileDoesNotExist
                            RenameFileError.NewNameContainsSlash::class.simpleName -> RenameFileError.NewNameContainsSlash
                            RenameFileError.FileNameNotAvailable::class.simpleName -> RenameFileError.FileNameNotAvailable
                            RenameFileError.NewNameEmpty::class.simpleName -> RenameFileError.NewNameEmpty
                            RenameFileError.CannotRenameRoot::class.simpleName -> RenameFileError.CannotRenameRoot
                            else -> RenameFileError.Unexpected("renameFileConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(RenameFileError.Unexpected("renameFileConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(RenameFileError.Unexpected(error))
                } else {
                    Err(RenameFileError.Unexpected("renameFileConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(RenameFileError.Unexpected("renameFileConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(RenameFileError.Unexpected("renameFileConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val createFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> {
            val ok = jv.obj?.obj("content")
            if (ok != null) {
                Ok(Klaxon().parseFromJsonObject<FileMetadata>(ok))
            } else {
                Err(CreateFileError.Unexpected("createFileConverter $unableToGetOk ${jv.obj?.toJsonString()}"))
            }
        }
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            CreateFileError.NoAccount::class.simpleName -> CreateFileError.NoAccount
                            CreateFileError.DocumentTreatedAsFolder::class.simpleName -> CreateFileError.DocumentTreatedAsFolder
                            CreateFileError.FileNameNotAvailable::class.simpleName -> CreateFileError.FileNameNotAvailable
                            CreateFileError.CouldNotFindAParent::class.simpleName -> CreateFileError.CouldNotFindAParent
                            CreateFileError.FileNameContainsSlash::class.simpleName -> CreateFileError.FileNameContainsSlash
                            CreateFileError.FileNameEmpty::class.simpleName -> CreateFileError.FileNameEmpty
                            else -> CreateFileError.Unexpected("createFileConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(CreateFileError.Unexpected("createFileConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(CreateFileError.Unexpected(error))
                } else {
                    Err(CreateFileError.Unexpected("createFileConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(RenameFileError.Unexpected("renameFileConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(CreateFileError.Unexpected("createFileConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val deleteFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            DeleteFileError.FileDoesNotExist::class.simpleName -> DeleteFileError.FileDoesNotExist
                            else -> DeleteFileError.Unexpected("deleteFileConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(DeleteFileError.Unexpected("deleteFileConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(DeleteFileError.Unexpected(error))
                } else {
                    Err(DeleteFileError.Unexpected("deleteFileConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(DeleteFileError.Unexpected("deleteFileConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(DeleteFileError.Unexpected("deleteFileConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val readDocumentConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> {
            val ok = jv.obj?.string("content")
            if (ok != null) {
                Ok(ok)
            } else {
                Err(ReadDocumentError.Unexpected("readDocumentConverter $unableToGetOk ${jv.obj?.toJsonString()}"))
            }
        }
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            ReadDocumentError.TreatedFolderAsDocument::class.simpleName -> ReadDocumentError.TreatedFolderAsDocument
                            ReadDocumentError.NoAccount::class.simpleName -> ReadDocumentError.NoAccount
                            ReadDocumentError.FileDoesNotExist::class.simpleName -> ReadDocumentError.FileDoesNotExist
                            else -> ReadDocumentError.Unexpected("readDocumentConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(ReadDocumentError.Unexpected("readDocumentConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(ReadDocumentError.Unexpected(error))
                } else {
                    Err(ReadDocumentError.Unexpected("readDocumentConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(ReadDocumentError.Unexpected("readDocumentConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(ReadDocumentError.Unexpected("readDocumentConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val writeDocumentConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            WriteToDocumentError.FolderTreatedAsDocument::class.simpleName -> WriteToDocumentError.FolderTreatedAsDocument
                            WriteToDocumentError.NoAccount::class.simpleName -> WriteToDocumentError.NoAccount
                            WriteToDocumentError.FileDoesNotExist::class.simpleName -> WriteToDocumentError.FileDoesNotExist
                            else -> WriteToDocumentError.Unexpected("writeDocumentConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(WriteToDocumentError.Unexpected("writeDocumentConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(WriteToDocumentError.Unexpected(error))
                } else {
                    Err(WriteToDocumentError.Unexpected("writeDocumentConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(WriteToDocumentError.Unexpected("writeDocumentConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(WriteToDocumentError.Unexpected("writeDocumentConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val moveFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            MoveFileError.DocumentTreatedAsFolder::class.simpleName -> MoveFileError.DocumentTreatedAsFolder
                            MoveFileError.NoAccount::class.simpleName -> MoveFileError.NoAccount
                            MoveFileError.FileDoesNotExist::class.simpleName -> MoveFileError.FileDoesNotExist
                            MoveFileError.TargetParentDoesNotExist::class.simpleName -> MoveFileError.TargetParentDoesNotExist
                            MoveFileError.TargetParentHasChildNamedThat::class.simpleName -> MoveFileError.TargetParentHasChildNamedThat
                            MoveFileError.CannotMoveRoot::class.simpleName -> MoveFileError.CannotMoveRoot
                            else -> MoveFileError.Unexpected("moveFileConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(MoveFileError.Unexpected("moveFileConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(MoveFileError.Unexpected(error))
                } else {
                    Err(MoveFileError.Unexpected("moveFileConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(MoveFileError.Unexpected("moveFileConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(MoveFileError.Unexpected("moveFileConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val syncAllConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            SyncAllError.CouldNotReachServer::class.simpleName -> SyncAllError.CouldNotReachServer
                            SyncAllError.NoAccount::class.simpleName -> SyncAllError.NoAccount
                            SyncAllError.ExecuteWorkError::class.simpleName -> SyncAllError.ExecuteWorkError
                            else -> SyncAllError.Unexpected("syncAllConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(SyncAllError.Unexpected("syncAllConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(SyncAllError.Unexpected(error))
                } else {
                    Err(SyncAllError.Unexpected("syncAllConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(SyncAllError.Unexpected("syncAllConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(SyncAllError.Unexpected("syncAllConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val calculateSyncWorkConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> {
            val ok = jv.obj?.obj("content")
            if (ok != null) {
                Ok(Klaxon().parseFromJsonObject<WorkCalculated>(ok))
            } else {
                Err(CalculateWorkError.Unexpected("calculateSyncWorkConverter $unableToGetOk ${jv.obj?.toJsonString()}"))
            }
        }
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            CalculateWorkError.CouldNotReachServer::class.simpleName -> CalculateWorkError.CouldNotReachServer
                            CalculateWorkError.NoAccount::class.simpleName -> CalculateWorkError.NoAccount
                            CalculateWorkError.ClientUpdateRequired::class.simpleName -> CalculateWorkError.ClientUpdateRequired
                            else -> CalculateWorkError.Unexpected("calculateSyncWorkConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(CalculateWorkError.Unexpected("calculateSyncWorkConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(CalculateWorkError.Unexpected(error))
                } else {
                    Err(CalculateWorkError.Unexpected("calculateSyncWorkConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(CalculateWorkError.Unexpected("calculateSyncWorkConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(CalculateWorkError.Unexpected("calculateSyncWorkConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val executeSyncWorkConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (val errorTag = jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            ExecuteWorkError.CouldNotReachServer::class.simpleName -> ExecuteWorkError.CouldNotReachServer
                            ExecuteWorkError.ClientUpdateRequired::class.simpleName -> ExecuteWorkError.ClientUpdateRequired
                            else -> ExecuteWorkError.Unexpected("executeSyncWorkConverter $unmatchedUiError $error")
                        }
                    )
                } else {
                    Err(ExecuteWorkError.Unexpected("executeSyncWorkConverter $unableToGetUiError ${jv.obj?.toJsonString()}"))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(ExecuteWorkError.Unexpected(error))
                } else {
                    Err(ExecuteWorkError.Unexpected("executeSyncWorkConverter $unableToGetUnexpectedError ${jv.obj?.toJsonString()}"))
                }
            }
            else -> Err(ExecuteWorkError.Unexpected("executeSyncWorkConverter $unmatchedErrorTag $errorTag"))
        }
        else -> Err(ExecuteWorkError.Unexpected("executeSyncWorkConverter $unmatchedTag ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}
