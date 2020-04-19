//
//  Util.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/19/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

func intEpochToString(micros: Int) -> String {
    let date = Date(timeIntervalSince1970: TimeInterval(micros/1000000))
    let formatter = DateFormatter()
    formatter.dateFormat = "yyyy/mm/dd hh:mm a"
    return formatter.string(from: date)
}
