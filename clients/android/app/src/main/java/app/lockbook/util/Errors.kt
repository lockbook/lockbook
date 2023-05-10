package app.lockbook.util

import android.content.res.Resources
import androidx.annotation.StringRes
import app.lockbook.R
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

@OptIn(ExperimentalSerializationApi::class)
@JsonClassDiscriminator("tag")
@Serializable
abstract class IntermCoreResult<O, E>
        where E : Enum<E>, E : UiCoreError {
    @Serializable
    @SerialName("Ok")
    class CoreOk<O>(val content: O?) : IntermCoreResult<O, Empty>()

    @Serializable
    @SerialName("Err")
    class CoreErr<E>(val content: IntermCoreError<E>) : IntermCoreResult<Unit, E>()
            where E : Enum<E>, E : UiCoreError

    fun toResult(isNullable: Boolean = false): Result<O, CoreError<E>> {
        return when (this) {
            is CoreOk -> {
                if (isNullable) {
                    Ok(content as O)
                } else {
                    Ok(content ?: Unit as O)
                }
            }
            is CoreErr -> when (content) {
                is IntermCoreError.UiError -> {
                    Err(CoreError.UiError(content.content))
                }
                is IntermCoreError.Unexpected -> {
                    Err(CoreError.Unexpected(content.content))
                }
                else -> {
                    // impossible
                    Err(CoreError.Unexpected("Could not deserialize."))
                }
            }
            else -> {
                // impossible
                Err(CoreError.Unexpected("Could not deserialize."))
            }
        }
    }

    fun unwrapUnexpected() {
        (this as CoreErr).content as IntermCoreError.Unexpected
    }
}

@OptIn(ExperimentalSerializationApi::class)
@JsonClassDiscriminator("tag")
@Serializable
abstract class IntermCoreError<E : Enum<E>> {
    @Serializable
    @SerialName("UiError")
    class UiError<E : Enum<E>>(val content: E) : IntermCoreError<E>()

    @Serializable
    @SerialName("Unexpected")
    class Unexpected(val content: String) : IntermCoreError<Empty>()
}

sealed class CoreError<E>
where E : UiCoreError {

    class UiError<E : UiCoreError>(val content: E) : CoreError<E>()
    class Unexpected<E : UiCoreError>(val content: String) : CoreError<E>()

    fun toLbError(res: Resources): LbError = when (this) {
        is UiError -> content.toLbError(res)
        is Unexpected -> {
            LbError.newProgError(content)
        }
    }
}

interface UiCoreError {
    fun toLbError(res: Resources): LbError
}

@Serializable
enum class Empty : UiCoreError

@Serializable
enum class InitError : UiCoreError

@Serializable
enum class GetUsageError : UiCoreError {
    CouldNotReachServer,
    ClientUpdateRequired;

    override fun toLbError(res: Resources): LbError {
        return when (this) {
            CouldNotReachServer -> LbError.newUserError(getString(res, R.string.could_not_reach_server))
            ClientUpdateRequired -> LbError.newUserError(getString(res, R.string.client_update_required))
        }
    }
}
@Serializable
enum class GetStateError : UiCoreError

@Serializable
enum class CreateAccountError : UiCoreError {
    UsernameTaken,
    InvalidUsername,
    CouldNotReachServer,
    AccountExistsAlready,
    ClientUpdateRequired,
    ServerDisabled;

    override fun toLbError(res: Resources): LbError = when (this) {
        UsernameTaken -> LbError.newUserError(getString(res, R.string.username_taken))
        InvalidUsername -> LbError.newUserError(getString(res, R.string.invalid_username))
        CouldNotReachServer -> LbError.newUserError(getString(res, R.string.could_not_reach_server))
        AccountExistsAlready -> LbError.newUserError(getString(res, R.string.account_exists_already))
        ClientUpdateRequired -> LbError.newUserError(getString(res, R.string.client_update_required))
        ServerDisabled -> LbError.newUserError(getString(res, R.string.new_account_disabled))
    }
}

@Serializable
enum class ImportError : UiCoreError {
    AccountStringCorrupted,
    AccountExistsAlready,
    AccountDoesNotExist,
    UsernamePKMismatch,
    CouldNotReachServer,
    ClientUpdateRequired;

    override fun toLbError(res: Resources): LbError = when (this) {
        AccountStringCorrupted -> LbError.newUserError(getString(res, R.string.account_string_corrupted))
        AccountExistsAlready -> LbError.newUserError(getString(res, R.string.account_exists_already))
        AccountDoesNotExist -> LbError.newUserError(getString(res, R.string.account_does_not_exist))
        UsernamePKMismatch -> LbError.newUserError(getString(res, R.string.username_pk_mismatch))
        CouldNotReachServer -> LbError.newUserError(getString(res, R.string.could_not_reach_server))
        ClientUpdateRequired -> LbError.newUserError(getString(res, R.string.client_update_required))
    }
}

@Serializable
enum class AccountExportError : UiCoreError {
    NoAccount;

    override fun toLbError(res: Resources): LbError = when (this) {
        NoAccount -> LbError.newUserError(getString(res, R.string.no_account))
    }
}

@Serializable
enum class GetAccountError : UiCoreError {
    NoAccount;

    override fun toLbError(res: Resources): LbError = when (this) {
        NoAccount -> LbError.newUserError(getString(res, R.string.no_account))
    }
}

@Serializable
enum class GetRootError : UiCoreError {
    NoRoot;

    override fun toLbError(res: Resources): LbError = when (this) {
        NoRoot -> LbError.newUserError(getString(res, R.string.no_root))
    }
}

@Serializable
enum class WriteToDocumentError : UiCoreError {
    FileDoesNotExist,
    FolderTreatedAsDocument,
    InsufficientPermission;

    override fun toLbError(res: Resources): LbError = when (this) {
        FileDoesNotExist -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
        FolderTreatedAsDocument -> LbError.newUserError(getString(res, R.string.folder_treated_as_document))
        InsufficientPermission -> LbError.newUserError(getString(res, R.string.insufficient_permissions))
    }
}

@Serializable
enum class CreateFileError : UiCoreError {
    DocumentTreatedAsFolder,
    CouldNotFindAParent,
    FileNameNotAvailable,
    FileNameContainsSlash,
    FileNameEmpty,
    FileNameTooLong,
    LinkInSharedFolder,
    LinkTargetIsOwned,
    LinkTargetNonexistent,
    MultipleLinksToSameFile,
    InsufficientPermission;

    override fun toLbError(res: Resources): LbError = when (this) {
        DocumentTreatedAsFolder -> LbError.newUserError(getString(res, R.string.document_treated_as_folder))
        CouldNotFindAParent -> LbError.newUserError(getString(res, R.string.could_not_find_a_parent))
        FileNameNotAvailable -> LbError.newUserError(getString(res, R.string.file_name_not_available))
        FileNameContainsSlash -> LbError.newUserError(getString(res, R.string.file_name_contains_slash))
        FileNameEmpty -> LbError.newUserError(getString(res, R.string.file_name_empty))
        FileNameTooLong -> LbError.newUserError(getString(res, R.string.file_name_too_long))
        LinkInSharedFolder -> LbError.newUserError(getString(res, R.string.link_in_shared_folder))
        LinkTargetIsOwned -> LbError.newUserError(getString(res, R.string.link_target_is_owned))
        LinkTargetNonexistent -> LbError.newUserError(getString(res, R.string.link_target_nonexistent))
        MultipleLinksToSameFile -> LbError.newUserError(getString(res, R.string.multiple_links_to_same_file))
        InsufficientPermission -> LbError.newUserError(getString(res, R.string.insufficient_permissions))
    }
}

@Serializable
enum class GetChildrenError : UiCoreError

@Serializable
enum class GetFileByIdError : UiCoreError {
    NoFileWithThatId;

    override fun toLbError(res: Resources): LbError = when (this) {
        NoFileWithThatId -> LbError.newUserError(getString(res, R.string.no_file_with_that_id))
    }
}

@Serializable
enum class FileDeleteError : UiCoreError {
    FileDoesNotExist,
    CannotDeleteRoot,
    InsufficientPermission;

    override fun toLbError(res: Resources): LbError = when (this) {
        FileDoesNotExist -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
        CannotDeleteRoot -> LbError.newUserError(getString(res, R.string.cannot_delete_root))
        InsufficientPermission -> LbError.newUserError(getString(res, R.string.insufficient_permissions))
    }
}

@Serializable
enum class ReadDocumentError : UiCoreError {
    TreatedFolderAsDocument,
    FileDoesNotExist;

    override fun toLbError(res: Resources): LbError = when (this) {
        TreatedFolderAsDocument -> LbError.newUserError(getString(res, R.string.folder_treated_as_document))
        FileDoesNotExist -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
    }
}

@Serializable
enum class ExportDrawingToDiskError : UiCoreError {
    FolderTreatedAsDrawing,
    FileDoesNotExist,
    InvalidDrawing,
    BadPath,
    FileAlreadyExistsInDisk;

    override fun toLbError(res: Resources): LbError = when (this) {
        FolderTreatedAsDrawing -> LbError.newUserError(getString(res, R.string.folder_treated_as_drawing))
        FileDoesNotExist -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
        InvalidDrawing -> LbError.newUserError(getString(res, R.string.invalid_drawing))
        BadPath -> LbError.newUserError(getString(res, R.string.bad_path))
        FileAlreadyExistsInDisk -> LbError.newUserError(getString(res, R.string.file_already_exists_on_disk))
    }
}

@Serializable
enum class RenameFileError : UiCoreError {
    FileDoesNotExist,
    NewNameContainsSlash,
    FileNameNotAvailable,
    FileNameTooLong,
    NewNameEmpty,
    CannotRenameRoot,
    InsufficientPermission;

    override fun toLbError(res: Resources): LbError = when (this) {
        FileDoesNotExist -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
        NewNameContainsSlash -> LbError.newUserError(getString(res, R.string.file_name_contains_slash))
        FileNameNotAvailable -> LbError.newUserError(getString(res, R.string.file_name_not_available))
        FileNameTooLong -> LbError.newUserError(getString(res, R.string.file_name_too_long))
        NewNameEmpty -> LbError.newUserError(getString(res, R.string.file_name_empty))
        CannotRenameRoot -> LbError.newUserError(getString(res, R.string.cannot_rename_root))
        InsufficientPermission -> LbError.newUserError(getString(res, R.string.insufficient_permissions))
    }
}

@Serializable
enum class MoveFileError : UiCoreError {
    FileDoesNotExist,
    DocumentTreatedAsFolder,
    TargetParentDoesNotExist,
    TargetParentHasChildNamedThat,
    CannotMoveRoot,
    FolderMovedIntoItself,
    LinkInSharedFolder,
    InsufficientPermission;

    override fun toLbError(res: Resources): LbError = when (this) {
        FileDoesNotExist -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
        DocumentTreatedAsFolder -> LbError.newUserError(getString(res, R.string.document_treated_as_folder))
        TargetParentDoesNotExist -> LbError.newUserError(getString(res, R.string.could_not_find_a_parent))
        TargetParentHasChildNamedThat -> LbError.newUserError(getString(res, R.string.target_parent_has_a_child_named_that))
        CannotMoveRoot -> LbError.newUserError(getString(res, R.string.cannot_move_root))
        FolderMovedIntoItself -> LbError.newUserError(getString(res, R.string.folder_moved_into_itself))
        LinkInSharedFolder -> LbError.newUserError(getString(res, R.string.link_in_shared_folder))
        InsufficientPermission -> LbError.newUserError(getString(res, R.string.insufficient_permissions))
    }
}

@Serializable
enum class SyncAllError : UiCoreError {
    Retry,
    CouldNotReachServer,
    ClientUpdateRequired,
    UsageIsOverFreeTierDataCap;

    override fun toLbError(res: Resources): LbError = when (this) {
        Retry -> LbError.newUserError(getString(res, R.string.retry_sync))
        CouldNotReachServer -> LbError.newUserError(getString(res, R.string.could_not_reach_server))
        ClientUpdateRequired -> LbError.newUserError(getString(res, R.string.client_update_required))
        UsageIsOverFreeTierDataCap -> LbError.newUserError(getString(res,
R.string.usage_is_over_free_tier_data_cap))
    }
}

@Serializable
enum class CalculateWorkError : UiCoreError {
    CouldNotReachServer,
    ClientUpdateRequired;

    override fun toLbError(res: Resources): LbError = when (this) {
        CouldNotReachServer -> LbError.newUserError(getString(res, R.string.could_not_reach_server))
        ClientUpdateRequired -> LbError.newUserError(getString(res, R.string.client_update_required))
    }
}

@Serializable
enum class UpgradeAccountGooglePlayError : UiCoreError {
    AlreadyPremium,
    InvalidPurchaseToken,
    ExistingRequestPending,
    CouldNotReachServer,
    ClientUpdateRequired;

    override fun toLbError(res: Resources): LbError = when (this) {
        AlreadyPremium -> LbError.newUserError(getString(res, R.string.already_premium))
        InvalidPurchaseToken -> LbError.newUserError(getString(res, R.string.invalid_purchase_token))
        ExistingRequestPending -> LbError.newUserError(getString(res, R.string.existing_request_pending))
        CouldNotReachServer -> LbError.newUserError(getString(res, R.string.could_not_reach_server))
        ClientUpdateRequired -> LbError.newUserError(getString(res, R.string.client_update_required))
    }
}

@Serializable
enum class CancelSubscriptionError : UiCoreError {
    NotPremium,
    AlreadyCanceled,
    UsageIsOverFreeTierDataCap,
    ExistingRequestPending,
    CouldNotReachServer,
    ClientUpdateRequired,
    CannotCancelForAppStore;

    override fun toLbError(res: Resources): LbError = when (this) {
        NotPremium -> LbError.newUserError(getString(res, R.string.not_premium))
        AlreadyCanceled -> LbError.newUserError(getString(res, R.string.already_canceled))
        UsageIsOverFreeTierDataCap -> LbError.newUserError(getString(res, R.string.usage_is_over_free_tier_data_cap))
        ExistingRequestPending -> LbError.newUserError(getString(res, R.string.existing_request_pending))
        CouldNotReachServer -> LbError.newUserError(getString(res, R.string.could_not_reach_server))
        ClientUpdateRequired -> LbError.newUserError(getString(res, R.string.client_update_required))
        CannotCancelForAppStore -> LbError.newUserError(getString(res, R.string.cannot_cancel_for_app_store))
    }
}

@Serializable
enum class GetSubscriptionInfoError : UiCoreError {
    CouldNotReachServer,
    ClientUpdateRequired;

    override fun toLbError(res: Resources): LbError = when (this) {
        CouldNotReachServer -> LbError.newUserError(getString(res, R.string.could_not_reach_server))
        ClientUpdateRequired -> LbError.newUserError(getString(res, R.string.client_update_required))
    }
}

@Serializable
enum class ExportFileError : UiCoreError {
    ParentDoesNotExist,
    DiskPathTaken,
    DiskPathInvalid;

    override fun toLbError(res: Resources): LbError = when (this) {
        ParentDoesNotExist -> LbError.newUserError(getString(res, R.string.could_not_find_a_parent))
        // Used basic errors since specific errors are not useful to the user
        DiskPathTaken -> LbError.newUserError(getString(res, R.string.basic_error))
        DiskPathInvalid -> LbError.newUserError(getString(res, R.string.basic_error))
    }
}

@Serializable
enum class ShareFileError : UiCoreError {
    CannotShareRoot,
    FileNonexistent,
    ShareAlreadyExists,
    LinkInSharedFolder,
    InsufficientPermission;

    override fun toLbError(res: Resources): LbError = when (this) {
        CannotShareRoot -> LbError.newUserError(getString(res, R.string.cannot_share_root))
        FileNonexistent -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
        ShareAlreadyExists -> LbError.newUserError(getString(res, R.string.share_already_exists))
        LinkInSharedFolder -> LbError.newUserError(getString(res, R.string.link_in_shared_folder))
        InsufficientPermission -> LbError.newUserError(getString(res, R.string.insufficient_permissions))
    }
}

@Serializable
enum class DeletePendingShareError : UiCoreError {
    FileNonexistent,
    ShareNonexistent;

    override fun toLbError(res: Resources): LbError = when (this) {
        FileNonexistent -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
        ShareNonexistent -> LbError.newUserError(getString(res, R.string.share_non_existant))
    }
}

@Serializable
enum class GetDrawingError : UiCoreError {
    FolderTreatedAsDrawing,
    InvalidDrawing,
    FileDoesNotExist;

    override fun toLbError(res: Resources): LbError = when (this) {
        FolderTreatedAsDrawing -> LbError.newUserError(getString(res, R.string.folder_treated_as_drawing))
        InvalidDrawing -> LbError.newUserError(getString(res, R.string.invalid_drawing))
        FileDoesNotExist -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
    }
}

@Serializable
enum class SaveDrawingError : UiCoreError {
    FileDoesNotExist,
    FolderTreatedAsDrawing,
    InvalidDrawing;

    override fun toLbError(res: Resources): LbError = when (this) {
        FileDoesNotExist -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
        FolderTreatedAsDrawing -> LbError.newUserError(getString(res, R.string.folder_treated_as_drawing))
        InvalidDrawing -> LbError.newUserError(getString(res, R.string.invalid_drawing))
    }
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

fun getString(
    res: Resources,
    @StringRes stringRes: Int,
    vararg formatArgs: Any = emptyArray()
): String = res.getString(stringRes, *formatArgs)

fun basicErrorString(res: Resources): String = getString(res, R.string.basic_error)
