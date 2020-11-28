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

public class FfiError<U: UiError>: AnyFfiError, Decodable {
    let kind: Kind
    
    enum Kind {
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

public protocol UiError: Decodable, Error {
    
}

public enum GetStateError: String, UiError {
    case Stub
}

public enum MigrationError: String, UiError {
    case StateRequiresCleaning
}

public enum CreateAccountError: String, UiError {
    case AccountExistsAlready
    case ClientUpdateRequired
    case CouldNotReachServer
    case InvalidUsername
    case UsernameTaken
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

public enum GetAccountError: String, UiError {
    case NoAccount
}

public enum CreateFileAtPathError: String, UiError {
    case DocumentTreatedAsFolder
    case FileAlreadyExists
    case NoAccount
    case NoRoot
    case PathContainsEmptyFile
    case PathDoesntStartWithRoot
}

public enum WriteToDocumentError: String, UiError {
    case FileDoesNotExist
    case FolderTreatedAsDocument
    case NoAccount
}

public enum CreateFileError: String, UiError {
    case CouldNotFindAParent
    case DocumentTreatedAsFolder
    case FileNameContainsSlash
    case FileNameEmpty
    case FileNameNotAvailable
    case NoAccount
}

public enum GetRootError: String, UiError {
    case NoRoot
}

public enum GetChildrenError: String, UiError {
    case Stub
}

public enum GetFileByIdError: String, UiError {
    case NoFileWithThatId
}

public enum GetFileByPathError: String, UiError {
    case NoFileAtThatPath
}

public enum InsertFileError: String, UiError {
    case Stub
}

public enum ReadDocumentError: String, UiError {
    case FileDoesNotExist
    case NoAccount
    case TreatedFolderAsDocument
}

public enum ListPathsError: String, UiError {
    case Stub
}

public enum ListMetadatasError: String, UiError {
    case Stub
}

public enum RenameFileError: String, UiError {
    case CannotRenameRoot
    case FileDoesNotExist
    case FileNameNotAvailable
    case NewNameContainsSlash
    case NewNameEmpty
}

public enum MoveFileError: String, UiError {
    case CannotMoveRoot
    case DocumentTreatedAsFolder
    case FileDoesNotExist
    case FolderMovedIntoItself
    case NoAccount
    case TargetParentDoesNotExist
    case TargetParentHasChildNamedThat
}

public enum SyncAllError: String, UiError {
    case NoAccount
    case ClientUpdateRequired
    case CouldNotReachServer
    case ExecuteWorkError
}
public enum CalculateWorkError: String, UiError {
    case NoAccount
    case CouldNotReachServer
    case ClientUpdateRequired
}
public enum ExecuteWorkError: String, UiError {
    case CouldNotReachServer
    case ClientUpdateRequired
    case BadAccount
}
public enum SetLastSyncedError: String, UiError {
    case Stub
}
public enum GetLastSyncedError: String, UiError {
    case Stub
}
public enum GetUsageError: String, UiError {
    case NoAccount
    case CouldNotReachServer
    case ClientUpdateRequired
}

public enum FileDeleteError: String, UiError {
    case CannotDeleteRoot
    case FileDoesNotExist
}
