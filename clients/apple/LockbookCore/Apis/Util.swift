//
//  Util.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/19/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

func intEpochToString(epoch: Int) -> String {
    let date = Date(timeIntervalSince1970: TimeInterval(epoch/1000))
    let formatter = DateFormatter()
    formatter.dateFormat = "yyyy/MM/dd hh:mm a"
    return formatter.string(from: date)
}

func deserialize<T: Decodable>(data: Data) -> Result<T, Error> {
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    decoder.dateDecodingStrategy = .millisecondsSince1970
    do {
        let result = try decoder.decode(T.self, from: data)
        return Result.success(result)
    } catch let error {
//        print("Incoming JSON \(data)")
        return Result.failure(error)
    }
}

func deserializeResult<T: Decodable>(jsonResultStr: String) -> Result<T, ApplicationError> {
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
    
//    let result: Result<T, ApplicationError> = deserializeResult(jsonResultStr: resultString)
    return deserializeResult(jsonResultStr: resultString)
//    switch result {
//        case .success(let value):
//            if let valueString = value {
//                let result: Result<T, Error> = deserialize(jsonStr: valueString)
//                return result.mapError { (err) -> ApplicationError in
//                    return ApplicationError.General(err)
//                }
//            } else {
//                return Result.failure(ApplicationError.Serialization("Ok value missing! Was this a unit?"))
//            }
//        case .failure(let err):
//            return Result.failure(err)
//    }
}

struct Empty: Decodable {
    
}
