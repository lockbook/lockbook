import Foundation
import CLockbookCore

public func intEpochToString(epoch: UInt64) -> String {
    let date = Date(timeIntervalSince1970: TimeInterval(epoch/1000))
    let formatter = DateFormatter()
    formatter.dateFormat = "yyyy/MM/dd hh:mm a"
    return formatter.string(from: date)
}

public func timeAgo(epoch: UInt64) -> String {
    let date = Date(timeIntervalSince1970: TimeInterval(epoch/1000))

    let calendar = Calendar.current
    let minuteAgo = calendar.date(byAdding: .minute, value: -1, to: Date())!
    let hourAgo = calendar.date(byAdding: .hour, value: -1, to: Date())!
    let dayAgo = calendar.date(byAdding: .day, value: -1, to: Date())!
    let weekAgo = calendar.date(byAdding: .day, value: -7, to: Date())!
    if minuteAgo < date {
        return "just now"
    } else if hourAgo < date {
        let diff = Calendar.current.dateComponents([.minute], from: date, to: Date()).minute ?? 0
        return "\(diff) min ago"
    } else if dayAgo < date {
        let diff = Calendar.current.dateComponents([.hour], from: date, to: Date()).hour ?? 0
        return "\(diff) hrs ago"
    } else if weekAgo < date {
        let diff = Calendar.current.dateComponents([.day], from: date, to: Date()).day ?? 0
        return "\(diff) days ago"
    }
    
    let diff = Calendar.current.dateComponents([.weekOfYear], from: date, to: Date()).weekOfYear ?? 0
    return "\(diff) weeks ago"
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

func deserializeResult<T: Decodable, E: UiError>(jsonResultStr: String) -> FfiResult<T, E> {
    do {
        return try deserialize(data: jsonResultStr.data(using: .utf8)!).get()
    } catch {
        return .failure(.init(unexpected: "Swift Deserialization error: \(error.localizedDescription) \(jsonResultStr)"))
    }
}

func fromPrimitiveResult<T: Decodable, E: UiError>(result: UnsafePointer<Int8>) -> FfiResult<T, E> {
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
