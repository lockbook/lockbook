package app.lockbook.utils

sealed class InitLoggerError {
    data class Unexpected(val error: String) : InitLoggerError()
}

sealed class CreateAccountError {
    object UsernameTaken : CreateAccountError()
    object InvalidUsername : CreateAccountError()
    object CouldNotReachServer : CreateAccountError()
    object AccountExistsAlready : CreateAccountError()
    data class UnexpectedError(val error: String) : CreateAccountError()
}

sealed class ImportError {
    object AccountStringCorrupted : ImportError()
    object AccountExistsAlready : ImportError()
    object AccountDoesNotExist : ImportError()
    object UsernamePKMismatch : ImportError()
    object CouldNotReachServer : ImportError()
    data class UnexpectedError(val error: String) : ImportError()
}

sealed class AccountExportError {
    object NoAccount : AccountExportError()
    data class UnexpectedError(val error: String) : AccountExportError()
}

sealed class GetAccountError {
    object NoAccount : GetAccountError()
    data class UnexpectedError(val error: String) : GetAccountError()
}

sealed class SetLastSyncedError {
    data class UnexpectedError(val error: String) : SetLastSyncedError()
}

sealed class WriteToDocumentError {
    object NoAccount : WriteToDocumentError()
    object FileDoesNotExist : WriteToDocumentError()
    object FolderTreatedAsDocument : WriteToDocumentError()
    data class UnexpectedError(val error: String) : WriteToDocumentError()
}

sealed class CreateFileError {
    object NoAccount : CreateFileError()
    object DocumentTreatedAsFolder : CreateFileError()
    object CouldNotFindAParent : CreateFileError()
    object FileNameNotAvailable : CreateFileError()
    object FileNameContainsSlash : CreateFileError()
    object FileNameEmpty : CreateFileError()
    data class UnexpectedError(val error: String) : CreateFileError()
}

sealed class GetRootError {
    object NoRoot : GetRootError()
    data class UnexpectedError(val error: String) : GetRootError()
}

sealed class GetChildrenError {
    data class UnexpectedError(val error: String) : GetChildrenError()
}

sealed class GetFileByIdError {
    object NoFileWithThatId : GetFileByIdError()
    data class UnexpectedError(val error: String) : GetFileByIdError()
}

sealed class InsertFileError {
    data class UnexpectedError(val error: String) : InsertFileError()
}

sealed class DeleteFileError {
    object NoFileWithThatId : DeleteFileError()
    data class UnexpectedError(val error: String) : DeleteFileError()
}

sealed class ReadDocumentError {
    object TreatedFolderAsDocument : ReadDocumentError()
    object NoAccount : ReadDocumentError()
    object FileDoesNotExist : ReadDocumentError()
    data class UnexpectedError(val error: String) : ReadDocumentError()
}

sealed class RenameFileError {
    object FileDoesNotExist : RenameFileError()
    object NewNameContainsSlash : RenameFileError()
    object FileNameNotAvailable : RenameFileError()
    object NewNameEmpty : RenameFileError()
    object CannotRenameRoot : RenameFileError()
    data class UnexpectedError(val error: String) : RenameFileError()
}

sealed class MoveFileError {
    object NoAccount : MoveFileError()
    object FileDoesNotExist : MoveFileError()
    object DocumentTreatedAsFolder : MoveFileError()
    object TargetParentDoesNotExist : MoveFileError()
    object TargetParentHasChildNamedThat : MoveFileError()
    object CannotMoveRoot : MoveFileError()
    data class UnexpectedError(val error: String) : MoveFileError()
}

sealed class SyncAllError {
    object NoAccount : SyncAllError()
    object CouldNotReachServer : SyncAllError()
    data class ExecuteWorkError(val error: List<app.lockbook.utils.ExecuteWorkError>) : SyncAllError()
    data class UnexpectedError(val error: String) : SyncAllError()
}

sealed class CalculateWorkError {
    object NoAccount : CalculateWorkError()
    object CouldNotReachServer : CalculateWorkError()
    data class UnexpectedError(val error: String) : CalculateWorkError()
}

sealed class ExecuteWorkError {
    object CouldNotReachServer : ExecuteWorkError()
    data class UnexpectedError(val error: String) : ExecuteWorkError()
}
