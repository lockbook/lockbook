//
//  Account.swift
//  ios
//
//  Created by Raayan Pillai on 7/6/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

struct Account: Codable {
    typealias Username = String
    var username: Username
    // var keys: String
}

extension Account: Identifiable {
    var id: String { username }
}
