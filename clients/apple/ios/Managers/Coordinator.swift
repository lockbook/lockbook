//
//  Coordinator.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/19/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

final class Coordinator: ObservableObject {
    private var syncTimer: Timer
    private var lockbookApi: LockbookApi
    var root: FileMetadata
    var account: Account
    var currentId: UUID
    @Published var files: [FileMetadata]
    @Published var currentView: PushedItem?
    @Published var progress: Optional<(Float, String)>

    init() {
        self.syncTimer = Timer()
        let api = FakeApi()
        self.lockbookApi = api
        self.root = (try? api.getRoot().get())!
        self.currentId = self.root.id
        self.account = Account(username: "tester")
        self.files = (try? api.listFiles().get())!
        self.progress = Optional.some((0.0, "Something"))
    }
    
    init(lockbookApi: LockbookApi, account: Account) throws {
        self.syncTimer = Timer()
        self.lockbookApi = lockbookApi
        self.root = try self.lockbookApi.getRoot().get()
        self.currentId = self.root.id
        self.account = account
        self.files = try self.lockbookApi.listFiles().get()
        self.progress = Optional.none
        self.syncTimer = Timer.scheduledTimer(withTimeInterval: 30.0, repeats: true, block: { (Timer) in
            self.sync()
        })
    }
    
    func sync() -> Void {
        let result = self.lockbookApi.synchronize().flatMap { (_) -> CoreResult<[FileMetadata]> in
            self.lockbookApi.listFiles()
        }
        switch result {
        case .success(let newFiles):
            self.files = newFiles
        case .failure(let err):
            print("Sync failed with error: \(err)")
        }
    }
    
    func getRoot() -> UUID? {
        switch self.lockbookApi.getRoot() {
        case .success(let rootMeta):
            return Optional.some(rootMeta.id)
        case .failure(let err):
            print("Failed getting root with error: \(err)")
            return Optional.none
        }
    }
    
    func navigateAndListFiles(dirId: UUID) -> [FileMetadata] {
        self.currentId = dirId
        switch (self.lockbookApi.listFiles()) {
        case .success(let files):
            return files
        case .failure(let err):
            print("List files failed with error: \(err)")
            return []
        }
    }
    
    func createFile(name: String, isFolder: Bool) -> Bool {
        switch self.lockbookApi.createFile(name: name, dirId: currentId, isFolder: isFolder) {
        case .success(_):
            return true
        case .failure(let err):
            print("Create file failed with error: \(err)")
            return false
        }
    }
    
    func getFile(meta: FileMetadata) -> Optional<DecryptedValue> {
        switch self.lockbookApi.getFile(id: meta.id) {
        case .success(let file):
            return Optional.some(file)
        case .failure(let err):
            print("Get file failed with error: \(err)")
            return Optional.none
        }
    }
    
    func updateFile(id: UUID, content: String) -> Bool {
        switch self.lockbookApi.updateFile(id: id, content: content) {
        case .success(_):
            return true
        case .failure(let err):
            print("Get file failed with error: \(err)")
            return false
        }
    }
    
    func markFileForDeletion(id: UUID) -> Void {
        let _ = self.lockbookApi.markFileForDeletion(id: id)
    }
    
    enum PushedItem {
        case welcomeView
        case fileBrowserView
        case debugView
    }
}
