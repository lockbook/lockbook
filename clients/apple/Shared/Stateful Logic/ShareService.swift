import SwiftLockbookCore
import SwiftUI

struct ShareInfo {
    var writeAccessUsers: [String]
    var readAccessUsers: [String]
}

class ShareService: ObservableObject {
    
    let core: LockbookApi
    
    @Published var pendingShares: [File] = []
    @Published var shareInfos: [File : ShareInfo] = [:]
    
    init(_ core: LockbookApi) {
        self.core = core
    }
    
    func calculatePendingShares() {
        switch core.getPendingShares() {
        case .success(let shares):
            pendingShares = shares
        case .failure(let err):
            DI.errors.handleError(err)
        }
    }
    
    func rejectShare(id: UUID) {
        if case .failure(let err) = core.deletePendingShare(id: id) {
            DI.errors.handleError(err)
        }
        
        calculatePendingShares()
    }
    
    func acceptShare(targetMeta: File, parent: UUID) {
        if case .failure(let err) = core.createLink(name: targetMeta.name, dirId: parent, target: targetMeta.id) {
            DI.errors.handleError(err)
        }
    }
    
    func shareFile(id: UUID, username: String, isWrite: Bool) {
        DispatchQueue.global(qos: .userInitiated).async {
            let operation = self.core.shareFile(id: id, username: username, isWrite: isWrite)

            DispatchQueue.main.async {
                switch operation {
                case .success(_):
                    DI.sync.sync()
                case .failure(let error):
                    DI.errors.handleError(error)
                }
            }
        }
    }
    
    func calculateShareInfo(file: File) {
        var writeAccessUsers: [String] = []
        var readAccessUsers: [String] = []
        
        file.shares.forEach { share in
            switch share.mode {
            case .Read:
                readAccessUsers.append(share.sharedWith)
            case .Write:
                writeAccessUsers.append(share.sharedWith)
            }
        }
        
        shareInfos[file] = ShareInfo.init(writeAccessUsers: writeAccessUsers, readAccessUsers: readAccessUsers)
    }
}
