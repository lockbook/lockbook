import Foundation
import SwiftLockbookCore

class SyncService: ObservableObject {
    let core: LockbookApi
    
    @Published var syncing: Bool = false
    @Published var offline: Bool = false
    @Published var upgrade: Bool = false
    
    private var syncTimer: Timer? = nil
    
    init(_ core: LockbookApi) {
        self.core = core
        
        startOrRestartSyncTimer()
    }
    
    func startOrRestartSyncTimer() {
        syncTimer?.invalidate()
        syncTimer = Timer.scheduledTimer(timeInterval: 30*60, target: self, selector: #selector(syncTimerTick), userInfo: nil, repeats: true)
    }
    
    @objc func syncTimerTick() {
        sync()
    }
    
    func documentChangeHappened() {
        startOrRestartSyncTimer()
        DI.status.checkForLocalWork()
    }
    
    func foregroundSync() {
        DI.files.refreshSuggestedDocs()
        sync()
    }
    
    func postSyncSteps() {
        DI.files.refresh()
        DI.status.setLastSynced()
        DI.status.checkForLocalWork()
    }
    
    func sync() {
        if syncing {
            return
        }
        
        if DI.accounts.account == nil {
            print("tried to sync before having an account, ignoring")
            return
        }
        
        syncing = true
        
        DispatchQueue.global(qos: .userInteractive).async {
            let result = self.core.syncAll()
                        
                DispatchQueue.main.async {
                    self.syncing = false
                    
                    switch result {
                    case .success(_):
                        self.offline = false
                        self.postSyncSteps()
                    case .failure(let error):
                        switch error.kind {
                        case .UiError(let uiError):
                            switch uiError {
                            case .CouldNotReachServer:
                                self.offline = true
                            case .ClientUpdateRequired:
                                self.upgrade = true
                            case .Retry:
                                // TODO
                                DI.errors.handleError(ErrorWithTitle(title: "Retry", message: "SyncService wants retry"))
                            case .UsageIsOverFreeTierDataCap:
                                DI.errors.handleError(ErrorWithTitle(title: "Usage", message: "Your total data usage exceeds free tier. Please remove some data."))
                            }
                        default:
                            DI.errors.handleError(error)
                        }
                    }
                }
        }
    }
}
