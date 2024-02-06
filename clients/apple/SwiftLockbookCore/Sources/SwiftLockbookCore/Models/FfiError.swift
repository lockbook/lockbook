import Foundation

public class AnyFfiError: Error, Equatable {
    public let message: String
    
    init(message: String) {
        self.message = message
    }
    
    public static func == (lhs: AnyFfiError, rhs: AnyFfiError) -> Bool {
        lhs.message == rhs.message
    }
}

public class ErrorWithTitle: AnyFfiError {
    public let title: String
    
    public init(title: String, message: String) {
        self.title = title
        super.init(message: message)
    }
}

public class FfiError<U: UiError>: AnyFfiError, Decodable {
    public let kind: Kind
    
    public enum Kind: Equatable {
        case UiError(U)
        case Unexpected(String)
    }
    
    enum Keys: String, Decodable {
        case UiError
        case Unexpected
    }
    
    public init(unexpected: String) {
        self.kind = .Unexpected(unexpected)
        super.init(message: "\(kind)")
    }
    
    public init(_ error: U) {
        self.kind = .UiError(error)
        super.init(message: "\(kind)")
    }
    
    required public convenience init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: TagContentKeys.self)
        let error = try container.decode(Keys.self, forKey: .tag)
        switch error {
        case .UiError:
            self.init(try container.decode(U.self, forKey: .content))
        case .Unexpected:
            self.init(unexpected: try container.decode(String.self, forKey: .content))
        }
    }
}

extension FfiError: CustomStringConvertible {
    public var description: String {
        "\(String(describing: Self.self)): \(message)"
    }
}

public protocol UiError: Decodable, Equatable, Error {}

public enum CreateAccountError: String, UiError {
    case AccountExistsAlready
    case ClientUpdateRequired
    case CouldNotReachServer
    case InvalidUsername
    case UsernameTaken
    case ServerDisabled
}

public enum ImportError: String, UiError {
    case AccountDoesNotExist
    case AccountExistsAlready
    case AccountStringCorrupted
    case ClientUpdateRequired
    case CouldNotReachServer
    case UsernamePKMismatch
}

public enum AccountExportError: String, UiError {
    case NoAccount
}

public enum DeleteAccountError: String, UiError {
    case CouldNotReachServer
    case ClientUpdateRequired
}

public enum GetAccountError: String, UiError {
    case NoAccount
}

public enum CreateFileAtPathError: String, UiError {
    case DocumentTreatedAsFolder
    case FileAlreadyExists
    case NoRoot
    case PathContainsEmptyFile
    case PathDoesntStartWithRoot
    case InsufficientPermission
}

public enum WriteToDocumentError: String, UiError {
    case FileDoesNotExist
    case FolderTreatedAsDocument
    case InsufficientPermission
}

public enum CreateFileError: String, UiError {
    case CouldNotFindAParent
    case DocumentTreatedAsFolder
    case FileNameContainsSlash
    case FileNameEmpty
    case FileNameNotAvailable
    case FileNameTooLong
    case LinkInSharedFolder
    case LinkTargetIsOwned
    case LinkTargetNonexistent
    case MultipleLinksToSameFile
    case InsufficientPermission
}

public enum GetRootError: String, UiError {
    case NoRoot
}

public enum GetChildrenError: String, UiError {
    case Stub
}

public enum InitError: String, UiError {
    case Stub
}

public enum GetFileByIdError: String, UiError {
    case NoFileWithThatId
}

public enum GetFileByPathError: String, UiError {
    case NoFileAtThatPath
}

public enum ReadDocumentError: String, UiError {
    case FileDoesNotExist
    case TreatedFolderAsDocument
}

public enum ListPathsError: String, UiError {
    case Stub
}

public enum ListMetadatasError: String, UiError {
    case Stub
}

public enum GetPathByIdError: String, UiError {
    case Stub
}

public enum RenameFileError: String, UiError {
    case CannotRenameRoot
    case FileDoesNotExist
    case FileNameNotAvailable
    case FileNameTooLong
    case NewNameContainsSlash
    case NewNameEmpty
    case InsufficientPermission
}

public enum MoveFileError: String, UiError {
    case CannotMoveRoot
    case DocumentTreatedAsFolder
    case FileDoesNotExist
    case FolderMovedIntoItself
    case TargetParentDoesNotExist
    case TargetParentHasChildNamedThat
    case LinkInSharedFolder
    case InsufficientPermission
}

public enum SyncAllError: String, UiError {
    case ClientUpdateRequired
    case CouldNotReachServer
    case Retry
    case UsageIsOverDataCap
}
public enum CalculateWorkError: String, UiError {
    case CouldNotReachServer
    case ClientUpdateRequired
}
public enum GetLastSyncedError: String, UiError {
    case Stub
}
public enum GetUsageError: String, UiError {
    case CouldNotReachServer
    case ClientUpdateRequired
}

public enum FileDeleteError: String, UiError {
    case CannotDeleteRoot
    case FileDoesNotExist
    case InsufficientPermission
}
public enum GetLocalChangesError: String, UiError {
    case Stub
}

public enum GetDrawingError: String, UiError {
    case FolderTreatedAsDrawing
    case InvalidDrawing
    case FileDoesNotExist
}

public enum SaveDrawingError: String, UiError {
    case FileDoesNotExist
    case FolderTreatedAsDrawing
    case InvalidDrawing
}

public enum ExportDrawingError: String, UiError {
    case FolderTreatedAsDrawing
    case FileDoesNotExist
    case InvalidDrawing
}

public enum UpgradeAccountAppStoreError: String, UiError {
    case AppStoreAccountAlreadyLinked
    case AlreadyPremium
    case InvalidAuthDetails
    case ExistingRequestPending
    case CouldNotReachServer
    case ClientUpdateRequired
}

public enum CancelSubscriptionError: String, UiError {
    case NotPremium
    case AlreadyCanceled
    case UsageIsOverFreeTierDataCap
    case ExistingRequestPending
    case CouldNotReachServer
    case ClientUpdateRequired
    case CannotCancelForAppStore
}

public enum ShareFileError: String, UiError {
    case CannotShareRoot
    case FileNonexistent
    case ShareAlreadyExists
    case LinkInSharedFolder
    case InsufficientPermission
}

public enum GetPendingShares: String, UiError {
    case Stub
}

public enum DeletePendingShareError: String, UiError {
    case FileNonexistent
    case ShareNonexistent
}

public enum ImportFilesError: String, UiError {
    case FileNonexistent
    case FileNotFolder
}

public enum ExportFileError: String, UiError {
    case FileNonexistent
    case DiskPathInvalid
    case DiskPathTaken
}

public enum ExportDrawingToDiskError: String, UiError {
    case FolderTreatedAsDrawing
    case FileDoesNotExist
    case InvalidDrawing
    case BadPath
    case FileAlreadyExistsInDisk
}

public enum SearchFilePathsError: String, UiError {
    case Stub
}

public enum GeneralSearchError: String, UiError {
    case Stub
}

public enum SuggestedDocsError: String, UiError {
    case Stub
}
