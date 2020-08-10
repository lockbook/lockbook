//
//  LockbookApi.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/11/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

typealias CoreResult<T> = Result<T, ApplicationError>

protocol LockbookApi {
    // Account
    func getAccount() -> CoreResult<Account>
    func createAccount(username: String) -> CoreResult<Account>
    func importAccount(accountString: String) -> CoreResult<Account>
    func exportAccount() -> CoreResult<String>
    
    // Work
    func synchronize() -> CoreResult<Empty>
    func calculateWork() -> CoreResult<WorkMetadata>
    func executeWork(work: WorkUnit) -> CoreResult<Empty>
    func setLastSynced(lastSync: UInt64) -> CoreResult<Empty>
    
    // Directory
    func getRoot() -> CoreResult<FileMetadata>
    func listFiles() -> CoreResult<[FileMetadata]>
    
    // Document
    func getFile(id: UUID) -> CoreResult<DecryptedValue>
    func createFile(name: String, dirId: UUID, isFolder: Bool) -> CoreResult<FileMetadata>
    func updateFile(id: UUID, content: String) -> CoreResult<Bool>
    func markFileForDeletion(id: UUID) -> CoreResult<Bool>
}

struct CoreApi: LockbookApi {
    var documentsDirectory: String
    
    func getAccount() -> CoreResult<Account> {
        fromPrimitiveResult(result: get_account(documentsDirectory))
    }
    
    func createAccount(username: String) -> CoreResult<Account> {
        let result: Result<Empty, ApplicationError> = fromPrimitiveResult(result: create_account(documentsDirectory, username))
        
        return result.flatMap { print($0 as Any); return getAccount() }
    }
    
    func importAccount(accountString: String) -> CoreResult<Account> {
        let result: Result<Empty, ApplicationError> = fromPrimitiveResult(result: import_account(documentsDirectory, accountString.trimmingCharacters(in: .whitespacesAndNewlines)))
        
        return result.flatMap { print($0 as Any); return getAccount() }
    }
    
    func exportAccount() -> CoreResult<String> {
        fromPrimitiveResult(result: export_account(documentsDirectory))
    }
    
    func synchronize() -> CoreResult<Empty> {
        fromPrimitiveResult(result: sync_all(documentsDirectory))
    }
    
    func calculateWork() -> CoreResult<WorkMetadata> {
        fromPrimitiveResult(result: calculate_work(documentsDirectory))
    }
    
    func executeWork(work: WorkUnit) -> CoreResult<Empty> {
        switch serialize(obj: work) {
        case .success(let workUnitStr):
            return fromPrimitiveResult(result: execute_work(documentsDirectory, workUnitStr))
        case .failure(let err):
            return CoreResult.failure(ApplicationError.General(err))
        }
    }
    
    func setLastSynced(lastSync: UInt64) -> CoreResult<Empty> {
        fromPrimitiveResult(result: set_last_synced(documentsDirectory, lastSync))
    }
    
    func getRoot() -> CoreResult<FileMetadata> {
        fromPrimitiveResult(result: get_root(documentsDirectory))
    }
    
    func listFiles() -> CoreResult<[FileMetadata]> {
        fromPrimitiveResult(result: list_files(documentsDirectory))
    }
    
    func getFile(id: UUID) -> CoreResult<DecryptedValue> {
        fromPrimitiveResult(result: get_file_by_path(documentsDirectory, id.uuidString))
    }
    
    func createFile(name: String, dirId: UUID, isFolder: Bool) -> CoreResult<FileMetadata> {
        fromPrimitiveResult(result: create_file(documentsDirectory, name, dirId.uuidString, "Document"))
    }
    
    func updateFile(id: UUID, content: String) -> CoreResult<Bool> {
        fromPrimitiveResult(result: write_document(documentsDirectory, id.uuidString, content))
    }
    
    func markFileForDeletion(id: UUID) -> CoreResult<Bool> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
}


struct FakeApi: LockbookApi {
    func getAccount() -> CoreResult<Account> {
        CoreResult.success(Account(username: username))
    }
    
    func createAccount(username: String) -> CoreResult<Account> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    func importAccount(accountString: String) -> CoreResult<Account> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    func exportAccount() -> CoreResult<String> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    func synchronize() -> CoreResult<Empty> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    func calculateWork() -> CoreResult<WorkMetadata> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    func executeWork(work: WorkUnit) -> CoreResult<Empty> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    func setLastSynced(lastSync: UInt64) -> CoreResult<Empty> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    func getRoot() -> CoreResult<FileMetadata> {
        return CoreResult.success(FileMetadata(fileType: .Folder, id: rootUuid, parent: rootUuid, name: "first_file.md", owner: "root", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false))
    }
    
    func listFiles() -> CoreResult<[FileMetadata]> {
        return Result.success(fileMetas)
    }
    
    func getFile(id: UUID) -> CoreResult<DecryptedValue> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    func createFile(name: String, dirId: UUID, isFolder: Bool) -> CoreResult<FileMetadata> {
        let now = Date().timeIntervalSince1970
        return CoreResult.success(FileMetadata(fileType: .Document, id: UUID(uuidString: "c30a513a-0d75-4f10-ba1e-7a261ebbbe05").unsafelyUnwrapped, parent: dirId, name: "new_file.md", owner: username, contentVersion: Int(now), metadataVersion: Int(now), deleted: false))
    }
    
    func updateFile(id: UUID, content: String) -> CoreResult<Bool> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    func markFileForDeletion(id: UUID) -> CoreResult<Bool> {
        CoreResult.failure(ApplicationError.Lockbook(CoreError.lazy()))
    }
    
    let username: Account.Username = "tester"
    let rootUuid = UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped
    let fileMetas = [
        FileMetadata(fileType: .Document, id: UUID(uuidString: "e956c7a2-db7f-4f9d-98c3-217847acf23a").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "first_file.md", owner: "jeff", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false),
        FileMetadata(fileType: .Document, id: UUID(uuidString: "644d1d56-8e24-4a32-8304-e906435f95db").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "another_file.md", owner: "jeff", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false),
        FileMetadata(fileType: .Document, id: UUID(uuidString: "c30a513a-0d75-4f10-ba1e-7a261ebbbe05").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "third_file.md", owner: "jeff", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false),
        FileMetadata(fileType: .Folder, id: UUID(uuidString: "cdcb3342-7373-4b11-96e9-eb25a703febb").unsafelyUnwrapped, parent: UUID(uuidString: "aa9c473b-79d3-4d11-b6c7-7c82d6fb94cc").unsafelyUnwrapped, name: "nice_stuff", owner: "jeff", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false),
    ]
}
