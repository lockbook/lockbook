//
//  Error.swift
//  ios
//
//  Created by Raayan Pillai on 7/6/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

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
