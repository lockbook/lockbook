import Foundation
import SwiftLockbookCore

class SyncService: ObservableObject {
    let core: LockbookApi
    
    @Published var syncing: Bool = false
    @Published var offline: Bool = false
    @Published var updateRequired: Bool = false
    @Published var outOfSpace: Bool = false
    
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
    
    func postSyncSteps() {
        DI.files.refresh()
        DI.status.setLastSynced()
        DI.status.checkForLocalWork()
    }
    
    func sync() {
        if syncing {
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
                    self.outOfSpace = false
                    self.updateRequired = false
                    self.postSyncSteps()
                case .failure(let error):
                    switch error.kind {
                    case .UiError(let uiError):
                        switch uiError {
                        case .CouldNotReachServer:
                            self.offline = true
                        case .NoAccount:
                            print("No account yet, but tried to sync, ignoring")
                        case .ClientUpdateRequired:
                            self.updateRequired = true
                        case .OutOfSpace:
                            self.outOfSpace = true
                        }
                    default:
                        DI.errors.handleError(error)
                    }
                }
            }
        }
    }
}
