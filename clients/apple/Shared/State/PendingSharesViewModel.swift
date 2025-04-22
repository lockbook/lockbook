import SwiftUI
import SwiftWorkspace

class PendingSharesViewModel: ObservableObject {
    @Published var pendingShares: [File]? = nil
    @Published var error: String? = nil
    
    @Published var selectSheetInfo: SelectFolderAction? = nil
    
    init() {
        self.loadPendingShares()
    }
    
    func loadPendingShares() {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.getPendingShares()
            
            DispatchQueue.main.async {
                switch res {
                case .success(let pendingShares):
                    self.pendingShares = pendingShares
                case .failure(let err):
                    self.error = err.msg
                }
            }
        }
    }
    
    func rejectShare(id: UUID) {
        switch AppState.lb.deletePendingShare(id: id) {
        case .success(_):
            self.loadPendingShares()
        case .failure(let err):
            self.error = err.msg
        }
    }
}
