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
sealed class IntermCoreResult<O, out E : UiCoreError> {
    @Serializable
    @SerialName("Ok")
    class Ok<O>(val content: O? = null) : IntermCoreResult<O, UiCoreError>()

    @Serializable
    @SerialName("Err")
    class Err<out E : UiCoreError>(val content: CoreError<E>) : IntermCoreResult<Unit, E>()

    fun toResult(): com.github.michaelbull.result.Result<O, CoreError<E>> {
        return when (this) {
            is Ok -> {
                if (content != null) {
                    com.github.michaelbull.result.Ok(content)
                } else {
                    com.github.michaelbull.result.Ok(Unit as O)
                }
            }
            is Err -> com.github.michaelbull.result.Err(content)
        }
    }

    fun unwrapUnexpected() {
        (this as Err).content as CoreError.Unexpected
    }
}

@OptIn(ExperimentalSerializationApi::class)
@JsonClassDiscriminator("tag")
@Serializable
sealed class CoreError<out E : UiCoreError> {
    @Serializable
    @SerialName("UiError")
    class UiError<out E : UiCoreError>(val content: E) : CoreError<E>()

    @Serializable
    @SerialName("Unexpected")
    class Unexpected<out E : UiCoreError>(val content: String) : CoreError<E>()

    fun toLbError(res: Resources): LbError = when (this) {
        is UiError -> content.toLbError(res)
        is Unexpected -> {
            LbError.newProgError(content)
        }
    }
}

@Serializable
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
        SyncAllError.ClientUpdateRequired -> LbError.newUserError(
            getString(
                res,
                R.string.client_update_required
            )
        )
        GetUsageError.CouldNotReachServer,
        CreateAccountError.CouldNotReachServer,
        ImportError.CouldNotReachServer,
        SyncAllError.CouldNotReachServer,
        CalculateWorkError.CouldNotReachServer -> LbError.newUserError(
            getString(
                res,
                R.string.could_not_reach_server
            )
        )
        MigrationError.StateRequiresCleaning -> LbError.newUserError(
            getString(
                res,
                R.string.state_requires_cleaning
            )
        )
        CreateAccountError.AccountExistsAlready,
        ImportError.AccountExistsAlready -> LbError.newUserError(
            getString(
                res,
                R.string.account_exists_already
            )
        )
        CreateAccountError.InvalidUsername -> LbError.newUserError(
            getString(
                res,
                R.string.invalid_username
            )
        )
        CreateAccountError.UsernameTaken -> LbError.newUserError(
            getString(
                res,
                R.string.username_taken
            )
        )
        ImportError.AccountDoesNotExist -> LbError.newUserError(
            getString(
                res,
                R.string.account_does_not_exist
            )
        )
        ImportError.AccountStringCorrupted -> LbError.newUserError(
            getString(
                res,
                R.string.account_string_corrupted
            )
        )
        ImportError.UsernamePKMismatch -> LbError.newUserError(
            getString(
                res,
                R.string.username_pk_mismatch
            )
        )
        GetRootError.NoRoot -> LbError.newUserError(getString(res, R.string.no_root))
        WriteToDocumentError.FileDoesNotExist,
        FileDeleteError.FileDoesNotExist,
        ReadDocumentError.FileDoesNotExist,
        SaveDocumentToDiskError.FileDoesNotExist,
        ExportDrawingError.FileDoesNotExist,
        ExportDrawingToDiskError.FileDoesNotExist,
        RenameFileError.FileDoesNotExist,
        MoveFileError.FileDoesNotExist -> LbError.newUserError(
            getString(
                res,
                R.string.file_does_not_exist
            )
        )
        WriteToDocumentError.FolderTreatedAsDocument,
        ReadDocumentError.TreatedFolderAsDocument,
        SaveDocumentToDiskError.TreatedFolderAsDocument -> LbError.newUserError(
            getString(
                res,
                R.string.folder_treated_as_document
            )
        )
        CreateFileError.CouldNotFindAParent,
        MoveFileError.TargetParentDoesNotExist -> LbError.newUserError(
            getString(
                res,
                R.string.could_not_find_a_parent
            )
        )
        CreateFileError.DocumentTreatedAsFolder,
        MoveFileError.DocumentTreatedAsFolder -> LbError.newUserError(
            getString(
                res,
                R.string.document_treated_as_folder
            )
        )
        CreateFileError.FileNameContainsSlash,
        RenameFileError.NewNameContainsSlash -> LbError.newUserError(
            getString(
                res,
                R.string.file_name_contains_slash
            )
        )
        CreateFileError.FileNameEmpty,
        RenameFileError.NewNameEmpty -> LbError.newUserError(
            getString(
                res,
                R.string.file_name_empty
            )
        )
        CreateFileError.FileNameNotAvailable,
        RenameFileError.FileNameNotAvailable -> LbError.newUserError(
            getString(
                res,
                R.string.file_name_not_available
            )
        )
        GetFileByIdError.NoFileWithThatId -> LbError.newUserError(
            getString(
                res,
                R.string.no_file_with_that_id
            )
        )
        FileDeleteError.CannotDeleteRoot -> LbError.newUserError(
            getString(
                res,
                R.string.cannot_delete_root
            )
        )
        SaveDocumentToDiskError.BadPath,
        ExportDrawingToDiskError.BadPath -> LbError.newUserError(getString(res, R.string.bad_path))
        SaveDocumentToDiskError.FileAlreadyExistsInDisk,
        ExportDrawingToDiskError.FileAlreadyExistsInDisk -> LbError.newUserError(
            getString(
                res,
                R.string.file_already_exists_on_disk
            )
        )
        ExportDrawingError.FolderTreatedAsDrawing,
        ExportDrawingToDiskError.FolderTreatedAsDrawing -> LbError.newUserError(
            getString(
                res,
                R.string.folder_treated_as_drawing
            )
        )
        ExportDrawingError.InvalidDrawing,
        ExportDrawingToDiskError.InvalidDrawing -> LbError.newUserError(
            getString(
                res,
                R.string.invalid_drawing
            )
        )
        RenameFileError.CannotRenameRoot -> LbError.newUserError(
            getString(
                res,
                R.string.cannot_rename_root
            )
        )
        MoveFileError.CannotMoveRoot -> LbError.newUserError(
            getString(
                res,
                R.string.cannot_move_root
            )
        )
        MoveFileError.FolderMovedIntoItself -> LbError.newUserError(
            getString(
                res,
                R.string.folder_moved_into_itself
            )
        )
        MoveFileError.TargetParentHasChildNamedThat -> LbError.newUserError(
            getString(
                res,
                R.string.target_parent_has_a_child_named_that
            )
        )
        CreateAccountError.ServerDisabled -> LbError.newUserError(
            getString(
                res,
                R.string.new_account_disabled
            )
        )
    }
}

@Serializable
sealed class InitLoggerError : UiCoreError()

@Serializable
sealed class GetUsageError : UiCoreError() {
    @Serializable
    object NoAccount : GetUsageError()

    @Serializable
    object CouldNotReachServer : GetUsageError()

    @Serializable
    object ClientUpdateRequired : GetUsageError()
}

@Serializable
sealed class GetStateError : UiCoreError()

@Serializable
sealed class MigrationError : UiCoreError() {
    @Serializable
    object StateRequiresCleaning : MigrationError()
}

@Serializable
sealed class CreateAccountError : UiCoreError() {
    @Serializable
    object UsernameTaken : CreateAccountError()

    @Serializable
    object InvalidUsername : CreateAccountError()

    @Serializable
    object CouldNotReachServer : CreateAccountError()

    @Serializable
    object AccountExistsAlready : CreateAccountError()

    @Serializable
    object ClientUpdateRequired : CreateAccountError()

    @Serializable
    object ServerDisabled : CreateAccountError()
}

@Serializable
sealed class ImportError : UiCoreError() {
    @Serializable
    object AccountStringCorrupted : ImportError()

    @Serializable
    object AccountExistsAlready : ImportError()

    @Serializable
    object AccountDoesNotExist : ImportError()

    @Serializable
    object UsernamePKMismatch : ImportError()

    @Serializable
    object CouldNotReachServer : ImportError()

    @Serializable
    object ClientUpdateRequired : ImportError()
}

sealed class AccountExportError : UiCoreError() {
    @Serializable
    object NoAccount : AccountExportError()
}

@Serializable
sealed class GetAccountError : UiCoreError() {
    @Serializable
    object NoAccount : GetAccountError()
}

@Serializable
sealed class GetRootError : UiCoreError() {
    @Serializable
    object NoRoot : GetRootError()
}

@Serializable
sealed class WriteToDocumentError : UiCoreError() {
    @Serializable
    object NoAccount : WriteToDocumentError()

    @Serializable
    object FileDoesNotExist : WriteToDocumentError()

    @Serializable
    object FolderTreatedAsDocument : WriteToDocumentError()
}

@Serializable
sealed class CreateFileError : UiCoreError() {
    @Serializable
    object NoAccount : CreateFileError()

    @Serializable
    object DocumentTreatedAsFolder : CreateFileError()

    @Serializable
    object CouldNotFindAParent : CreateFileError()

    @Serializable
    object FileNameNotAvailable : CreateFileError()

    @Serializable
    object FileNameContainsSlash : CreateFileError()

    @Serializable
    object FileNameEmpty : CreateFileError()
}

@Serializable
sealed class GetChildrenError : UiCoreError() {
}

@Serializable
sealed class GetFileByIdError : UiCoreError() {
    @Serializable
    object NoFileWithThatId : GetFileByIdError()
}

@Serializable
sealed class FileDeleteError : UiCoreError() {
    @Serializable
    object FileDoesNotExist : FileDeleteError()

    @Serializable
    object CannotDeleteRoot : FileDeleteError()
}

@Serializable
sealed class ReadDocumentError : UiCoreError() {
    @Serializable
    object TreatedFolderAsDocument : ReadDocumentError()

    @Serializable
    object NoAccount : ReadDocumentError()

    @Serializable
    object FileDoesNotExist : ReadDocumentError()
}

@Serializable
sealed class SaveDocumentToDiskError : UiCoreError() {
    @Serializable
    object TreatedFolderAsDocument : SaveDocumentToDiskError()

    @Serializable
    object NoAccount : SaveDocumentToDiskError()

    @Serializable
    object FileDoesNotExist : SaveDocumentToDiskError()

    @Serializable
    object BadPath : SaveDocumentToDiskError()

    @Serializable
    object FileAlreadyExistsInDisk : SaveDocumentToDiskError()
}

@Serializable
sealed class ExportDrawingError : UiCoreError() {
    @Serializable
    object FolderTreatedAsDrawing : ExportDrawingError()

    @Serializable
    object FileDoesNotExist : ExportDrawingError()

    @Serializable
    object NoAccount : ExportDrawingError()

    @Serializable
    object InvalidDrawing : ExportDrawingError()
}

@Serializable
sealed class ExportDrawingToDiskError : UiCoreError() {
    @Serializable
    object FolderTreatedAsDrawing : ExportDrawingToDiskError()

    @Serializable
    object FileDoesNotExist : ExportDrawingToDiskError()

    @Serializable
    object NoAccount : ExportDrawingToDiskError()

    @Serializable
    object InvalidDrawing : ExportDrawingToDiskError()

    @Serializable
    object BadPath : ExportDrawingToDiskError()

    @Serializable
    object FileAlreadyExistsInDisk : ExportDrawingToDiskError()
}

@Serializable
sealed class RenameFileError : UiCoreError() {
    @Serializable
    object FileDoesNotExist : RenameFileError()

    @Serializable
    object NewNameContainsSlash : RenameFileError()

    @Serializable
    object FileNameNotAvailable : RenameFileError()

    @Serializable
    object NewNameEmpty : RenameFileError()

    @Serializable
    object CannotRenameRoot : RenameFileError()
}

@Serializable
sealed class MoveFileError : UiCoreError() {
    @Serializable
    object NoAccount : MoveFileError()

    @Serializable
    object FileDoesNotExist : MoveFileError()

    @Serializable
    object DocumentTreatedAsFolder : MoveFileError()

    @Serializable
    object TargetParentDoesNotExist : MoveFileError()

    @Serializable
    object TargetParentHasChildNamedThat : MoveFileError()

    @Serializable
    object CannotMoveRoot : MoveFileError()

    @Serializable
    object FolderMovedIntoItself : MoveFileError()
}

@Serializable
sealed class SyncAllError : UiCoreError() {
    @Serializable
    object NoAccount : SyncAllError()

    @Serializable
    object CouldNotReachServer : SyncAllError()

    @Serializable
    object ClientUpdateRequired : SyncAllError()
}

@Serializable
sealed class CalculateWorkError : UiCoreError() {
    @Serializable
    object NoAccount : CalculateWorkError()

    @Serializable
    object CouldNotReachServer : CalculateWorkError()

    @Serializable
    object ClientUpdateRequired : CalculateWorkError()
}

val <T> T.exhaustive: T
    get() = this

data class LbError(val kind: LbErrorKind, val msg: String) {
    companion @Serializable
    object {
        fun newProgError(msg: String) = LbError(LbErrorKind.Program, msg)
        fun newUserError(msg: String) = LbError(LbErrorKind.User, msg)
        fun basicError(res: Resources) = LbError(LbErrorKind.Program, basicErrorString(res))
    }
}

enum class LbErrorKind {
    Program,
    User,
}

fun getString(
    res: Resources,
    @StringRes stringRes: Int,
    vararg formatArgs: Any = emptyArray()
): String = res.getString(stringRes, *formatArgs)

fun basicErrorString(res: Resources): String = getString(res, R.string.basic_error)
