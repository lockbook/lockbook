//
//  Coordinator.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/19/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation
import SwiftUI
import SwiftLockbookCore

final class Coordinator: ObservableObject {
    private var syncTimer: Timer
    var lockbookApi: LockbookApi // be weary of using this outside this class
    var root: FileMetadata
    var account: Account
    var currentId: UUID
    @Published var files: [FileMetadata]
    @Published var currentView: PushedItem?
    @Published var progress: Optional<(Float, String, Color)>
    let defaults = UserDefaults.standard
    @Published var autoSync: Bool
    @Published var incrementalAutoSync: Bool

    /// Fake coordinator, for use in previews!
    init() {
        self.syncTimer = Timer()
        let api = FakeApi()
        self.lockbookApi = api
        self.root = (try? api.getRoot().get())!
        self.currentId = self.root.id
        self.account = Account(username: "tester")
        self.files = (try? api.listFiles().get())!
        self.progress = Optional.some((0.5, "Something", Color.blue))
        self.autoSync = true
        self.incrementalAutoSync = false
    }
    
    init(lockbookApi: LockbookApi, account: Account) throws {
        self.syncTimer = Timer()
        self.lockbookApi = lockbookApi
        self.root = try self.lockbookApi.getRoot().get()
        self.currentId = self.root.id
        self.account = account
        self.files = try self.lockbookApi.listFiles().get()
        self.progress = Optional.none
        self.autoSync = self.defaults.bool(forKey: "AutoSync")
        self.incrementalAutoSync = self.defaults.bool(forKey: "IncrementalAutoSync")
        self.syncTimer = Timer.scheduledTimer(withTimeInterval: 30.0, repeats: true, block: { (Timer) in
            if (self.autoSync) {
                if (self.incrementalAutoSync) {
                    self.fullIncrementalSync()
                } else {
                    self.sync()
                }
            } else {
                print("Auto-sync Disabled")
            }
        })
    }
    
    /// Retrieves file metadata from core and replaces the current metadatas
    func reloadFiles() -> Void {
        if case .success(let files) = self.lockbookApi.listFiles() {
            self.files = files
        }
    }
    
    /// Does a brute full-sync
    func sync() -> Void {
        switch self.lockbookApi.synchronize() {
        case .success(_):
            self.reloadFiles()
        case .failure(let err):
            print("Sync failed with error: \(err)")
        }
    }
    
    /// Calculates work and executes the first work unit
    func incrementalSync() -> Void  {
        if case .success(let workMeta) = self.lockbookApi.calculateWork() {
            if let wu = workMeta.workUnits.first {
                self.incrementAndExecute(work: ArraySlice([wu]), processed: 0, total: 1)
            }
        }
    }
    
    private func incrementAndExecute(work: ArraySlice<WorkUnit>, processed: Int, total: Int) -> Void {
        let progress = Float(processed) / Float(total)
        if let wu = work.first {
            let tail = work.dropFirst()
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                print(progress, processed, total)
                self.progress = Optional.some((progress, "Processing \(wu.type()) Change \(wu.get().name)", Color.gray))
                switch self.lockbookApi.executeWork(work: wu) {
                case .success(_):
                    let _ = self.lockbookApi.setLastSynced(lastSync: UInt64(wu.get().metadataVersion))
                    self.progress = Optional.some(((Float(processed+1)/Float(total)), "Processed \(wu.type()) Change \(wu.get().name)", Color.blue))
                    self.incrementAndExecute(work: tail, processed: processed+1, total: total)
                case .failure(let err):
                    print(err)
                    self.progress = Optional.some(((Float(processed+1)/Float(total)), "Failed \(wu.type()) Change \(wu.get().name)", Color.red))
                }
                self.reloadFiles()
            }
        } else {
            self.progress = Optional.some((progress, "Synced \(processed) files!", Color.green))
            DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
                self.progress = Optional.none
            }
        }
    }
    
    /// Calculates work and executes every work unit (great to plug a hook for a progress bar or something)
    func fullIncrementalSync() -> Void {
        switch self.lockbookApi.calculateWork() {
        case .success(let workMeta):
            if (workMeta.workUnits.count > 0) {
                self.progress = Optional.some((Float(0.0), "Syncing", Color.yellow))
                self.incrementAndExecute(work: ArraySlice(workMeta.workUnits), processed: 0, total: workMeta.workUnits.count)
            }
        case .failure(let err):
            self.progress = Optional.some((Float(0.0), "Err: \(err)", Color.red))
        }
    }
    
    func getRoot() -> FileMetadata? {
        switch self.lockbookApi.getRoot() {
        case .success(let rootMeta):
            return Optional.some(rootMeta)
        case .failure(let err):
            print("Failed getting root with error: \(err)")
            return Optional.none
        }
    }
    
    func navigateAndListFiles(dirId: UUID) -> [FileMetadata] {
        self.currentId = dirId
        switch (self.lockbookApi.listFiles()) {
        case .success(let files):
            return files.filter { $0.parent == dirId && $0.id != dirId }
        case .failure(let err):
            print("List files failed with error: \(err)")
            return []
        }
    }
    
    func createFile(name: String, isFolder: Bool) -> Bool {
        switch self.lockbookApi.createFile(name: name, dirId: currentId, isFolder: isFolder) {
        case .success(_):
            self.reloadFiles()
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
    
    func toggleAutoSync() -> Void {
        self.autoSync = !self.autoSync
        self.defaults.set(self.autoSync, forKey: "AutoSync")
    }
    
    func toggleIncrementalAutoSync() -> Void {
        self.incrementalAutoSync = !self.incrementalAutoSync
        self.defaults.set(self.incrementalAutoSync, forKey: "IncrementalAutoSync")
    }
    
    
    enum PushedItem {
        case welcomeView
        case fileBrowserView
        case debugView
    }
}
