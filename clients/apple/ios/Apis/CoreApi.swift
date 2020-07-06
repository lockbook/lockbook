//
//  LockbookApi.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/11/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

protocol LockbookApi {
    func getAccount() -> Optional<String>
    func createAccount(username: String) -> Bool
    func importAccount(accountString: String) -> Bool
    func sync() -> [FileMetadata]
    func calculateWork() -> Result<[WorkUnit], GeneralError>
    func getRoot() -> UUID
    func listFiles(dirId: UUID) -> [FileMetadata]
    func createFile(name: String) -> Optional<FileMetadata>
    func getFile(id: UUID) -> Optional<DecryptedValue>
    func updateFile(id: UUID, content: String) -> Bool
    func markFileForDeletion(id: UUID) -> Bool
    func purgeLocal() -> Bool
}

struct CoreApi: LockbookApi {
    let documentsDirectory: String
    
    private func isDbPresent() -> Bool {
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
    
    func importAccount(accountString: String) -> Bool {
        let result = import_account(documentsDirectory, accountString)
        if (result == 1) {
            return true
        }
        return false
    }
    
    func sync() -> [FileMetadata] {
        if (isDbPresent()) {
            let result = sync_files(documentsDirectory)
            let resultString = String(cString: result!)
            // We need to release the pointer once we have the result string
            release_pointer(UnsafeMutablePointer(mutating: result))
         
            
            if let resultMetas: [FileMetadata] = try? deserialize(jsonStr: resultString).get() {
                return resultMetas
            }
        }
        return [FileMetadata].init()
    }
    
    func calculateWork() -> Result<[WorkUnit], GeneralError> {
        let result: Result<[WorkUnit], GeneralError> = fromPrimitiveResult(result: calculate_work(documentsDirectory))
        
        return result
    }
    
    func getRoot() -> UUID {
        let result = get_root(documentsDirectory)
        let resultString = String(cString: result!)
        release_pointer(UnsafeMutablePointer(mutating: result))
        
        return UUID(uuidString: resultString)!
    }
    
    func listFiles(dirId: UUID) -> [FileMetadata] {
        if (isDbPresent()) {
            let result = list_files(documentsDirectory, dirId.uuidString)
            let resultString = String(cString: result!)
            release_pointer(UnsafeMutablePointer(mutating: result))
            
            if let files: [FileMetadata] = try? deserialize(jsonStr: resultString).get() {
                return files
            }
        }
        return [FileMetadata].init()
    }

    func createFile(name: String) -> Optional<FileMetadata> {
        let rootId = getRoot()
        print(rootId)
        let result = create_file(documentsDirectory, name, rootId.uuidString)
        let resultString = String(cString: result!)
        release_pointer(UnsafeMutablePointer(mutating: result))
        
        let resultMeta: Optional<FileMetadata> = try? deserialize(jsonStr: resultString).get()
        return resultMeta
    }
    
    func getFile(id: UUID) -> Optional<DecryptedValue> {
        let result = get_file(documentsDirectory, id.uuidString)
        let resultString = String(cString: result!)
        release_pointer(UnsafeMutablePointer(mutating: result))
        
        let resultFile: Optional<DecryptedValue> = try? deserialize(jsonStr: resultString).get()
        return resultFile
    }
    
    func updateFile(id: UUID, content: String) -> Bool {
        let result = update_file(documentsDirectory, id.uuidString, content)
        if (result == 1) {
            return true
        }
        return false
    }
    
    func markFileForDeletion(id: UUID) -> Bool {
        let result = mark_file_for_deletion(documentsDirectory, id.uuidString)
        if (result == 1) {
            return true
        }
        return false
    }
    
    func purgeLocal() -> Bool {
        if(purge_files(documentsDirectory) == 1) {
            return true
        }
        return false
    }
}


struct FakeApi: LockbookApi {
    var fakeUsername: String = "FakeApi"
    var fakeMetadatas: [FileMetadata] = [
        FileMetadata(fileType: .Document, id: UUID(uuidString: "e956c7a2-db7f-4f9d-98c3-217847acf23a").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "first_file.md", owner: "jeff", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false),
        FileMetadata(fileType: .Document, id: UUID(uuidString: "644d1d56-8e24-4a32-8304-e906435f95db").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "another_file.md", owner: "jeff", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false),
        FileMetadata(fileType: .Document, id: UUID(uuidString: "c30a513a-0d75-4f10-ba1e-7a261ebbbe05").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "third_file.md", owner: "jeff", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false),
        FileMetadata(fileType: .Folder, id: UUID(uuidString: "cdcb3342-7373-4b11-96e9-eb25a703febb").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "nice_stuff", owner: "jeff", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false),
    ]
    
    func getAccount() -> Optional<String> {
        Optional.some(fakeUsername)
    }
    
    func createAccount(username: String) -> Bool {
        false
    }
    
    func importAccount(accountString: String) -> Bool {
        false
    }
    
    func sync() -> [FileMetadata] {
        var rander = SystemRandomNumberGenerator()
        return fakeMetadatas.shuffled(using: &rander)
    }
    
    func calculateWork() -> Result<[WorkUnit], GeneralError> {
        return Result.failure(GeneralError.init(message: "Fake api can't calculate work bub.", type: .Error))
    }
    
    func getRoot() -> UUID {
        UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped
    }
    
    func listFiles(dirId: UUID) -> [FileMetadata] {
        return sync()
    }
    
    func createFile(name: String) -> Optional<FileMetadata> {
        let now = Date().timeIntervalSince1970
        return Optional.some(FileMetadata(fileType: .Document, id: UUID(uuidString: "c30a513a-0d75-4f10-ba1e-7a261ebbbe05").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "new_file.md", owner: "jeff", contentVersion: Int(now), metadataVersion: Int(now), deleted: false))
    }
    
    func getFile(id: UUID) -> Optional<DecryptedValue> {
        Optional.none
    }
    
    func updateFile(id: UUID, content: String) -> Bool {
        false
    }
    
    func markFileForDeletion(id: UUID) -> Bool {
        false
    }
    
    func purgeLocal() -> Bool {
        false
    }
}
