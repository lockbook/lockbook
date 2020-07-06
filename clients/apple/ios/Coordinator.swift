//
//  Coordinator.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/19/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

final class Coordinator: ObservableObject {
    @Published var currentView: PushedItem?
    @Published var files: [FileMetadata]
    @Published var username: String = "NOUSER"
    @Published var progress: Optional<(Float, String)>
    private var lockbookApi: LockbookApi
    private var syncTimer: Timer
    
    
    init() {
        self.syncTimer = Timer()
        self.lockbookApi = FakeApi()
        self._files = Published.init(initialValue: self.lockbookApi.sync())
        self._progress = Published.init(initialValue: Optional.some((0.0, "Something")))
    }
    
    init(lockbookApi: LockbookApi) {
        self.syncTimer = Timer()
        self.lockbookApi = lockbookApi
        self._files = Published.init(initialValue: lockbookApi.sync())
        self._progress = Published.init(initialValue: Optional.none)
        if let username = lockbookApi.getAccount() {
            self.username = username
        }
        self.syncTimer = Timer.scheduledTimer(withTimeInterval: 30.0, repeats: true, block: { (Timer) in
            self.files = lockbookApi.sync()
        })
    }
    
    func sync() -> Void {
        self.files = self.lockbookApi.sync()
    }
    
    func createAccount(username: String) -> Bool {
        if self.lockbookApi.createAccount(username: username) {
            self.username = username
            return true
        }
        return false
    }
    
    func importAccount(accountString: String) -> Bool {
        if self.lockbookApi.importAccount(accountString: accountString) {
            if let username = self.lockbookApi.getAccount() {
                self.username = username
                return true
            }
        }
        return false
    }
    
    func getRoot() -> UUID {
        self.lockbookApi.getRoot()
    }
    
    func listFiles(dirId: UUID) -> [FileMetadata] {
        self.lockbookApi.listFiles(dirId: dirId)
    }
    
    func createFile(name: String) -> Bool {
        if let file = self.lockbookApi.createFile(name: name) {
            print("File created \(file)")
            return true
        }
        return false
    }
    
    func getFile(meta: FileMetadata) -> Optional<DecryptedValue> {
        return self.lockbookApi.getFile(id: meta.id)
    }
    
    func updateFile(id: UUID, content: String) -> Bool {
        return self.lockbookApi.updateFile(id: id, content: content)
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
