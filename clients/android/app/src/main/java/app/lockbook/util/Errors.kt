package app.lockbook.util

import android.content.res.Resources
import androidx.annotation.StringRes
import app.lockbook.R
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

@OptIn(ExperimentalSerializationApi::class)
@JsonClassDiscriminator("tag")
@Serializable
sealed class CoreError<out E: UiCoreError> {
    @Serializable
    @SerialName("UiError")
    class UiError<out E: UiCoreError>(val content: E) : CoreError<E>()

    @Serializable
    @SerialName("Unexpected")
    class Unexpected(val content: String) : CoreError<Nothing>()

    fun toLbError(res: Resources): LbError = when (this) {
        is UiError -> content.toLbError(res)
        is Unexpected -> {
            LbError.newProgError(content)
        }
    }
}

@OptIn(ExperimentalSerializationApi::class)
@JsonClassDiscriminator("tag")
@Serializable
sealed class IntermCoreResult<O, out E: UiCoreError> {
    @Serializable
    @SerialName("Ok")
    class Ok<O>(val content: O) : IntermCoreResult<O, Nothing>()

    @Serializable
    @SerialName("Err")
    class Err<out E: UiCoreError>(val content: CoreError<E>) : IntermCoreResult<Unit, E>()

    fun toResult(): com.github.michaelbull.result.Result<O, CoreError<E>> {
        return when (this) {
            is Ok -> {
                com.github.michaelbull.result.Ok(content)
            }
            is Err -> com.github.michaelbull.result.Err(content)
        }
    }
}

sealed class UiCoreError {
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
        CreateAccountError.ServerDisabled -> LbError.newUserError(getString(res, R.string.new_account_disabled))
    }
}

@kotlinx.serialization.Serializable
sealed class InitLoggerError: UiCoreError()

@kotlinx.serialization.Serializable
sealed class GetUsageError: UiCoreError() {
    object NoAccount : GetUsageError()
    object CouldNotReachServer : GetUsageError()
    object ClientUpdateRequired : GetUsageError()
}

@kotlinx.serialization.Serializable
sealed class GetStateError

@kotlinx.serialization.Serializable
sealed class MigrationError: UiCoreError() {
    object StateRequiresCleaning : MigrationError()
}

@kotlinx.serialization.Serializable
sealed class CreateAccountError: UiCoreError() {
    object UsernameTaken : CreateAccountError()
    object InvalidUsername : CreateAccountError()
    object CouldNotReachServer : CreateAccountError()
    object AccountExistsAlready : CreateAccountError()
    object ClientUpdateRequired : CreateAccountError()
    object ServerDisabled : CreateAccountError()
}

@kotlinx.serialization.Serializable
sealed class ImportError: UiCoreError() {
    object AccountStringCorrupted : ImportError()
    object AccountExistsAlready : ImportError()
    object AccountDoesNotExist : ImportError()
    object UsernamePKMismatch : ImportError()
    object CouldNotReachServer : ImportError()
    object ClientUpdateRequired : ImportError()
}

sealed class AccountExportError: UiCoreError() {
    object NoAccount : AccountExportError()
}

@kotlinx.serialization.Serializable
sealed class GetAccountError: UiCoreError() {
    object NoAccount : GetAccountError()
}

@kotlinx.serialization.Serializable
sealed class GetRootError: UiCoreError() {
    object NoRoot : GetRootError()
}

@kotlinx.serialization.Serializable
sealed class WriteToDocumentError: UiCoreError() {
    object NoAccount : WriteToDocumentError()
    object FileDoesNotExist : WriteToDocumentError()
    object FolderTreatedAsDocument : WriteToDocumentError()
}

@kotlinx.serialization.Serializable
sealed class CreateFileError: UiCoreError() {
    object NoAccount : CreateFileError()
    object DocumentTreatedAsFolder : CreateFileError()
    object CouldNotFindAParent : CreateFileError()
    object FileNameNotAvailable : CreateFileError()
    object FileNameContainsSlash : CreateFileError()
    object FileNameEmpty : CreateFileError()
}

@kotlinx.serialization.Serializable
sealed class GetChildrenError: UiCoreError() {
}

@kotlinx.serialization.Serializable
sealed class GetFileByIdError: UiCoreError() {
    object NoFileWithThatId : GetFileByIdError()
}

@kotlinx.serialization.Serializable
sealed class FileDeleteError: UiCoreError() {
    object FileDoesNotExist : FileDeleteError()
    object CannotDeleteRoot : FileDeleteError()
}

@kotlinx.serialization.Serializable
sealed class ReadDocumentError: UiCoreError() {
    object TreatedFolderAsDocument : ReadDocumentError()
    object NoAccount : ReadDocumentError()
    object FileDoesNotExist : ReadDocumentError()
}

@kotlinx.serialization.Serializable
sealed class SaveDocumentToDiskError: UiCoreError() {
    object TreatedFolderAsDocument : SaveDocumentToDiskError()
    object NoAccount : SaveDocumentToDiskError()
    object FileDoesNotExist : SaveDocumentToDiskError()
    object BadPath : SaveDocumentToDiskError()
    object FileAlreadyExistsInDisk : SaveDocumentToDiskError()
}

@kotlinx.serialization.Serializable
sealed class ExportDrawingError: UiCoreError() {
    object FolderTreatedAsDrawing : ExportDrawingError()
    object FileDoesNotExist : ExportDrawingError()
    object NoAccount : ExportDrawingError()
    object InvalidDrawing : ExportDrawingError()
}

@kotlinx.serialization.Serializable
sealed class ExportDrawingToDiskError: UiCoreError() {
    object FolderTreatedAsDrawing : ExportDrawingToDiskError()
    object FileDoesNotExist : ExportDrawingToDiskError()
    object NoAccount : ExportDrawingToDiskError()
    object InvalidDrawing : ExportDrawingToDiskError()
    object BadPath : ExportDrawingToDiskError()
    object FileAlreadyExistsInDisk : ExportDrawingToDiskError()
}

@kotlinx.serialization.Serializable
sealed class RenameFileError: UiCoreError() {
    object FileDoesNotExist : RenameFileError()
    object NewNameContainsSlash : RenameFileError()
    object FileNameNotAvailable : RenameFileError()
    object NewNameEmpty : RenameFileError()
    object CannotRenameRoot : RenameFileError()
}

@kotlinx.serialization.Serializable
sealed class MoveFileError: UiCoreError() {
    object NoAccount : MoveFileError()
    object FileDoesNotExist : MoveFileError()
    object DocumentTreatedAsFolder : MoveFileError()
    object TargetParentDoesNotExist : MoveFileError()
    object TargetParentHasChildNamedThat : MoveFileError()
    object CannotMoveRoot : MoveFileError()
    object FolderMovedIntoItself : MoveFileError()
}

@kotlinx.serialization.Serializable
sealed class SyncAllError: UiCoreError() {
    object NoAccount : SyncAllError()
    object CouldNotReachServer : SyncAllError()
    object ClientUpdateRequired : SyncAllError()
}

@kotlinx.serialization.Serializable
sealed class CalculateWorkError: UiCoreError() {
    object NoAccount : CalculateWorkError()
    object CouldNotReachServer : CalculateWorkError()
    object ClientUpdateRequired : CalculateWorkError()
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
