import Foundation
import CLockbookCore

public func intEpochToString(epoch: UInt64) -> String {
    let date = Date(timeIntervalSince1970: TimeInterval(epoch/1000))
    let formatter = DateFormatter()
    formatter.dateFormat = "yyyy/MM/dd hh:mm a"
    return formatter.string(from: date)
}

func serialize<T: Encodable>(obj: T) -> Result<String, Error> {
    let encoder = JSONEncoder.init()
    encoder.keyEncodingStrategy = .convertToSnakeCase
    do {
        let data = try encoder.encode(obj)
        let output = String(data: data, encoding: .utf8) ?? ""
//        print("Outgoing JSON \(output)")
        return Result.success(output)
    } catch let error {
        return Result.failure(error)
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
//    print("Incoming JSON \(jsonResultStr)")
    guard let dict = try? JSONSerialization.jsonObject(with: Data(jsonResultStr.utf8), options: []) as? [String: Any] else {
        return Result.failure(ApplicationError.Serialization("Couldn't deserialize dict!"))
    }
    
    if let ok = dict["Ok"] {
        guard let data = try? JSONSerialization.data(withJSONObject: ok, options: .fragmentsAllowed) else {
            return Result.failure(ApplicationError.Serialization("Not valid JSON!"))
        }
        return deserialize(data: data).mapError { ApplicationError.General($0) }
    } else {
        if let err = dict["Err"] {
            guard let data = try? JSONSerialization.data(withJSONObject: err, options: .fragmentsAllowed) else {
                return Result.failure(ApplicationError.Serialization("Not valid JSON!"))
            }
            guard let errMsg = String.init(data: data, encoding: .utf8) else {
                return Result.failure(ApplicationError.Serialization("Err was not a UTF-8 string!"))
            }
            return Result.failure(ApplicationError.Lockbook(CoreError.init(message: errMsg, type: .Unhandled)))
        } else {
            return Result.failure(ApplicationError.Serialization("Could not find Ok or Err!"))
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
