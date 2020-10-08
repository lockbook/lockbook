import Foundation
import CLockbookCore

public func intEpochToString(epoch: UInt64) -> String {
    let date = Date(timeIntervalSince1970: TimeInterval(epoch/1000))
    let formatter = DateFormatter()
    formatter.dateFormat = "yyyy/MM/dd hh:mm a"
    return formatter.string(from: date)
}

func serialize<T: Encodable>(obj: T) -> Result<String, ApplicationError> {
    let encoder = JSONEncoder.init()
    encoder.keyEncodingStrategy = .convertToSnakeCase
    do {
        let data = try encoder.encode(obj)
        let output = String(data: data, encoding: .utf8) ?? ""
        return Result.success(output)
    } catch let error {
        return Result.failure(ApplicationError.Serialization("Failed serializing! \(error)"))
    }
}

func deserialize<T: Decodable>(data: Data) -> Result<T, Error> {
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    decoder.dateDecodingStrategy = .millisecondsSince1970
    do {
        let result = try decoder.decode(T.self, from: data)
        return Result.success(result)
    } catch let error {
        return Result.failure(error)
    }
}

func deserializeResult<T: Decodable>(jsonResultStr: String) -> Result<T, ApplicationError> {
    guard let dict = try? JSONSerialization.jsonObject(with: Data(jsonResultStr.utf8), options: []) as? [String: Any] else {
        return Result.failure(ApplicationError.Deserialization("Couldn't deserialize dict!", jsonResultStr))
    }
    
    if let ok = dict["Ok"] {
        guard let data = try? JSONSerialization.data(withJSONObject: ok, options: .fragmentsAllowed) else {
            return Result.failure(ApplicationError.Deserialization("Not valid JSON!", jsonResultStr))
        }
        return deserialize(data: data).mapError { ApplicationError.Deserialization("Failed deserializing! \($0)", jsonResultStr) }
    } else {
        if let err = dict["Err"] {
            guard let data = try? JSONSerialization.data(withJSONObject: err, options: .fragmentsAllowed) else {
                return Result.failure(ApplicationError.Deserialization("Not valid JSON!", jsonResultStr))
            }
            let coreErrorRes: Result<CoreError, Error> = deserialize(data: data)
            switch coreErrorRes {
            case .success(let coreError):
                return .failure(.Lockbook(coreError))
            case .failure(let err):
                return .failure(ApplicationError.Deserialization("Failed deserializing! \(err)", jsonResultStr))
            }
        } else {
            return Result.failure(ApplicationError.Deserialization("Could not find Ok or Err!", jsonResultStr))
        }
    }
}

func fromPrimitiveResult<T: Decodable>(result: UnsafePointer<Int8>) -> Result<T, ApplicationError> {
    let resultString = String(cString: result)
    release_pointer(UnsafeMutablePointer(mutating: result))
    
    return deserializeResult(jsonResultStr: resultString)
}

public struct Empty: Decodable {
    
}
