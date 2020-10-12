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

func deserializeResult<T: Decodable, E: Decodable>(jsonResultStr: String) -> CoreResult<T, E> {
    do {
        return try JSONDecoder().decode(CoreResult.self, from: jsonResultStr.data(using: .utf8)!)
    } catch {
        return .failure(.Unexpected("\(error.localizedDescription) \(jsonResultStr)"))
    }
}

func fromPrimitiveResult<T: Decodable, E: Decodable>(result: UnsafePointer<Int8>) -> CoreResult<T, E> {
    let resultString = String(cString: result)
    release_pointer(UnsafeMutablePointer(mutating: result))
    
    return deserializeResult(jsonResultStr: resultString)
}

public struct Empty: Decodable {
    
}

enum TagContentKeys: String, CodingKey {
    case tag
    case content
}
