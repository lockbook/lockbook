import Foundation

public enum FfiResult<T: Decodable, U: UiError> {
    case success(T)
    case failure(FfiError<U>)

    func get() throws -> T {
        switch self {
        case .success(let t):
            return t
        case .failure(let err):
            throw err
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
            self = .failure(.Unexpected("Failed to deserialize \(String(describing: Self.self)): \(container.codingPath)"))
        }
    }

    enum Keys: String, CodingKey {
        case Ok
        case Err
    }
}
