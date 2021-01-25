package app.lockbook.util

val <T> T.exhaustive: T
    get() = this

sealed class CoreError

sealed class InitLoggerError : CoreError() {
    data class Unexpected(val error: String) : InitLoggerError()
}

sealed class GetUsageError : CoreError() {
    object NoAccount : GetUsageError()
    object CouldNotReachServer : GetUsageError()
    object ClientUpdateRequired : GetUsageError()
    data class Unexpected(val error: String) : GetUsageError()
}

sealed class GetLastSynced : CoreError() {
    data class Unexpected(val error: String) : GetLastSynced()
}

sealed class GetStateError : CoreError() {
    data class Unexpected(val error: String) : GetStateError()
}

sealed class MigrationError : CoreError() {
    object StateRequiresCleaning : MigrationError()
    data class Unexpected(val error: String) : MigrationError()
}

sealed class CreateAccountError : CoreError() {
    object UsernameTaken : CreateAccountError()
    object InvalidUsername : CreateAccountError()
    object CouldNotReachServer : CreateAccountError()
    object AccountExistsAlready : CreateAccountError()
    object ClientUpdateRequired : CreateAccountError()
    data class Unexpected(val error: String) : CreateAccountError()
}

sealed class ImportError : CoreError() {
    object AccountStringCorrupted : ImportError()
    object AccountExistsAlready : ImportError()
    object AccountDoesNotExist : ImportError()
    object UsernamePKMismatch : ImportError()
    object CouldNotReachServer : ImportError()
    object ClientUpdateRequired : ImportError()
    data class Unexpected(val error: String) : ImportError()
}

sealed class AccountExportError : CoreError() {
    object NoAccount : AccountExportError()
    data class Unexpected(val error: String) : AccountExportError()
}

sealed class GetAccountError : CoreError() {
    object NoAccount : GetAccountError()
    data class Unexpected(val error: String) : GetAccountError()
}

sealed class SetLastSyncedError : CoreError() {
    data class Unexpected(val error: String) : SetLastSyncedError()
}

sealed class GetRootError : CoreError() {
    object NoRoot : GetRootError()
    data class Unexpected(val error: String) : GetRootError()
}

sealed class WriteToDocumentError : CoreError() {
    object NoAccount : WriteToDocumentError()
    object FileDoesNotExist : WriteToDocumentError()
    object FolderTreatedAsDocument : WriteToDocumentError()
    data class Unexpected(val error: String) : WriteToDocumentError()
}

sealed class CreateFileError : CoreError() {
    object NoAccount : CreateFileError()
    object DocumentTreatedAsFolder : CreateFileError()
    object CouldNotFindAParent : CreateFileError()
    object FileNameNotAvailable : CreateFileError()
    object FileNameContainsSlash : CreateFileError()
    object FileNameEmpty : CreateFileError()
    data class Unexpected(val error: String) : CreateFileError()
}

sealed class GetChildrenError : CoreError() {
    data class Unexpected(val error: String) : GetChildrenError()
}

sealed class GetFileByIdError : CoreError() {
    object NoFileWithThatId : GetFileByIdError()
    data class Unexpected(val error: String) : GetFileByIdError()
}

sealed class InsertFileError : CoreError() {
    data class Unexpected(val error: String) : InsertFileError()
}

sealed class FileDeleteError : CoreError() {
    object FileDoesNotExist : FileDeleteError()
    object CannotDeleteRoot : FileDeleteError()
    data class Unexpected(val error: String) : FileDeleteError()
}

sealed class ReadDocumentError : CoreError() {
    object TreatedFolderAsDocument : ReadDocumentError()
    object NoAccount : ReadDocumentError()
    object FileDoesNotExist : ReadDocumentError()
    data class Unexpected(val error: String) : ReadDocumentError()
}

sealed class RenameFileError : CoreError() {
    object FileDoesNotExist : RenameFileError()
    object NewNameContainsSlash : RenameFileError()
    object FileNameNotAvailable : RenameFileError()
    object NewNameEmpty : RenameFileError()
    object CannotRenameRoot : RenameFileError()
    data class Unexpected(val error: String) : RenameFileError()
}

sealed class MoveFileError : CoreError() {
    object NoAccount : MoveFileError()
    object FileDoesNotExist : MoveFileError()
    object DocumentTreatedAsFolder : MoveFileError()
    object TargetParentDoesNotExist : MoveFileError()
    object TargetParentHasChildNamedThat : MoveFileError()
    object CannotMoveRoot : MoveFileError()
    object FolderMovedIntoItself : MoveFileError()
    data class Unexpected(val error: String) : MoveFileError()
}

sealed class SyncAllError : CoreError() {
    object NoAccount : SyncAllError()
    object CouldNotReachServer : SyncAllError()
    object ExecuteWorkError : SyncAllError()
    object ClientUpdateRequired : SyncAllError()
    data class Unexpected(val error: String) : SyncAllError()
}

sealed class CalculateWorkError : CoreError() {
    object NoAccount : CalculateWorkError()
    object CouldNotReachServer : CalculateWorkError()
    object ClientUpdateRequired : CalculateWorkError()
    data class Unexpected(val error: String) : CalculateWorkError()
}

sealed class ExecuteWorkError : CoreError() {
    object CouldNotReachServer : ExecuteWorkError()
    object ClientUpdateRequired : ExecuteWorkError()
    object BadAccount : ExecuteWorkError()
    data class Unexpected(val error: String) : ExecuteWorkError()
}
