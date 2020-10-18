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
                    Err(InitLoggerError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(InitLoggerError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(InitLoggerError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

val getStateConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? = when (jv.obj?.string("tag")) {
            okTag -> {
                val ok = jv.obj?.string("content")
                if (ok != null) {
                    Err(when(ok) {
                        State.ReadyToUse.name -> State.ReadyToUse
                        State.Empty.name -> State.Empty
                        State.MigrationRequired.name -> State.MigrationRequired
                        State.StateRequiresClearing.name -> State.StateRequiresClearing
                        else -> {}
                    })
                } else {
                    Err(GetStateError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            errTag -> when (jv.obj?.obj("content")?.string("tag")) {
                unexpectedTag -> {
                    val error = jv.obj?.obj("content")?.string("content")
                    if (error != null) {
                        Err(GetStateError.Unexpected(error))
                    } else {
                        Err(GetStateError.Unexpected("Can't receive contents from UnexpectedError."))
                    }
                }
                else -> Err(GetStateError.Unexpected("Can't recognize an error tag."))
            }
            else -> Err(GetStateError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                    Err(when(error) {
                        MigrationError.StateRequiresCleaning::class.simpleName -> MigrationError.StateRequiresCleaning
                        else -> MigrationError.Unexpected("Can't recognize UiError content.")
                    })
                } else {
                    Err(MigrationError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(MigrationError.Unexpected(error))
                } else {
                    Err(MigrationError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(MigrationError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(MigrationError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                    Err(when(error) {
                        CreateAccountError.UsernameTaken::class.simpleName -> CreateAccountError.UsernameTaken
                        CreateAccountError.InvalidUsername::class.simpleName -> CreateAccountError.InvalidUsername
                        CreateAccountError.CouldNotReachServer::class.simpleName -> CreateAccountError.CouldNotReachServer
                        CreateAccountError.AccountExistsAlready::class.simpleName -> CreateAccountError.AccountExistsAlready
                        else -> CreateAccountError.Unexpected("Can't recognize UiError content.")
                    })
                } else {
                    Err(CreateAccountError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(CreateAccountError.Unexpected(error))
                } else {
                    Err(CreateAccountError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(CreateAccountError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(CreateAccountError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                    Err(when(error) {
                        ImportError.AccountStringCorrupted::class.simpleName -> ImportError.AccountStringCorrupted
                        ImportError.AccountExistsAlready::class.simpleName -> ImportError.AccountExistsAlready
                        ImportError.AccountDoesNotExist::class.simpleName -> ImportError.AccountDoesNotExist
                        ImportError.UsernamePKMismatch::class.simpleName -> ImportError.UsernamePKMismatch
                        ImportError.CouldNotReachServer::class.simpleName -> ImportError.CouldNotReachServer
                        else -> ImportError.Unexpected("Can't recognize UiError content.")
                    })
                } else {
                    Err(ImportError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(ImportError.Unexpected(error))
                } else {
                    Err(ImportError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(ImportError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(ImportError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                Err(AccountExportError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(when(error) {
                        AccountExportError.NoAccount::class.simpleName -> AccountExportError.NoAccount
                        else -> AccountExportError.Unexpected("Can't recognize UiError content.")
                    })
                } else {
                    Err(AccountExportError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(AccountExportError.Unexpected(error))
                } else {
                    Err(AccountExportError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(AccountExportError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(AccountExportError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                Err(GetAccountError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(when(error) {
                        GetAccountError.NoAccount::class.simpleName -> GetAccountError.NoAccount
                        else -> GetAccountError.Unexpected("Can't recognize UiError content.")
                    })
                } else {
                    Err(GetAccountError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(GetAccountError.Unexpected(error))
                } else {
                    Err(GetAccountError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(GetAccountError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(GetAccountError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                    Err(SetLastSyncedError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(SetLastSyncedError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(SetLastSyncedError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                Err(GetRootError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(when(error) {
                        GetRootError.NoRoot::class.simpleName -> GetRootError.NoRoot
                        else -> GetRootError.Unexpected("Can't recognize UiError content.")
                    })
                } else {
                    Err(GetRootError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(GetRootError.Unexpected(error))
                } else {
                    Err(GetRootError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(GetRootError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(GetRootError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                Err(GetChildrenError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(GetChildrenError.Unexpected(error))
                } else {
                    Err(GetChildrenError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(GetChildrenError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(GetChildrenError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                Err(GetFileByIdError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(when(error) {
                        GetFileByIdError.NoFileWithThatId::class.simpleName -> GetFileByIdError.NoFileWithThatId
                        else -> GetFileByIdError.Unexpected("Can't recognize UiError content.")
                    })
                } else {
                    Err(GetFileByIdError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(GetFileByIdError.Unexpected(error))
                } else {
                    Err(GetFileByIdError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(GetFileByIdError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(GetFileByIdError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                    Err(InsertFileError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(InsertFileError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(InsertFileError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                    Err(when(error) {
                        RenameFileError.FileDoesNotExist::class.simpleName -> RenameFileError.FileDoesNotExist
                        RenameFileError.NewNameContainsSlash::class.simpleName -> RenameFileError.NewNameContainsSlash
                        RenameFileError.FileNameNotAvailable::class.simpleName -> RenameFileError.FileNameNotAvailable
                        RenameFileError.NewNameEmpty::class.simpleName -> RenameFileError.NewNameEmpty
                        RenameFileError.CannotRenameRoot::class.simpleName -> RenameFileError.CannotRenameRoot
                        else -> RenameFileError.Unexpected("Can't recognize UiError content.")
                    })
                } else {
                    Err(RenameFileError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(RenameFileError.Unexpected(error))
                } else {
                    Err(RenameFileError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(RenameFileError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(RenameFileError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                Err(CreateFileError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(when(error) {
                        CreateFileError.NoAccount::class.simpleName -> CreateFileError.NoAccount
                        CreateFileError.DocumentTreatedAsFolder::class.simpleName -> CreateFileError.DocumentTreatedAsFolder
                        CreateFileError.FileNameNotAvailable::class.simpleName -> CreateFileError.FileNameNotAvailable
                        CreateFileError.CouldNotFindAParent::class.simpleName -> CreateFileError.CouldNotFindAParent
                        CreateFileError.FileNameContainsSlash::class.simpleName -> CreateFileError.FileNameContainsSlash
                        CreateFileError.FileNameEmpty::class.simpleName -> CreateFileError.FileNameEmpty
                        else -> CreateFileError.Unexpected("Can't recognize UiError content.")
                    })
                } else {
                    Err(CreateFileError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(CreateFileError.Unexpected(error))
                } else {
                    Err(CreateFileError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(CreateFileError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(CreateFileError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                    Err(when(error) {
                        DeleteFileError.NoFileWithThatId::class.simpleName -> DeleteFileError.NoFileWithThatId
                        else -> DeleteFileError.Unexpected("Can't recognize UiError content.")
                    })
                } else {
                    Err(DeleteFileError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(DeleteFileError.Unexpected(error))
                } else {
                    Err(DeleteFileError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(DeleteFileError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(DeleteFileError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                Err(ReadDocumentError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(when(error) {
                        ReadDocumentError.TreatedFolderAsDocument::class.simpleName -> ReadDocumentError.TreatedFolderAsDocument
                        ReadDocumentError.NoAccount::class.simpleName -> ReadDocumentError.NoAccount
                        ReadDocumentError.FileDoesNotExist::class.simpleName -> ReadDocumentError.FileDoesNotExist
                        else -> ReadDocumentError.Unexpected("Can't recognize UiError content.")
                    })
                } else {
                    Err(ReadDocumentError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(ReadDocumentError.Unexpected(error))
                } else {
                    Err(ReadDocumentError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(ReadDocumentError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(ReadDocumentError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                    Err(when(error) {
                        WriteToDocumentError.FolderTreatedAsDocument::class.simpleName -> WriteToDocumentError.FolderTreatedAsDocument
                        WriteToDocumentError.NoAccount::class.simpleName -> WriteToDocumentError.NoAccount
                        WriteToDocumentError.FileDoesNotExist::class.simpleName -> WriteToDocumentError.FileDoesNotExist
                        else -> WriteToDocumentError.Unexpected("Can't recognize UiError content.")
                    })
                } else {
                    Err(WriteToDocumentError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(WriteToDocumentError.Unexpected(error))
                } else {
                    Err(WriteToDocumentError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(WriteToDocumentError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(WriteToDocumentError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                    Err(when(error) {
                        MoveFileError.DocumentTreatedAsFolder::class.simpleName -> MoveFileError.DocumentTreatedAsFolder
                        MoveFileError.NoAccount::class.simpleName -> MoveFileError.NoAccount
                        MoveFileError.FileDoesNotExist::class.simpleName -> MoveFileError.FileDoesNotExist
                        MoveFileError.TargetParentDoesNotExist::class.simpleName -> MoveFileError.TargetParentDoesNotExist
                        MoveFileError.TargetParentHasChildNamedThat::class.simpleName -> MoveFileError.TargetParentHasChildNamedThat
                        MoveFileError.CannotMoveRoot::class.simpleName -> MoveFileError.CannotMoveRoot
                        else -> MoveFileError.Unexpected("Can't recognize UiError content.")
                    })
                } else {
                    Err(MoveFileError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(MoveFileError.Unexpected(error))
                } else {
                    Err(MoveFileError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(MoveFileError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(MoveFileError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                    Err(when(error) {
                        SyncAllError.CouldNotReachServer::class.simpleName -> SyncAllError.CouldNotReachServer
                        SyncAllError.NoAccount::class.simpleName -> SyncAllError.NoAccount
                        SyncAllError.ExecuteWorkError::class.simpleName -> SyncAllError.ExecuteWorkError
                        else -> SyncAllError.Unexpected("Can't recognize UiError content.")
                    })
                } else {
                    Err(SyncAllError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(SyncAllError.Unexpected(error))
                } else {
                    Err(SyncAllError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(SyncAllError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(SyncAllError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                Err(CalculateWorkError.Unexpected("Can't receive contents from UnexpectedError."))
            }
        }
        errTag -> when (jv.obj?.obj("content")?.string("tag")) {
            uiErrorTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(when(error) {
                        CalculateWorkError.CouldNotReachServer::class.simpleName -> CalculateWorkError.CouldNotReachServer
                        CalculateWorkError.NoAccount::class.simpleName -> CalculateWorkError.NoAccount
                        else -> CalculateWorkError.Unexpected("Can't recognize UiError content.")
                    })
                } else {
                    Err(CalculateWorkError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(CalculateWorkError.Unexpected(error))
                } else {
                    Err(CalculateWorkError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(CalculateWorkError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(CalculateWorkError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
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
                    Err(when(error) {
                        ExecuteWorkError.CouldNotReachServer::class.simpleName -> ExecuteWorkError.CouldNotReachServer
                        else -> ExecuteWorkError.Unexpected("Can't recognize UiError content.")
                    })
                } else {
                    Err(ExecuteWorkError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            unexpectedTag -> {
                val error = jv.obj?.obj("content")?.string("content")
                if (error != null) {
                    Err(ExecuteWorkError.Unexpected(error))
                } else {
                    Err(ExecuteWorkError.Unexpected("Can't receive contents from UnexpectedError."))
                }
            }
            else -> Err(ExecuteWorkError.Unexpected("Can't recognize an error tag."))
        }
        else -> Err(ExecuteWorkError.Unexpected("Unable to parse tag: ${jv.obj?.toJsonString()}"))
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}
