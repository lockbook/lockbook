import SwiftWorkspace
import SwiftUI

struct ShareInfo {
    var writeAccessUsers: [String]
    var readAccessUsers: [String]
}

class ShareService: ObservableObject {
    
    let core: Lb
    
    @Published var pendingShares: [File]? = nil
    @Published var id: UUID? = nil
    @Published var shareInfo: ShareInfo? = nil
    
    var showPendingSharesView: Bool = false
    
    init(_ core: Lb) {
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
                DI.errors.showError(err)
            }
        }
    }
    
    func rejectShare(id: UUID) {
        if case .failure(let err) = core.deletePendingShare(id: id) {
            DI.errors.showError(err)
        }
        
        calculatePendingShares()
    }
    
    func calculateShareInfo(id: UUID) {
        let maybeMeta = DI.files.idsAndFiles[id]
        
        if let meta = maybeMeta {
            var writeAccessUsers: [String] = []
            var readAccessUsers: [String] = []
            
            meta.shares.forEach { share in
                switch share.mode {
                case .read:
                    readAccessUsers.append(share.with)
                case .write:
                    writeAccessUsers.append(share.with)
                }
            }
            
            shareInfo = ShareInfo(writeAccessUsers: writeAccessUsers, readAccessUsers: readAccessUsers)
            self.id = id
        }
    }
}
