package app.lockbook.util

import android.content.res.Resources
import androidx.annotation.StringRes
import app.lockbook.R
import kotlinx.serialization.*
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.buildClassSerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.json.*

@OptIn(ExperimentalSerializationApi::class)
@JsonClassDiscriminator("tag")
@Serializable
sealed class IntermCoreResult<O, out E : UiCoreError> {
    @Serializable
    @SerialName("Ok")
    class Ok<O>(val content: O) : IntermCoreResult<O, UiCoreError>()

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

interface UiCoreError {
    fun toLbError(res: Resources): LbError
}
@Serializable
enum class InitLoggerError : UiCoreError
@Serializable
enum class GetUsageError : UiCoreError {
    NoAccount,
    CouldNotReachServer,
    ClientUpdateRequired;

    override fun toLbError(res: Resources): LbError {
        return when (this) {
            NoAccount -> LbError.newUserError(getString(res, R.string.no_account))
            CouldNotReachServer -> LbError.newUserError(getString(res, R.string.could_not_reach_server))
            ClientUpdateRequired -> LbError.newUserError(getString(res, R.string.client_update_required))
        }
    }
}
@Serializable
enum class GetStateError : UiCoreError
@Serializable
enum class MigrationError : UiCoreError {
    StateRequiresCleaning;

    override fun toLbError(res: Resources): LbError = when(this) {
        StateRequiresCleaning -> LbError.newUserError(getString(res, R.string.state_requires_cleaning))
    }
}
@Serializable
enum class CreateAccountError : UiCoreError {
    UsernameTaken,
    InvalidUsername,
    CouldNotReachServer,
    AccountExistsAlready,
    ClientUpdateRequired,
    ServerDisabled;

    override fun toLbError(res: Resources): LbError = when(this) {
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

    override fun toLbError(res: Resources): LbError = when(this) {
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

    override fun toLbError(res: Resources): LbError = when(this) {
        NoAccount -> LbError.newUserError(getString(res, R.string.no_account))
    }
}

@Serializable
enum class GetAccountError : UiCoreError {
    NoAccount;

    override fun toLbError(res: Resources): LbError = when(this) {
        NoAccount -> LbError.newUserError(getString(res, R.string.no_account))
    }
}
@Serializable
enum class GetRootError : UiCoreError {
    NoRoot;

    override fun toLbError(res: Resources): LbError = when(this) {
        NoRoot -> LbError.newUserError(getString(res, R.string.no_root))
    }
}
@Serializable
enum class WriteToDocumentError : UiCoreError {
    NoAccount,
    FileDoesNotExist,
    FolderTreatedAsDocument;

    override fun toLbError(res: Resources): LbError = when(this) {
        NoAccount -> LbError.newUserError(getString(res, R.string.no_account))
        FileDoesNotExist -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
        FolderTreatedAsDocument -> LbError.newUserError(getString(res, R.string.folder_treated_as_document))
    }
}
@Serializable
enum class CreateFileError : UiCoreError {
    NoAccount,
    DocumentTreatedAsFolder,
    CouldNotFindAParent,
    FileNameNotAvailable,
    FileNameContainsSlash,
    FileNameEmpty;

    override fun toLbError(res: Resources): LbError = when(this) {
        NoAccount -> LbError.newUserError(getString(res, R.string.no_account))
        DocumentTreatedAsFolder -> LbError.newUserError(getString(res, R.string.document_treated_as_folder))
        CouldNotFindAParent -> LbError.newUserError(getString(res, R.string.could_not_find_a_parent))
        FileNameNotAvailable -> LbError.newUserError(getString(res, R.string.file_name_not_available))
        FileNameContainsSlash -> LbError.newUserError(getString(res, R.string.file_name_contains_slash))
        FileNameEmpty -> LbError.newUserError(getString(res, R.string.file_name_empty))
    }
}
@Serializable
enum class GetChildrenError : UiCoreError
@Serializable
enum class GetFileByIdError : UiCoreError {
    NoFileWithThatId;

    override fun toLbError(res: Resources): LbError = when(this) {
        NoFileWithThatId -> LbError.newUserError(getString(res, R.string.no_file_with_that_id))
    }
}
@Serializable
enum class FileDeleteError : UiCoreError {
    FileDoesNotExist,
    CannotDeleteRoot;

    override fun toLbError(res: Resources): LbError = when(this) {
        FileDoesNotExist -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
        CannotDeleteRoot -> LbError.newUserError(getString(res, R.string.cannot_delete_root))
    }
}
@Serializable
enum class ReadDocumentError : UiCoreError {
    TreatedFolderAsDocument,
    NoAccount,
    FileDoesNotExist;

    override fun toLbError(res: Resources): LbError = when(this) {
        TreatedFolderAsDocument -> LbError.newUserError(getString(res, R.string.folder_treated_as_document))
        NoAccount -> LbError.newUserError(getString(res, R.string.no_account))
        FileDoesNotExist -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
    }
}
@Serializable
enum class SaveDocumentToDiskError : UiCoreError {
    TreatedFolderAsDocument,
    NoAccount,
    FileDoesNotExist,
    BadPath,
    FileAlreadyExistsInDisk;

    override fun toLbError(res: Resources): LbError = when(this) {
        TreatedFolderAsDocument -> LbError.newUserError(getString(res, R.string.folder_treated_as_document))
        NoAccount -> LbError.newUserError(getString(res, R.string.no_account))
        FileDoesNotExist -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
        BadPath -> LbError.newUserError(getString(res, R.string.bad_path))
        FileAlreadyExistsInDisk -> LbError.newUserError(getString(res, R.string.file_already_exists_on_disk))
    }
}
@Serializable
enum class ExportDrawingToDiskError : UiCoreError {
    FolderTreatedAsDrawing,
    FileDoesNotExist,
    NoAccount,
    InvalidDrawing,
    BadPath,
    FileAlreadyExistsInDisk;

    override fun toLbError(res: Resources): LbError = when(this) {
        FolderTreatedAsDrawing -> LbError.newUserError(getString(res, R.string.folder_treated_as_drawing))
        FileDoesNotExist -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
        NoAccount -> LbError.newUserError(getString(res, R.string.no_account))
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
    NewNameEmpty,
    CannotRenameRoot;

    override fun toLbError(res: Resources): LbError = when(this) {
        FileDoesNotExist -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
        NewNameContainsSlash -> LbError.newUserError(getString(res, R.string.file_name_contains_slash))
        FileNameNotAvailable -> LbError.newUserError(getString(res, R.string.file_name_not_available))
        NewNameEmpty -> LbError.newUserError(getString(res, R.string.file_name_empty))
        CannotRenameRoot -> TODO()
    }
}
@Serializable
enum class MoveFileError : UiCoreError {
    NoAccount,
    FileDoesNotExist,
    DocumentTreatedAsFolder,
    TargetParentDoesNotExist,
    TargetParentHasChildNamedThat,
    CannotMoveRoot,
    FolderMovedIntoItself;

    override fun toLbError(res: Resources): LbError = when(this) {
        NoAccount -> LbError.newUserError(getString(res, R.string.no_account))
        FileDoesNotExist -> LbError.newUserError(getString(res, R.string.file_does_not_exist))
        DocumentTreatedAsFolder -> LbError.newUserError(getString(res, R.string.document_treated_as_folder))
        TargetParentDoesNotExist -> LbError.newUserError(getString(res, R.string.could_not_find_a_parent))
        TargetParentHasChildNamedThat -> LbError.newUserError(getString(res, R.string.target_parent_has_a_child_named_that))
        CannotMoveRoot -> LbError.newUserError(getString(res, R.string.cannot_move_root))
        FolderMovedIntoItself -> LbError.newUserError(getString(res, R.string.folder_moved_into_itself))
    }
}
@Serializable
enum class SyncAllError : UiCoreError {
    NoAccount,
    CouldNotReachServer,
    ClientUpdateRequired;

    override fun toLbError(res: Resources): LbError = when(this) {
        NoAccount -> LbError.newUserError(getString(res, R.string.no_account))
        CouldNotReachServer -> LbError.newUserError(getString(res, R.string.could_not_reach_server))
        ClientUpdateRequired -> LbError.newUserError(getString(res, R.string.client_update_required))
    }
}
@Serializable
enum class CalculateWorkError : UiCoreError {
    NoAccount,
    CouldNotReachServer,
    ClientUpdateRequired;

    override fun toLbError(res: Resources): LbError = when(this) {
        NoAccount -> LbError.newUserError(getString(res, R.string.no_account))
        CouldNotReachServer -> LbError.newUserError(getString(res, R.string.could_not_reach_server))
        ClientUpdateRequired -> LbError.newUserError(getString(res, R.string.client_update_required))
    }
}

open class SafeSerializer<T>(
    private val serializer: KSerializer<T>
): KSerializer<T?> {
    override val descriptor = serializer.descriptor

    // safe because @Serializable skips null fields
    override fun serialize(encoder: Encoder, value: T?) = encoder.encodeSerializableValue(serializer, value!!)

    override fun deserialize(decoder: Decoder): T? = try {
        decoder.decodeSerializableValue(serializer)
    } catch (_: Exception) {
        null
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
