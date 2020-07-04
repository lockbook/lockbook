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
