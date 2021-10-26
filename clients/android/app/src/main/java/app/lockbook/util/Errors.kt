package app.lockbook.util

import android.content.res.Resources
import androidx.annotation.StringRes
import app.lockbook.R

sealed class CoreError {
    fun toLbError(res: Resources): LbError = when (this) {
        GetUsageError.NoAccount,
        GetAccountError.NoAccount,
        AccountExportError.NoAccount,
        CreateFileError.NoAccount,
        WriteToDocumentError.NoAccount,
        ReadDocumentError.NoAccount,
        SaveDocumentToDiskError.NoAccount,
        ExportDrawingToDiskError.NoAccount,
        ExportDrawingError.NoAccount,
        MoveFileError.NoAccount,
        CalculateWorkError.NoAccount,
        SyncAllError.NoAccount -> LbError.newUserError(getString(res, R.string.no_account))
        GetUsageError.ClientUpdateRequired,
        CreateAccountError.ClientUpdateRequired,
        ImportError.ClientUpdateRequired,
        CalculateWorkError.ClientUpdateRequired,
        SyncAllError.ClientUpdateRequired -> LbError.newUserError(getString(res, R.string.client_update_required))
        GetUsageError.CouldNotReachServer,
        CreateAccountError.CouldNotReachServer,
        ImportError.CouldNotReachServer,
        SyncAllError.CouldNotReachServer,
        CalculateWorkError.CouldNotReachServer -> LbError.newUserError(getString(res, R.string.could_not_reach_server))
        MigrationError.StateRequiresCleaning -> LbError.newUserError(getString(res, R.string.state_requires_cleaning))
        CreateAccountError.AccountExistsAlready,
        ImportError.AccountExistsAlready -> LbError.newUserError(getString(res, R.string.account_exists_already))
        CreateAccountError.InvalidUsername -> LbError.newUserError(getString(res, R.string.invalid_username))
        CreateAccountError.UsernameTaken -> LbError.newUserError(getString(res, R.string.username_taken))
        ImportError.AccountDoesNotExist -> LbError.newUserError(getString(res, R.string.account_does_not_exist))
        ImportError.AccountStringCorrupted -> LbError.newUserError(getString(res, R.string.account_string_corrupted))
        ImportError.UsernamePKMismatch -> LbError.newUserError(getString(res, R.string.username_pk_mismatch))
        GetRootError.NoRoot -> LbError.newUserError(getString(res, R.string.no_root))
        WriteToDocumentError.FileDoesNotExist,
        FileDeleteError.FileDoesNotExist,
        ReadDocumentError.FileDoesNotExist,
        SaveDocumentToDiskError.FileDoesNotExist,
        ExportDrawingError.FileDoesNotExist,
        ExportDrawingToDiskError.FileDoesNotExist,
        RenameFileError.FileDoesNotExist,
        MoveFileError.FileDoesNotExist -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
        WriteToDocumentError.FolderTreatedAsDocument,
        ReadDocumentError.TreatedFolderAsDocument,
        SaveDocumentToDiskError.TreatedFolderAsDocument -> LbError.newUserError(getString(res, R.string.folder_treated_as_document))
        CreateFileError.CouldNotFindAParent,
        MoveFileError.TargetParentDoesNotExist -> LbError.newUserError(getString(res, R.string.could_not_find_a_parent))
        CreateFileError.DocumentTreatedAsFolder,
        MoveFileError.DocumentTreatedAsFolder -> LbError.newUserError(getString(res, R.string.document_treated_as_folder))
        CreateFileError.FileNameContainsSlash,
        RenameFileError.NewNameContainsSlash -> LbError.newUserError(getString(res, R.string.file_name_contains_slash))
        CreateFileError.FileNameEmpty,
        RenameFileError.NewNameEmpty -> LbError.newUserError(getString(res, R.string.file_name_empty))
        CreateFileError.FileNameNotAvailable,
        RenameFileError.FileNameNotAvailable -> LbError.newUserError(getString(res, R.string.file_name_not_available))
        GetFileByIdError.NoFileWithThatId -> LbError.newUserError(getString(res, R.string.no_file_with_that_id))
        FileDeleteError.CannotDeleteRoot -> LbError.newUserError(getString(res, R.string.cannot_delete_root))
        SaveDocumentToDiskError.BadPath,
        ExportDrawingToDiskError.BadPath -> LbError.newUserError(getString(res, R.string.bad_path))
        SaveDocumentToDiskError.FileAlreadyExistsInDisk,
        ExportDrawingToDiskError.FileAlreadyExistsInDisk -> LbError.newUserError(getString(res, R.string.file_already_exists_on_disk))
        ExportDrawingError.FolderTreatedAsDrawing,
        ExportDrawingToDiskError.FolderTreatedAsDrawing -> LbError.newUserError(getString(res, R.string.folder_treated_as_drawing))
        ExportDrawingError.InvalidDrawing,
        ExportDrawingToDiskError.InvalidDrawing -> LbError.newUserError(getString(res, R.string.invalid_drawing))
        RenameFileError.CannotRenameRoot -> LbError.newUserError(getString(res, R.string.cannot_rename_root))
        MoveFileError.CannotMoveRoot -> LbError.newUserError(getString(res, R.string.cannot_move_root))
        MoveFileError.FolderMovedIntoItself -> LbError.newUserError(getString(res, R.string.folder_moved_into_itself))
        MoveFileError.TargetParentHasChildNamedThat -> LbError.newUserError(getString(res, R.string.target_parent_has_a_child_named_that))
        is CalculateWorkError.Unexpected -> LbError.newProgError(this.error)
        is SyncAllError.Unexpected -> LbError.newProgError(this.error)
        is MoveFileError.Unexpected -> LbError.newProgError(this.error)
        is RenameFileError.Unexpected -> LbError.newProgError(this.error)
        is ExportDrawingToDiskError.Unexpected -> LbError.newProgError(this.error)
        is ExportDrawingError.Unexpected -> LbError.newProgError(this.error)
        is SaveDocumentToDiskError.Unexpected -> LbError.newProgError(this.error)
        is FileDeleteError.Unexpected -> LbError.newProgError(this.error)
        is GetFileByIdError.Unexpected -> LbError.newProgError(this.error)
        is ReadDocumentError.Unexpected -> LbError.newProgError(this.error)
        is GetRootError.Unexpected -> LbError.newProgError(this.error)
        is WriteToDocumentError.Unexpected -> LbError.newProgError(this.error)
        is GetAccountError.Unexpected -> LbError.newProgError(this.error)
        is SetLastSyncedError.Unexpected -> LbError.newProgError(this.error)
        is AccountExportError.Unexpected -> LbError.newProgError(this.error)
        is ImportError.Unexpected -> LbError.newProgError(this.error)
        is CreateAccountError.Unexpected -> LbError.newProgError(this.error)
        is MigrationError.Unexpected -> LbError.newProgError(this.error)
        is InitLoggerError.Unexpected -> LbError.newProgError(this.error)
        is GetStateError.Unexpected -> LbError.newProgError(this.error)
        is GetUsageError.Unexpected -> LbError.newProgError(this.error)
        is CreateFileError.Unexpected -> LbError.newProgError(this.error)
        is GetChildrenError.Unexpected -> LbError.newProgError(this.error)
    }
}

sealed class InitLoggerError : CoreError() {
    data class Unexpected(val error: String) : InitLoggerError()
}

sealed class GetUsageError : CoreError() {
    object NoAccount : GetUsageError()
    object CouldNotReachServer : GetUsageError()
    object ClientUpdateRequired : GetUsageError()
    data class Unexpected(val error: String) : GetUsageError()
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

sealed class SaveDocumentToDiskError : CoreError() {
    object TreatedFolderAsDocument : SaveDocumentToDiskError()
    object NoAccount : SaveDocumentToDiskError()
    object FileDoesNotExist : SaveDocumentToDiskError()
    object BadPath : SaveDocumentToDiskError()
    object FileAlreadyExistsInDisk : SaveDocumentToDiskError()
    data class Unexpected(val error: String) : SaveDocumentToDiskError()
}

sealed class ExportDrawingError : CoreError() {
    object FolderTreatedAsDrawing : ExportDrawingError()
    object FileDoesNotExist : ExportDrawingError()
    object NoAccount : ExportDrawingError()
    object InvalidDrawing : ExportDrawingError()
    data class Unexpected(val error: String) : ExportDrawingError()
}

sealed class ExportDrawingToDiskError : CoreError() {
    object FolderTreatedAsDrawing : ExportDrawingToDiskError()
    object FileDoesNotExist : ExportDrawingToDiskError()
    object NoAccount : ExportDrawingToDiskError()
    object InvalidDrawing : ExportDrawingToDiskError()
    object BadPath : ExportDrawingToDiskError()
    object FileAlreadyExistsInDisk : ExportDrawingToDiskError()
    data class Unexpected(val error: String) : ExportDrawingToDiskError()
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
    object ClientUpdateRequired : SyncAllError()
    data class Unexpected(val error: String) : SyncAllError()
}

sealed class CalculateWorkError : CoreError() {
    object NoAccount : CalculateWorkError()
    object CouldNotReachServer : CalculateWorkError()
    object ClientUpdateRequired : CalculateWorkError()
    data class Unexpected(val error: String) : CalculateWorkError()
}

val <T> T.exhaustive: T
    get() = this

data class LbError(val kind: LbErrorKind, val msg: String) {
    companion object {
        fun newProgError(msg: String) = LbError(LbErrorKind.Program, msg)
        fun newUserError(msg: String) = LbError(LbErrorKind.User, msg)
        fun basicError(res: Resources) = LbError(LbErrorKind.Program, basicErrorString(res))
    }
}

enum class LbErrorKind {
    Program,
    User,
}

fun getString(res: Resources, @StringRes stringRes: Int, vararg formatArgs: Any = emptyArray()): String = res.getString(stringRes, *formatArgs)
fun basicErrorString(res: Resources): String = getString(res, R.string.basic_error)
