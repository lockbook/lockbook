//
//  LockbookApi.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/11/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

protocol LockbookApi {
    func isDbPresent() -> Bool
    func getAccount() -> Optional<String>
    func createAccount(username: String) -> Bool
    func updateMetadata() -> [FileMetadata]
}

struct CoreApi: LockbookApi {
    let documentsDirectory: String
    
    func isDbPresent() -> Bool {
        if (is_db_present(documentsDirectory) == 1) {
            return true
        }
        return false
    }
    
    func getAccount() -> Optional<String> {
        if (isDbPresent()) {
            let result = get_account(documentsDirectory)
            let resultString = String(cString: result!)
            release_pointer(UnsafeMutablePointer(mutating: result))
            return Optional.some(resultString)
        }
        return Optional.none
    }

    func createAccount(username: String) -> Bool {
        let result = create_account(documentsDirectory, username)
        if (result == 1) {
            return true
        }
        return false
    }
    
    func updateMetadata() -> [FileMetadata] {
        let result = list_files(documentsDirectory)
        let resultString = String(cString: result!)
        // We need to release the pointer once we have the result string
        release_pointer(UnsafeMutablePointer(mutating: result))
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

struct FakeApi: LockbookApi {
    var fakeUsername: String = "FakeApi"
    var fakeMetadata: [FileMetadata] = [
        FileMetadata(id: "aaaa", name: "first_file.md", path: "/", updatedAt: 0, status: "Remote"),
        FileMetadata(id: "bbbb", name: "another_file.md", path: "/", updatedAt: 1000, status: "Remote"),
        FileMetadata(id: "cccc", name: "third_file.md", path: "/", updatedAt: 1500, status: "Remote"),
    ]
    
    func isDbPresent() -> Bool {
        true
    }
    
    func getAccount() -> Optional<String> {
        Optional.some(fakeUsername)
    }
    
    func createAccount(username: String) -> Bool {
        false
    }
    
    func updateMetadata() -> [FileMetadata] {
        var rander = SystemRandomNumberGenerator()
        return fakeMetadata.shuffled(using: &rander)
    }
}
