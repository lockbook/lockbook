package app.lockbook.utils

import com.beust.klaxon.Converter
import com.beust.klaxon.JsonValue
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok

const val okTag = "Ok"
const val errTag = "Err"
const val unexpectedTag = "Unexpected"
const val uiErrorTag = "UiError"
const val unmatchedTag = "Couldn't match outermost tag to anything. Json: "
const val unmatchedErrorTag = "Couldn't match error tag to anything: "
const val unmatchedUiError = "Couldn't match a type of UiError: "
const val unableToGetUiError = "Couldn't get UiError type from content. Json: "
const val unableToGetUnexpectedError = "Couldn't get UnexpectedError message from content. Json: "
const val unableToGetOk = "Couldn't get Ok tag content. Json: "

val initLoggerConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(InitLoggerError.Unexpected(error))
                } else {
                    Err(InitLoggerError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(InitLoggerError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(InitLoggerError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
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
                        else -> GetStateError.Unexpected(unmatchedUiError + ok)
                    }
                )
            } else {
                Err(GetStateError.Unexpected(unableToGetOk + jv.obj?.toJsonString()))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(GetStateError.Unexpected(error))
                } else {
                    Err(GetStateError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(GetStateError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(GetStateError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val migrateDBConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            MigrationError.StateRequiresCleaning::class.simpleName -> MigrationError.StateRequiresCleaning
                            else -> MigrationError.Unexpected(unmatchedUiError + error)
                        }
                    )
                } else {
                    Err(MigrationError.Unexpected(unableToGetUiError + jv.obj?.toJsonString()))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(MigrationError.Unexpected(error))
                } else {
                    Err(MigrationError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(MigrationError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(MigrationError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val createAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
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
                            else -> CreateAccountError.Unexpected(unmatchedUiError + error)
                        }
                    )
                } else {
                    Err(CreateAccountError.Unexpected(unableToGetUiError + jv.obj?.toJsonString()))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(CreateAccountError.Unexpected(error))
                } else {
                    Err(CreateAccountError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(CreateAccountError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(CreateAccountError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val importAccountConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
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
                            else -> ImportError.Unexpected(unmatchedUiError + error)
                        }
                    )
                } else {
                    Err(ImportError.Unexpected(unableToGetUiError + jv.obj?.toJsonString()))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(ImportError.Unexpected(error))
                } else {
                    Err(ImportError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(ImportError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(ImportError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
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
                Err(AccountExportError.Unexpected(unableToGetOk + jv.obj?.toJsonString()))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            AccountExportError.NoAccount::class.simpleName -> AccountExportError.NoAccount
                            else -> AccountExportError.Unexpected(unmatchedUiError + error)
                        }
                    )
                } else {
                    Err(AccountExportError.Unexpected(unableToGetUiError + jv.obj?.toJsonString()))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(AccountExportError.Unexpected(error))
                } else {
                    Err(AccountExportError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(AccountExportError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(AccountExportError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
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
                Err(GetAccountError.Unexpected(unableToGetOk + jv.obj?.toJsonString()))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            GetAccountError.NoAccount::class.simpleName -> GetAccountError.NoAccount
                            else -> GetAccountError.Unexpected(unmatchedUiError + error)
                        }
                    )
                } else {
                    Err(GetAccountError.Unexpected(unableToGetUiError + jv.obj?.toJsonString()))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(GetAccountError.Unexpected(error))
                } else {
                    Err(GetAccountError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(GetAccountError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(GetAccountError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val setLastSyncedConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(SetLastSyncedError.Unexpected(error))
                } else {
                    Err(SetLastSyncedError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(SetLastSyncedError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(SetLastSyncedError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
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
                Err(GetRootError.Unexpected(unableToGetOk + jv.obj?.toJsonString()))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            GetRootError.NoRoot::class.simpleName -> GetRootError.NoRoot
                            else -> GetRootError.Unexpected(unmatchedUiError + error)
                        }
                    )
                } else {
                    Err(GetRootError.Unexpected(unableToGetUiError + jv.obj?.toJsonString()))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(GetRootError.Unexpected(error))
                } else {
                    Err(GetRootError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(GetRootError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(GetRootError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
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
                Err(GetChildrenError.Unexpected(unableToGetOk + jv.obj?.toJsonString()))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(GetChildrenError.Unexpected(error))
                } else {
                    Err(GetChildrenError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(GetChildrenError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(GetChildrenError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
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
                Err(GetFileByIdError.Unexpected(unableToGetOk + jv.obj?.toJsonString()))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            GetFileByIdError.NoFileWithThatId::class.simpleName -> GetFileByIdError.NoFileWithThatId
                            else -> GetFileByIdError.Unexpected(unmatchedUiError + error)
                        }
                    )
                } else {
                    Err(GetFileByIdError.Unexpected(unableToGetUiError + jv.obj?.toJsonString()))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(GetFileByIdError.Unexpected(error))
                } else {
                    Err(GetFileByIdError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(GetFileByIdError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(GetFileByIdError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val insertFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(InsertFileError.Unexpected(error))
                } else {
                    Err(InsertFileError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(InsertFileError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(InsertFileError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val renameFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
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
                            else -> RenameFileError.Unexpected(unmatchedUiError + error)
                        }
                    )
                } else {
                    Err(RenameFileError.Unexpected(unableToGetUiError + jv.obj?.toJsonString()))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(RenameFileError.Unexpected(error))
                } else {
                    Err(RenameFileError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(RenameFileError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(RenameFileError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
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
                Err(CreateFileError.Unexpected(unableToGetOk + jv.obj?.toJsonString()))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
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
                            else -> CreateFileError.Unexpected(unmatchedUiError + error)
                        }
                    )
                } else {
                    Err(CreateFileError.Unexpected(unableToGetUiError + jv.obj?.toJsonString()))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(CreateFileError.Unexpected(error))
                } else {
                    Err(CreateFileError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(CreateFileError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(CreateFileError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val deleteFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            DeleteFileError.NoFileWithThatId::class.simpleName -> DeleteFileError.NoFileWithThatId
                            else -> DeleteFileError.Unexpected(unmatchedUiError + error)
                        }
                    )
                } else {
                    Err(DeleteFileError.Unexpected(unableToGetUiError + jv.obj?.toJsonString()))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(DeleteFileError.Unexpected(error))
                } else {
                    Err(DeleteFileError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(DeleteFileError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(DeleteFileError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val readDocumentConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> {
            val ok = jv.obj?.obj("content")
            if (ok != null) {
                Ok(Klaxon().parseFromJsonObject<DecryptedValue>(ok))
            } else {
                Err(ReadDocumentError.Unexpected(unableToGetOk + jv.obj?.toJsonString()))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            ReadDocumentError.TreatedFolderAsDocument::class.simpleName -> ReadDocumentError.TreatedFolderAsDocument
                            ReadDocumentError.NoAccount::class.simpleName -> ReadDocumentError.NoAccount
                            ReadDocumentError.FileDoesNotExist::class.simpleName -> ReadDocumentError.FileDoesNotExist
                            else -> ReadDocumentError.Unexpected(unmatchedUiError + error)
                        }
                    )
                } else {
                    Err(ReadDocumentError.Unexpected(unableToGetUiError + jv.obj?.toJsonString()))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(ReadDocumentError.Unexpected(error))
                } else {
                    Err(ReadDocumentError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(ReadDocumentError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(ReadDocumentError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val writeDocumentConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            WriteToDocumentError.FolderTreatedAsDocument::class.simpleName -> WriteToDocumentError.FolderTreatedAsDocument
                            WriteToDocumentError.NoAccount::class.simpleName -> WriteToDocumentError.NoAccount
                            WriteToDocumentError.FileDoesNotExist::class.simpleName -> WriteToDocumentError.FileDoesNotExist
                            else -> WriteToDocumentError.Unexpected(unmatchedUiError + error)
                        }
                    )
                } else {
                    Err(WriteToDocumentError.Unexpected(unableToGetUiError + jv.obj?.toJsonString()))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(WriteToDocumentError.Unexpected(error))
                } else {
                    Err(WriteToDocumentError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(WriteToDocumentError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(WriteToDocumentError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val moveFileConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
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
                            else -> MoveFileError.Unexpected(unmatchedUiError + error)
                        }
                    )
                } else {
                    Err(MoveFileError.Unexpected(unableToGetUiError + jv.obj?.toJsonString()))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(MoveFileError.Unexpected(error))
                } else {
                    Err(MoveFileError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(MoveFileError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(MoveFileError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val syncAllConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            SyncAllError.CouldNotReachServer::class.simpleName -> SyncAllError.CouldNotReachServer
                            SyncAllError.NoAccount::class.simpleName -> SyncAllError.NoAccount
                            SyncAllError.ExecuteWorkError::class.simpleName -> SyncAllError.ExecuteWorkError
                            else -> SyncAllError.Unexpected(unmatchedUiError + error)
                        }
                    )
                } else {
                    Err(SyncAllError.Unexpected(unableToGetUiError + jv.obj?.toJsonString()))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(SyncAllError.Unexpected(error))
                } else {
                    Err(SyncAllError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(SyncAllError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(SyncAllError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
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
                Err(CalculateWorkError.Unexpected(unableToGetOk + jv.obj?.toJsonString()))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            CalculateWorkError.CouldNotReachServer::class.simpleName -> CalculateWorkError.CouldNotReachServer
                            CalculateWorkError.NoAccount::class.simpleName -> CalculateWorkError.NoAccount
                            CalculateWorkError.ClientUpdateRequired::class.simpleName -> CalculateWorkError.ClientUpdateRequired
                            else -> CalculateWorkError.Unexpected(unmatchedUiError + error)
                        }
                    )
                } else {
                    Err(CalculateWorkError.Unexpected(unableToGetUiError + jv.obj?.toJsonString()))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(CalculateWorkError.Unexpected(error))
                } else {
                    Err(CalculateWorkError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(CalculateWorkError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(CalculateWorkError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val executeSyncWorkConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
        okTag -> Ok(Unit)
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(
                        when (error) {
                            ExecuteWorkError.CouldNotReachServer::class.simpleName -> ExecuteWorkError.CouldNotReachServer
                            ExecuteWorkError.ClientUpdateRequired::class.simpleName -> ExecuteWorkError.ClientUpdateRequired
                            else -> ExecuteWorkError.Unexpected(unmatchedUiError + error)
                        }
                    )
                } else {
                    Err(ExecuteWorkError.Unexpected(unableToGetUiError + jv.obj?.toJsonString()))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(ExecuteWorkError.Unexpected(error))
                } else {
                    Err(ExecuteWorkError.Unexpected(unableToGetUnexpectedError + jv.obj?.toJsonString()))
                }
            }
            else -> Err(ExecuteWorkError.Unexpected(unmatchedErrorTag + jv.obj?.toJsonString()))
        }
        else -> Err(ExecuteWorkError.Unexpected(unmatchedTag + jv.obj?.toJsonString()))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}
