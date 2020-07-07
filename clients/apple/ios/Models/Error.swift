//
//  Error.swift
//  ios
//
//  Created by Raayan Pillai on 7/6/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

struct CoreError: Error, Codable {
    var message: String
    
    static func lazy() -> CoreError {
        return CoreError.init(message: "Lazy error!")
    }
}
