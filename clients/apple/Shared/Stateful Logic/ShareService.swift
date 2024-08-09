import SwiftLockbookCore
import SwiftUI

struct ShareInfo {
    var writeAccessUsers: [String]
    var readAccessUsers: [String]
}

class ShareService: ObservableObject {
    
    let core: LockbookApi
    
    @Published var pendingShares: [File]? = nil
    @Published var id: UUID? = nil
    @Published var shareInfo: ShareInfo? = nil
    
    var showPendingSharesView: Bool = false
    
    init(_ core: LockbookApi) {
        self.core = core
        
        calculatePendingShares()
    }
    
    func calculatePendingShares() {
        if DI.accounts.account == nil {
            print("No account yet, but tried to update last synced, ignoring")
            return
        }
        
        DispatchQueue.global(qos: .userInitiated).async {
            switch self.core.getPendingShares() {
            case .success(let shares):
                DispatchQueue.main.async {
                    self.pendingShares = shares
                }
            case .failure(let err):
                DI.errors.handleError(err)
            }
        }
    }
    
    func rejectShare(id: UUID) {
        if case .failure(let err) = core.deletePendingShare(id: id) {
            DI.errors.handleError(err)
        }
        
        calculatePendingShares()
    }
    
    func shareFile(id: UUID, username: String, isWrite: Bool) {
        if case .failure(let err) = core.shareFile(id: id, username: username, isWrite: isWrite) {
            DI.errors.handleError(err)
        }
    }
    
    func calculateShareInfo(id: UUID) {
        let maybeMeta = DI.files.idsAndFiles[id]
        
        if let meta = maybeMeta {
            var writeAccessUsers: [String] = []
            var readAccessUsers: [String] = []
            
            meta.shares.forEach { share in
                switch share.mode {
                case .Read:
                    readAccessUsers.append(share.sharedWith)
                case .Write:
                    writeAccessUsers.append(share.sharedWith)
                }
            }
            
            shareInfo = ShareInfo(writeAccessUsers: writeAccessUsers, readAccessUsers: readAccessUsers)
            self.id = id
        }
    }
}
