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

func deserialize<T: Decodable>(jsonStr: String) -> Optional<T> {
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    do {
        let result = try decoder.decode(T.self, from: Data(jsonStr.utf8))
        return Optional.some(result)
    } catch {
        print("Incoming JSON \(jsonStr)")
        print("Serialization Error: \(error)")
        return Optional.none
    }
}

func fromPrimitiveResult<T: Decodable>(result: ResultWrapper) -> Result<T, GeneralError> {
    if (!result.is_error) {
        let successString = String(cString: result.value.success)
        release_pointer(UnsafeMutablePointer(mutating: result.value.success))
        
        if let success: T = deserialize(jsonStr: successString) {
            return Result.success(success)
        } else {
            return Result.failure(GeneralError.init(message: successString, type: .Success))
        }
    } else {
        let errorString = String(cString: result.value.error)
        release_pointer(UnsafeMutablePointer(mutating: result.value.error))
        
        if let error: GeneralError = deserialize(jsonStr: errorString) {
            return Result.failure(error)
        } else {
            return Result.failure(GeneralError(message: errorString, type: .Error))
        }
    }
//    return Result.failure(GeneralError())
}

enum ErrorType: String, Codable {
    case Success
    case Error
}

struct GeneralError: Decodable & Error {
    var message: String
    var type: ErrorType
}


