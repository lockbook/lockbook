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
    func updateMetadata(sync: Bool) -> [FileMetadata]
    func createFile(name: String, path: String) -> Optional<FileMetadata>
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
    
    func updateMetadata(sync: Bool) -> [FileMetadata] {
        let result = list_files(documentsDirectory, sync)
        let resultString = String(cString: result!)
        // We need to release the pointer once we have the result string
        release_pointer(UnsafeMutablePointer(mutating: result))
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        
        if let resultMetas: [FileMetadata] = deserialize(jsonStr: resultString) {
            return resultMetas
        } else {
            return [FileMetadata].init()
        }
    }
    
    func createFile(name: String, path: String) -> Optional<FileMetadata> {
        let result = create_file(documentsDirectory, name, path)
        let resultString = String(cString: result!)
        release_pointer(UnsafeMutablePointer(mutating: result))
        
        let resultMeta: Optional<FileMetadata> = deserialize(jsonStr: resultString)
        return resultMeta
    }
}

fileprivate func deserialize<T: Decodable>(jsonStr: String) -> Optional<T> {
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    do {
        print("Incoming JSON \(jsonStr)")
        let result = try decoder.decode(T.self, from: Data(jsonStr.utf8))
        return Optional.some(result)
    } catch {
        print("Serialization Error: \(error)")
        return Optional.none
    }
}

struct FakeApi: LockbookApi {
    var fakeUsername: String = "FakeApi"
    var fakeMetadatas: [FileMetadata] = [
        FileMetadata(id: "aaaa", name: "first_file.md", path: "/", updatedAt: 0, status: .Synced),
        FileMetadata(id: "bbbb", name: "another_file.md", path: "/", updatedAt: 1000, status: .Synced),
        FileMetadata(id: "cccc", name: "third_file.md", path: "/", updatedAt: 1500, status: .Local),
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
    
    func updateMetadata(sync: Bool) -> [FileMetadata] {
        var rander = SystemRandomNumberGenerator()
        return fakeMetadatas.shuffled(using: &rander)
    }
    
    func createFile(name: String, path: String) -> Optional<FileMetadata> {
        let now = Date().timeIntervalSince1970

        return Optional.some(FileMetadata(id: "new", name: name, path: path, updatedAt: Int(now), status: .Local))
    }
}
