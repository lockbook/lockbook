import Foundation
import SwiftLockbookCore

class StatusService: ObservableObject {
    let core: LockbookApi
    let account: AccountService
    let errors: UnexpectedErrorService
    
    @Published var work: Int = 0
    @Published var lastSynced: String = ""
    
    private var lastSyncedTimer: Timer? = nil
    
    init(_ core: LockbookApi, _ account: AccountService, _ errors: UnexpectedErrorService) {
        self.core = core
        self.account = account
        self.errors = errors
        
        startLastSyncedTimer()
    }
    
    func startLastSyncedTimer() {
        lastSyncedTimer = Timer.scheduledTimer(timeInterval: 60, target: self, selector: #selector(setLastSynced), userInfo: nil, repeats: true)
    }
    
    func checkForLocalWork() {
        DispatchQueue.global(qos: .userInitiated).async {
            let localChanges = self.core.getLocalChanges()
            DispatchQueue.main.async {
                switch localChanges {
                case .success(let local):
                    self.work = local.count
                case .failure(let err):
                    self.errors.handleError(err)
                }
            }
        }
    }
    
    
    @objc func setLastSynced() {
        DispatchQueue.global(qos: .userInteractive).async {
            if self.account.getAccount() == nil {
                print("No account yet, but tried to update last synced, ignoring")
                return
            }
            
            let lastSynced = self.core.getLastSyncedHumanString()
            
            DispatchQueue.main.async {
                switch lastSynced {
                case .success(let lastSynced):
                    self.lastSynced = lastSynced
                case .failure(let error):
                    self.errors.handleError(error)
                }
            }
        }
    }
}
