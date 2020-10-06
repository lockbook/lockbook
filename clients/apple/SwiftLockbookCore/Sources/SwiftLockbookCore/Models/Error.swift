import Foundation

public enum ApplicationError: Error {
    case Lockbook(CoreError)
    case Serialization(String)
    case State(String)
    case General(Error)
    
    public func message() -> String {
        switch self {
        case .Lockbook(let coreErr):
            return coreErr.message
        case .Serialization(let errMsg):
            return errMsg
        case .State(let errMsg):
            return errMsg
        case .General(let err):
            return err.localizedDescription
        }
    }
}

public struct CoreError: Error {
    var message: String
    var type: ErrorType
    
    static func lazy() -> CoreError {
        return CoreError.init(message: "Lazy error!", type: .Unhandled)
    }
}

public enum ErrorType {
    case Network
    case Database
    case Unhandled
}
