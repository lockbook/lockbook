//
//  LockbookApi.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/11/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

typealias CoreResult<T> = Result<T, CoreError>

protocol LockbookApi {
    // Account
    func getAccount() -> CoreResult<Account.Username>
    func createAccount(username: String) -> CoreResult<Account>
    func importAccount(accountString: String) -> CoreResult<Account>
    
    // Work
    func synchronize() -> CoreResult<Bool>
    func calculateWork() -> CoreResult<[WorkUnit]>
    func executeWork(work: [WorkUnit]) -> CoreResult<Bool>
    
    // Directory
    func getRoot() -> CoreResult<FileMetadata>
    func listFiles(dirId: UUID) -> CoreResult<[FileMetadata]>
    
    // Document
    func getFile(id: UUID) -> CoreResult<DecryptedValue>
    func createFile(name: String, dirId: UUID) -> CoreResult<FileMetadata>
    func updateFile(id: UUID, content: String) -> CoreResult<Bool>
    func markFileForDeletion(id: UUID) -> CoreResult<Bool>
}

struct CoreApi: LockbookApi {
    let documentsDirectory: String
    
    private func isDbPresent() -> Bool {
        is_db_present(documentsDirectory)
    }
    
    func getAccount() -> CoreResult<Account.Username> {
        return fromPrimitiveResult(result: get_account(documentsDirectory))
    }
    
    func createAccount(username: String) -> CoreResult<Account> {
        return fromPrimitiveResult(result: create_account(documentsDirectory, username))
    }
    
    func importAccount(accountString: String) -> CoreResult<Account> {
        return fromPrimitiveResult(result: import_account(documentsDirectory, accountString))
    }
    
    func synchronize() -> CoreResult<Bool> {
        return fromPrimitiveResult(result: sync_files(documentsDirectory))
    }
    
    func calculateWork() -> CoreResult<[WorkUnit]> {
        return fromPrimitiveResult(result: calculate_work(documentsDirectory))
    }
    
    func executeWork(work: [WorkUnit]) -> CoreResult<Bool> {
        return CoreResult.failure(CoreError(message: "Unimplemented!"))
    }
    
    func getRoot() -> CoreResult<FileMetadata> {
        return fromPrimitiveResult(result: get_root(documentsDirectory))
    }
    
    func listFiles(dirId: UUID) -> CoreResult<[FileMetadata]> {
        return fromPrimitiveResult(result: list_files(documentsDirectory, dirId.uuidString))
    }
    
    func getFile(id: UUID) -> CoreResult<DecryptedValue> {
        return fromPrimitiveResult(result: get_file(documentsDirectory, id.uuidString))
    }
    
    func createFile(name: String, dirId: UUID) -> CoreResult<FileMetadata> {
        return fromPrimitiveResult(result: create_file(documentsDirectory, name, dirId.uuidString))
    }
    
    func updateFile(id: UUID, content: String) -> CoreResult<Bool> {
        return fromPrimitiveResult(result: update_file(documentsDirectory, id.uuidString, content))
    }
    
    func markFileForDeletion(id: UUID) -> CoreResult<Bool> {
        return fromPrimitiveResult(result: mark_file_for_deletion(documentsDirectory, id.uuidString))
    }
}


struct FakeApi: LockbookApi {
    func getAccount() -> CoreResult<Account.Username> {
        CoreResult.success(username)
    }
    
    func createAccount(username: String) -> CoreResult<Account> {
        CoreResult.failure(CoreError.lazy())
    }
    
    func importAccount(accountString: String) -> CoreResult<Account> {
        CoreResult.failure(CoreError.lazy())
    }
    
    func synchronize() -> CoreResult<Bool> {
        CoreResult.failure(CoreError.lazy())
    }
    
    func calculateWork() -> CoreResult<[WorkUnit]> {
        return Result.failure(CoreError.init(message: "Fake api can't calculate work bub."))
    }
    
    func executeWork(work: [WorkUnit]) -> CoreResult<Bool> {
        CoreResult.failure(CoreError.lazy())
    }
    
    func getRoot() -> CoreResult<FileMetadata> {
        return CoreResult.success(FileMetadata(fileType: .Folder, id: rootUuid, parent: rootUuid, name: "first_file.md", owner: "root", contentVersion: 1587384000000, metadataVersion: 1587384000000, deleted: false))
    }
    
    func listFiles(dirId: UUID) -> CoreResult<[FileMetadata]> {
        return Result.success(fileMetas)
    }
    
    func getFile(id: UUID) -> CoreResult<DecryptedValue> {
        CoreResult.failure(CoreError.lazy())
    }
    
    func createFile(name: String, dirId: UUID) -> CoreResult<FileMetadata> {
        let now = Date().timeIntervalSince1970
        return CoreResult.success(FileMetadata(fileType: .Document, id: UUID(uuidString: "c30a513a-0d75-4f10-ba1e-7a261ebbbe05").unsafelyUnwrapped, parent: dirId, name: "new_file.md", owner: username, contentVersion: Int(now), metadataVersion: Int(now), deleted: false))
    }
    
    func updateFile(id: UUID, content: String) -> CoreResult<Bool> {
        CoreResult.failure(CoreError.lazy())
    }
    
    func markFileForDeletion(id: UUID) -> CoreResult<Bool> {
        CoreResult.failure(CoreError.lazy())
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
