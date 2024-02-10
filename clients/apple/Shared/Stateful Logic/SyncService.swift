import Foundation
import SwiftLockbookCore


// todo this should go away
class SyncService: ObservableObject {
    let core: LockbookApi
    
    // sync status
    @Published public var syncing: Bool = false
    @Published public var syncMsg: String? = nil
    @Published public var syncProgress: Float = 0.0
    
    // sync results
    @Published var offline: Bool = false
    @Published var upgrade: Bool = false
    
    @Published var outOfSpace: Bool = false
        
    init(_ core: LockbookApi) {
        self.core = core
    }
    
    func postSyncSteps() {
        DI.files.refresh()
        DI.status.setLastSynced()
        DI.status.checkForLocalWork()
        DI.share.calculatePendingShares()
        #if os(macOS)
        DI.settings.calculateUsage()
        #endif
    }
    
    func backgroundSync(onSuccess: (() -> Void)? = nil, onFailure: (() -> Void)? = nil) {
        if syncing {
            return
        }
        
        if DI.accounts.account == nil {
            print("tried to sync before having an account, ignoring")
            return
        }
        
        syncing = true

        withUnsafePointer(to: self) { syncServicePtr in
            let result = self.core.backgroundSync()
            
            DispatchQueue.main.async {
                self.cleanupSyncStatus()
                
                switch result {
                case .success(_):
                    self.outOfSpace = false
                    self.offline = false
                    self.postSyncSteps()
                    onSuccess?()
                case .failure(let error):
                    print("background sync error: \(error.message)")
                    
                    onFailure?()
                }
            }
        }
    }
    
    func cleanupSyncStatus() {
        self.syncing = false
        self.syncMsg = nil
        self.syncProgress = 0.0
    }
    
    func importSync() {
        syncing = true
                
        DispatchQueue.global(qos: .userInteractive).async {
            withUnsafePointer(to: self) { syncServicePtr in
                let result = self.core.syncAll(context: syncServicePtr, updateStatus: updateSyncStatus)
                
                DispatchQueue.main.async {
                    self.cleanupSyncStatus()
                    
                    switch result {
                    case .success(_):
                        DI.onboarding.getAccountAndFinalize()
                    case .failure(let error):
                        DI.errors.handleError(error)
                    }
                }
            }
        }
    }
}

func updateSyncStatus(context: UnsafePointer<Int8>?, cMsg: UnsafePointer<Int8>?, syncProgress: Float) -> Void {
    DispatchQueue.main.sync {
        guard let syncService = UnsafeRawPointer(context)?.load(as: SyncService.self) else {
            return
        }
        
        syncService.syncProgress = syncProgress
        
        if let cMsg = cMsg {
            syncService.syncMsg = String(cString: cMsg)
            syncService.core.freeText(s: cMsg)
        }
    }
}

