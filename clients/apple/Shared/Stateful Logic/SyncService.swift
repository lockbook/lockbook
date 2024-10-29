import Foundation
import SwiftWorkspace

// todo this should go away
class SyncService: ObservableObject {
    let core: Lb
    
    // sync status
    @Published public var syncing: Bool = false
    @Published public var syncMsg: String? = nil
    @Published public var syncProgress: Float = 0.0
    
    // sync results
    @Published var offline: Bool = false
    @Published var upgrade: Bool = false
    
    @Published var outOfSpace: Bool = false
        
    init(_ core: Lb) {
        self.core = core
    }
    
    func postSyncSteps() {
        DI.files.refresh()
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
        
        let result = self.core.sync(updateStatus: nil)
        
        DispatchQueue.main.async {
            self.cleanupSyncStatus()
            
            switch result {
            case .success(_):
                self.outOfSpace = false
                self.offline = false
                self.postSyncSteps()
                onSuccess?()
            case .failure(let error):
                print("background sync error: \(error.msg)")
                
                onFailure?()
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
            let result = self.core.sync(updateStatus: updateStatus)
            
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

func updateStatus(total: UInt, progress: UInt, id: UUID, msg: String) {
    DI.sync.syncProgress = Float(total) / Float(progress)
    DI.sync.syncMsg = msg
}

