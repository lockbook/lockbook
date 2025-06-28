import SwiftUI
import SwiftWorkspace
import Combine

class PendingSharesViewModel: ObservableObject {
    @Published var pendingShares: [File]? = nil
    @Published var error: String? = nil
    
    private var cancellables: Set<AnyCancellable> = []

        
    init() {
        self.loadPendingShares()
        
        AppState.lb.events.$pendingShares.sink { [weak self] pendingShares in
            self?.loadPendingShares()
        }
        .store(in: &cancellables)
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
