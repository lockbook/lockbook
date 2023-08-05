import Foundation
import SwiftLockbookCore

class SyncService: ObservableObject {
    let core: LockbookApi
    
    // sync status
    @Published public var syncing: Bool = false
    @Published public var isPushing: Bool? = nil
    @Published public var pushPullFileName: String? = nil
    @Published public var syncProgress: Float = 0.0
    
    // sync results
    @Published var offline: Bool = false
    @Published var upgrade: Bool = false
    
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
    
    func foregroundSync() {
        DI.files.refreshSuggestedDocs()
        sync()
    }
    
    func postSyncSteps() {
        DI.files.refresh()
        DI.status.setLastSynced()
        DI.status.checkForLocalWork()
        DI.share.calculatePendingShares()
        DI.settings.calculateUsage()
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

        print("Syncing...")
        let result = self.core.backgroundSync()
        print("Finished syncing...")
        
        syncing = false
        
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
    
    func importSync() {
        syncing = true
                
        DispatchQueue.global(qos: .userInteractive).async {
            withUnsafePointer(to: self) { syncServicePtr in
                let result = self.core.syncAll(context: syncServicePtr, updateStatus: { context, isPushing, maybeFileNamePtr, syncProgress in
                    DispatchQueue.main.sync {
                        guard let syncService = UnsafeRawPointer(context)?.load(as: SyncService.self) else {
                            return
                        }
                        
                        syncService.isPushing = isPushing
                        syncService.syncProgress = syncProgress
                        
                        if let fileNamePtr = maybeFileNamePtr {
                            syncService.pushPullFileName = String(cString: fileNamePtr)
                            syncService.core.freeText(s: fileNamePtr)
                        }
                    }
                })
                
                DispatchQueue.main.async {
                    self.syncing = false
                    self.isPushing = nil
                    self.pushPullFileName = nil
                    self.syncProgress = 0.0
                    
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
            withUnsafePointer(to: self) { syncServicePtr in
                print("Syncing...")
                let result = self.core.syncAll(context: syncServicePtr, updateStatus: { context, isPushing, maybeFileNamePtr, syncProgress in
                    DispatchQueue.main.sync {
                        guard let syncService = UnsafeRawPointer(context)?.load(as: SyncService.self) else {
                            return
                        }
                        
                        syncService.isPushing = isPushing
                        syncService.syncProgress = syncProgress
                        
                        if let fileNamePtr = maybeFileNamePtr {
                            syncService.pushPullFileName = String(cString: fileNamePtr)
                            syncService.core.freeText(s: fileNamePtr)
                        }
                    }
                })
                print("Finished syncing...")
                
                DispatchQueue.main.async {
                    self.syncing = false
                    self.isPushing = nil
                    self.pushPullFileName = nil
                    self.syncProgress = 0.0
                    
                    switch result {
                    case .success(_):
                        self.outOfSpace = false
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
                            case .UsageIsOverDataCap:
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
}
