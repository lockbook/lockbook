package app.lockbook.util

val <T> T.exhaustive: T
    get() = this

sealed class InitLoggerError {
    data class Unexpected(val error: String) : InitLoggerError()
}

sealed class GetUsageError {
    object NoAccount : GetUsageError()
    object CouldNotReachServer : GetUsageError()
    object ClientUpdateRequired : GetUsageError()
    data class Unexpected(val error: String) : GetUsageError()
}

sealed class GetStateError {
    data class Unexpected(val error: String) : GetStateError()
}

sealed class MigrationError {
    object StateRequiresCleaning : MigrationError()
    data class Unexpected(val error: String) : MigrationError()
}

sealed class CreateAccountError {
    object UsernameTaken : CreateAccountError()
    object InvalidUsername : CreateAccountError()
    object CouldNotReachServer : CreateAccountError()
    object AccountExistsAlready : CreateAccountError()
    object ClientUpdateRequired : CreateAccountError()
    data class Unexpected(val error: String) : CreateAccountError()
}

sealed class ImportError {
    object AccountStringCorrupted : ImportError()
    object AccountExistsAlready : ImportError()
    object AccountDoesNotExist : ImportError()
    object UsernamePKMismatch : ImportError()
    object CouldNotReachServer : ImportError()
    object ClientUpdateRequired : ImportError()
    data class Unexpected(val error: String) : ImportError()
}

sealed class AccountExportError {
    object NoAccount : AccountExportError()
    data class Unexpected(val error: String) : AccountExportError()
}

sealed class GetAccountError {
    object NoAccount : GetAccountError()
    data class Unexpected(val error: String) : GetAccountError()
}

sealed class SetLastSyncedError {
    data class Unexpected(val error: String) : SetLastSyncedError()
}

sealed class GetRootError {
    object NoRoot : GetRootError()
    data class Unexpected(val error: String) : GetRootError()
}

sealed class WriteToDocumentError {
    object NoAccount : WriteToDocumentError()
    object FileDoesNotExist : WriteToDocumentError()
    object FolderTreatedAsDocument : WriteToDocumentError()
    data class Unexpected(val error: String) : WriteToDocumentError()
}

sealed class CreateFileError {
    object NoAccount : CreateFileError()
    object DocumentTreatedAsFolder : CreateFileError()
    object CouldNotFindAParent : CreateFileError()
    object FileNameNotAvailable : CreateFileError()
    object FileNameContainsSlash : CreateFileError()
    object FileNameEmpty : CreateFileError()
    data class Unexpected(val error: String) : CreateFileError()
}

sealed class GetChildrenError {
    data class Unexpected(val error: String) : GetChildrenError()
}

sealed class GetFileByIdError {
    object NoFileWithThatId : GetFileByIdError()
    data class Unexpected(val error: String) : GetFileByIdError()
}

sealed class InsertFileError {
    data class Unexpected(val error: String) : InsertFileError()
}

sealed class DeleteFileError {
    object FileDoesNotExist : DeleteFileError()
    data class Unexpected(val error: String) : DeleteFileError()
}

sealed class ReadDocumentError {
    object TreatedFolderAsDocument : ReadDocumentError()
    object NoAccount : ReadDocumentError()
    object FileDoesNotExist : ReadDocumentError()
    data class Unexpected(val error: String) : ReadDocumentError()
}

sealed class RenameFileError {
    object FileDoesNotExist : RenameFileError()
    object NewNameContainsSlash : RenameFileError()
    object FileNameNotAvailable : RenameFileError()
    object NewNameEmpty : RenameFileError()
    object CannotRenameRoot : RenameFileError()
    data class Unexpected(val error: String) : RenameFileError()
}

sealed class MoveFileError {
    object NoAccount : MoveFileError()
    object FileDoesNotExist : MoveFileError()
    object DocumentTreatedAsFolder : MoveFileError()
    object TargetParentDoesNotExist : MoveFileError()
    object TargetParentHasChildNamedThat : MoveFileError()
    object CannotMoveRoot : MoveFileError()
    data class Unexpected(val error: String) : MoveFileError()
}

sealed class SyncAllError {
    object NoAccount : SyncAllError()
    object CouldNotReachServer : SyncAllError()
    object ExecuteWorkError : SyncAllError()
    data class Unexpected(val error: String) : SyncAllError()
}

sealed class CalculateWorkError {
    object NoAccount : CalculateWorkError()
    object CouldNotReachServer : CalculateWorkError()
    object ClientUpdateRequired : CalculateWorkError()
    data class Unexpected(val error: String) : CalculateWorkError()
}

sealed class ExecuteWorkError {
    object CouldNotReachServer : ExecuteWorkError()
    object ClientUpdateRequired : ExecuteWorkError()
    data class Unexpected(val error: String) : ExecuteWorkError()
}
