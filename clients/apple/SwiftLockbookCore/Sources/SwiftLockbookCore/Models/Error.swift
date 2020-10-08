import Foundation

public enum ApplicationError: Error {
    case Lockbook(CoreError)
    case LockbookUnhandled(String)
    case Serialization(String)
    case Deserialization(String, String)
    case State(String)
    
    public func message() -> String {
        switch self {
        case .Lockbook(let coreErr):
            return coreErr.rawValue
        case .LockbookUnhandled(let err):
            return err
        case .Serialization(let errMsg):
            return errMsg
        case .Deserialization(let errMsg, let json):
            return "\(errMsg) >>>> JSON: \(json)"
        case .State(let errMsg):
            return errMsg
        }
    }
    
    public static func lazy() -> ApplicationError {
        LockbookUnhandled("Lazy!")
    }
}

public enum CoreError: String, Decodable {
    case AccountDoesNotExist
    case AccountExistsAlready
    case AccountStringCorrupted
    case CannotMoveRoot
    case CannotRenameRoot
    case ClientUpdateRequired
    case CouldNotFindAParent
    case CouldNotReachServer
    case DocumentTreatedAsFolder
    case FileAlreadyExists
    case FileDoesNotExist
    case FileNameContainsSlash
    case FileNameEmpty
    case FileNameNotAvailable
    case FolderTreatedAsDocument
    case InvalidUsername
    case NewNameContainsSlash
    case NewNameEmpty
    case NoAccount
    case NoFileAtThatPath
    case NoFileWithThatId
    case NoRoot
    case PathContainsEmptyFile
    case PathDoesntStartWithRoot
    case StateRequiresCleaning
    case TargetParentDoesNotExist
    case TargetParentHasChildNamedThat
    case TreatedFolderAsDocument
    case UsernamePKMismatch
    case UsernameTaken
    // Sort of junk
    case BadAccount // Contains (GetAccountError)
    case ExecuteWorkError // Contains ([ExecuteWorkError])
    case UnexpectedError // Conttains (String)

}
