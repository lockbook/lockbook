import Foundation
import SwiftLockbookCore

class StatusService: ObservableObject {
    let core: LockbookApi
    
    @Published var work: Int = 0
    @Published var lastSynced: String = ""
    
    private var lastSyncedTimer: Timer? = nil
    
    init(_ core: LockbookApi) {
        self.core = core
        
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
                    DI.errors.handleError(err)
                }
            }
        }
    }
}
