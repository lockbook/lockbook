import Foundation

public enum AnyFfiResult<T: Decodable> {
    case success(T)
    case failure(AnyFfiError)
    
    public func flatMap<S>(transform: (T) -> AnyFfiResult<S>) -> AnyFfiResult<S> {
        switch self {
        case .success(let t):
            return transform(t)
        case .failure(let err):
            return .failure(err)
        }
    }
}

public enum FfiResult<T: Decodable, U: UiError> {
    case success(T)
    case failure(FfiError<U>)
    
    public func get() throws -> T {
        switch self {
        case .success(let t):
            return t
        case .failure(let err):
            throw err
        }
    }
    
    public func map<S>(transform: (T) -> S) -> FfiResult<S, U> {
        switch self {
        case .success(let t):
            return .success(transform(t))
        case .failure(let err):
            return .failure(err)
        }
    }
    
    public func flatMap<S>(transform: (T) -> FfiResult<S, U>) -> FfiResult<S, U> {
        switch self {
        case .success(let t):
            return transform(t)
        case .failure(let err):
            return .failure(err)
        }
    }
    
    public func eraseError() -> AnyFfiResult<T> {
        switch self {
        case .success(let s):
            return .success(s)
        case .failure(let e):
            return .failure(e)
        }
    }
}

extension FfiResult: Decodable {
    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        
        if (container.contains(.Ok)) {
            self = .success(try container.decode(T.self, forKey: .Ok))
        } else if (container.contains(.Err)) {
            self = .failure(try container.decode(FfiError.self, forKey: .Err))
        } else {
            self = .failure(.init(unexpected: "Failed to deserialize \(String(describing: Self.self)): \(container.codingPath)"))
        }
    }
    
    enum Keys: String, CodingKey {
        case Ok
        case Err
    }
}
