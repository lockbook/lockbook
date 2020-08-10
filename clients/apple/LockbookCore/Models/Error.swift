//
//  Error.swift
//  ios
//
//  Created by Raayan Pillai on 7/6/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

enum ApplicationError: Error {
    case Lockbook(CoreError)
    case Serialization(String)
    case State(String)
    case General(Error)
    
    func message() -> String {
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

struct CoreError: Error {
    var message: String
    var type: ErrorType
    
    static func lazy() -> CoreError {
        return CoreError.init(message: "Lazy error!", type: .Unhandled)
    }
}

enum ErrorType {
    case Network
    case Database
    case Unhandled
}
