package app.lockbook.utils

sealed class CoreError {
    object UsernameTaken : CoreError()
    object InvalidUsername : CoreError()
    object AccountStringCorrupted : CoreError()
    object AccountDoesNotExist : CoreError()
    object UsernamePKMismatch : CoreError()
    object FolderTreatedAsDocument : CoreError()
    object CouldNotFindAParent : CoreError()
    object FileNameContainsSlash : CoreError()
    object FileNameEmpty : CoreError()
    object NoRoot : CoreError()
    object NoFileWithThatId : CoreError()
    object TreatedFolderAsDocument : CoreError()
    object FileDoesNotExist : CoreError()
    object NewNameContainsSlash : CoreError()
    object FileNameNotAvailable : CoreError()
    object NewNameEmpty : CoreError()
    object CannotRenameRoot : CoreError()
    object DocumentTreatedAsFolder : CoreError()
    object TargetParentDoesNotExist : CoreError()
    object TargetParentHasChildNamedThat : CoreError()
    object CannotMoveRoot : CoreError()
    object ExecuteWorkError : CoreError()
    object NoAccount : CoreError()
    object AccountExistsAlready : CoreError()
    object CouldNotReachServer : CoreError()

    data class Unexpected(val error: String) : CoreError()
}

fun matchErrorName(name: String): CoreError {
    return when (name) {
        CoreError.UsernameTaken::class.simpleName -> CoreError.UsernameTaken
        CoreError.InvalidUsername::class.simpleName -> CoreError.InvalidUsername
        CoreError.AccountStringCorrupted::class.simpleName -> CoreError.AccountStringCorrupted
        CoreError.AccountDoesNotExist::class.simpleName -> CoreError.AccountDoesNotExist
        CoreError.UsernamePKMismatch::class.simpleName -> CoreError.UsernamePKMismatch
        CoreError.FolderTreatedAsDocument::class.simpleName -> CoreError.FolderTreatedAsDocument
        CoreError.CouldNotFindAParent::class.simpleName -> CoreError.CouldNotFindAParent
        CoreError.FileNameContainsSlash::class.simpleName -> CoreError.FileNameContainsSlash
        CoreError.FileNameEmpty::class.simpleName -> CoreError.FileNameEmpty
        CoreError.NoRoot::class.simpleName -> CoreError.NoRoot
        CoreError.NoFileWithThatId::class.simpleName -> CoreError.NoFileWithThatId
        CoreError.TreatedFolderAsDocument::class.simpleName -> CoreError.TreatedFolderAsDocument
        CoreError.FileDoesNotExist::class.simpleName -> CoreError.FileDoesNotExist
        CoreError.NewNameContainsSlash::class.simpleName -> CoreError.NewNameContainsSlash
        CoreError.FileNameNotAvailable::class.simpleName -> CoreError.FileNameNotAvailable
        CoreError.NewNameEmpty::class.simpleName -> CoreError.NewNameEmpty
        CoreError.CannotRenameRoot::class.simpleName -> CoreError.CannotRenameRoot
        CoreError.DocumentTreatedAsFolder::class.simpleName -> CoreError.DocumentTreatedAsFolder
        CoreError.TargetParentDoesNotExist::class.simpleName -> CoreError.TargetParentDoesNotExist
        CoreError.TargetParentHasChildNamedThat::class.simpleName -> CoreError.TargetParentHasChildNamedThat
        CoreError.CannotMoveRoot::class.simpleName -> CoreError.CannotMoveRoot
        CoreError.ExecuteWorkError::class.simpleName -> CoreError.ExecuteWorkError
        CoreError.NoAccount::class.simpleName -> CoreError.NoAccount
        CoreError.AccountExistsAlready::class.simpleName -> CoreError.AccountExistsAlready
        CoreError.CouldNotReachServer::class.simpleName -> CoreError.CouldNotReachServer
        else -> CoreError.Unexpected("Couldn't match content: $name")
    }
}
