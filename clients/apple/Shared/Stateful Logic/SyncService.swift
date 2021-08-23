import Foundation
import SwiftLockbookCore

class SyncService: ObservableObject {
    let core: LockbookApi
    let files: FileService
    let status: StatusService
    let errors: UnexpectedErrorService
    
    @Published var syncing: Bool = false
    @Published var offline: Bool = false
    
    private var syncTimer: Timer? = nil
    
    init(_ core: LockbookApi, _ files: FileService, _ status: StatusService, _ errors: UnexpectedErrorService) {
        self.core = core
        self.files = files
        self.status = status
        self.errors = errors
        
        self.files.openDrawing.writeListener = documentChangeHappened
        self.files.openDocument.writeListener = documentChangeHappened
        
        startOrRestartSyncTimer()
    }
    
    func startOrRestartSyncTimer() {
        syncTimer?.invalidate()
        syncTimer = Timer.scheduledTimer(timeInterval: 30*60, target: self, selector: #selector(syncTimerTick), userInfo: nil, repeats: true)
    }

    @objc func syncTimerTick() {
        syncing = true
    }
    
    func documentChangeHappened() {
        startOrRestartSyncTimer()
        status.checkForLocalWork()
    }
    
    func postSyncSteps() {
        files.refresh()
        status.setLastSynced()
        status.checkForLocalWork()
    }
    
    func sync() {
        if syncing {
            return
        }
        
        syncing = true
        
        DispatchQueue.global(qos: .userInteractive).async {
            let result = self.core.syncAll()
            
            DispatchQueue.main.async {
                
                switch result {
                case .success(_):
                    self.syncing = false
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
                            print("Upgrade required but not shown to user, ignoring (TODO)") // TODO
                        }
                    default:
                        self.errors.handleError(error)
                    }
                }
            }
        }
    }
}
