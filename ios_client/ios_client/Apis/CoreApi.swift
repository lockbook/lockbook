//
//  LockbookApi.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/11/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

struct CoreApi {
    func get_files() -> [FileMetadata] {
        let result = list_files()
        let resultString = String(cString: result!)
        // We need to release the pointer once we have the result string
        list_files_release(UnsafeMutablePointer(mutating: result))
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        
        do {
            print("Incoming JSON \(resultString)")
            let files = try decoder.decode([FileMetadata].self, from: Data(resultString.utf8))
            return files
        } catch {
            print("Serialization Error: \(error)")
            return []
        }
    }
}
