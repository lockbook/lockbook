//
//  Account.swift
//  ios
//
//  Created by Raayan Pillai on 7/6/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

public struct Account: Codable {
    public typealias Username = String
    
    public init(username: Username) {
        self.username = username
    }
    
    public var username: Username
    // var keys: String
}

extension Account: Identifiable {
    public var id: String { username }
}
