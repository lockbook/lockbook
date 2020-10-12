import Foundation

public enum CoreError<E: Decodable & Equatable>: Error, Equatable, Decodable {
    public static func == (lhs: CoreError<E>, rhs: CoreError<E>) -> Bool {
        switch (lhs, rhs) {
        case (UIError(let lui), UIError(let rui)):
            return lui == rui
        case (Unexpected(let lui), Unexpected(let rui)):
            return lui == rui
        default:
            return false
        }
    }

    case UIError(E)
    case Unexpected(String)

    enum ErrorTypes: String, Decodable {
        case UiError
        case Unexpected
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: TagContentKeys.self)
        let error = try container.decode(ErrorTypes.self, forKey: .tag)
        switch error {
        case .UiError:
            self = .UIError(try container.decode(E.self, forKey: .content))
        case .Unexpected:
            self = .Unexpected(try container.decode(String.self, forKey: .content))
        }
    }
    
    public static func lazy() -> Self {
        Unexpected("Lazy!")
    }
}

public enum InitLoggerError: String, Decodable {
    case Stub
}

public enum GetStateError: String, Decodable {
    case Stub
}

public enum MigrationError: String, Decodable {
    case StateRequiresCleaning
}

public enum CreateAccountError: String, Decodable {
    case AccountExistsAlready
    case ClientUpdateRequired
    case CouldNotReachServer
    case InvalidUsername
    case UsernameTaken
}

public enum ImportError: String, Decodable {
    case AccountDoesNotExist
    case AccountExistsAlready
    case AccountStringCorrupted
    case ClientUpdateRequired
    case CouldNotReachServer
    case UsernamePKMismatch
}

public enum AccountExportError: String, Decodable {
    case NoAccount
}

public enum GetAccountError: String, Decodable {
    case NoAccount
}

public enum CreateFileAtPathError: String, Decodable {
    case DocumentTreatedAsFolder
    case FileAlreadyExists
    case NoAccount
    case NoRoot
    case PathContainsEmptyFile
    case PathDoesntStartWithRoot
}

public enum WriteToDocumentError: String, Decodable {
    case FileDoesNotExist
    case FolderTreatedAsDocument
    case NoAccount
}

public enum CreateFileError: String, Decodable {
    case CouldNotFindAParent
    case DocumentTreatedAsFolder
    case FileNameContainsSlash
    case FileNameEmpty
    case FileNameNotAvailable
    case NoAccount
}

public enum GetRootError: String, Decodable {
    case NoRoot
}

public enum GetChildrenError: String, Decodable {
    case Stub
}

public enum GetFileByIdError: String, Decodable {
    case NoFileWithThatId
}

public enum GetFileByPathError: String, Decodable {
    case NoFileAtThatPath
}

public enum InsertFileError: String, Decodable {
    case Stub
}

public enum DeleteFileError: String, Decodable {
    case NoFileWithThatId
}

public enum ReadDocumentError: String, Decodable {
    case FileDoesNotExist
    case NoAccount
    case TreatedFolderAsDocument
}

public enum ListPathsError: String, Decodable {
    case Stub
}

public enum ListMetadatasError: String, Decodable {
    case Stub
}

public enum RenameFileError: String, Decodable {
    case CannotRenameRoot
    case FileDoesNotExist
    case FileNameNotAvailable
    case NewNameContainsSlash
    case NewNameEmpty
}

public enum MoveFileError: String, Decodable {
    case CannotMoveRoot
    case DocumentTreatedAsFolder
    case FileDoesNotExist
    case NoAccount
    case TargetParentDoesNotExist
    case TargetParentHasChildNamedThat
}

public enum SyncAllError: String, Decodable {
    case NoAccount
    case CouldNotReachServer
    case ExecuteWorkError
}
public enum CalculateWorkError: String, Decodable {
    case NoAccount
    case CouldNotReachServer
    case ClientUpdateRequired
}
public enum ExecuteWorkError: String, Decodable {
    case CouldNotReachServer
    case ClientUpdateRequired
    case BadAccount
}
public enum SetLastSyncedError: String, Decodable {
    case Stub
}
public enum GetLastSyncedError: String, Decodable {
    case Stub
}
public enum GetUsageError: String, Decodable {
    case NoAccount
    case CouldNotReachServer
    case ClientUpdateRequired
}
