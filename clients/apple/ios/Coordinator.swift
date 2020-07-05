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
    private var lockbookApi: LockbookApi
    private var timer: Timer
    
    
    init() {
        self.timer = Timer()
        self.lockbookApi = FakeApi()
        self._files = Published.init(initialValue: self.lockbookApi.sync())
    }
    
    init(lockbookApi: LockbookApi) {
        self.timer = Timer()
        self.lockbookApi = lockbookApi
        self._files = Published.init(initialValue: lockbookApi.sync())
        if let username = lockbookApi.getAccount() {
            self.username = username
        }
        self.timer = Timer.scheduledTimer(withTimeInterval: 30.0, repeats: true, block: { (Timer) in
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
    
    func getFile(id: UUID) -> Optional<DecryptedValue> {
        return self.lockbookApi.getFile(id: id)
    }
    
    func updateFile(id: UUID, content: String) -> Bool {
        return self.lockbookApi.updateFile(id: id, content: content)
    }
    
    func markFileForDeletion(id: UUID) -> Void {
        let _ = self.lockbookApi.markFileForDeletion(id: id)
    }
    
    enum PushedItem {
        case welcomeView
        case listView
        case debugView
    }
}
