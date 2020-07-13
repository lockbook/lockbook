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

func deserialize<T: Decodable>(jsonStr: String) -> Result<T, Error> {
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    decoder.dateDecodingStrategy = .millisecondsSince1970
    do {
        let result = try decoder.decode(T.self, from: Data(jsonStr.utf8))
        return Result.success(result)
    } catch let error {
        print("Incoming JSON \(jsonStr)")
        return Result.failure(error)
    }
}

func fromPrimitiveResult<T: Decodable>(result: ResultWrapper) -> Result<T, CoreError> {
    if (!result.is_error) {
        let successString = String(cString: result.value.success)
        release_pointer(UnsafeMutablePointer(mutating: result.value.success))
        
        let result: Result<T, Error> = deserialize(jsonStr: successString)
        switch result {
            case .success(let value):
                return Result.success(value)
            case .failure(let err):
                return Result.failure(CoreError(message: err.localizedDescription, type: .Unhandled))
        }
    } else {
        let error: ErrorType;
        
        switch result.error {
        case Network:
            error = .Network
        case Database:
            error = .Database
        default:
            error = .Unhandled
        }
        
        let errorString = String(cString: result.value.error)
        release_pointer(UnsafeMutablePointer(mutating: result.value.error))
        return Result.failure(CoreError(message: errorString, type: error))
    }
}
