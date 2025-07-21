import SwiftUI
import SwiftWorkspace
import Combine

struct PendingSharesIcon: View {
    @StateObject var model: PendingSharesIconViewModel
    
    let alertOffsetX: CGFloat = 10
    let alertOffsetY: CGFloat = 5
    
    #if os(iOS)
    let textOffsetX: CGFloat = 0.3
    let textOffsetY: CGFloat = 0.2
    #else
    let textOffsetX: CGFloat = 0.3
    let textOffsetY: CGFloat = 0.5
    #endif
    
    init(homeState: HomeState) {
        self._model = StateObject(wrappedValue: PendingSharesIconViewModel(homeState: homeState))
    }
    
    var body: some View {
        if let pendingSharesCount = model.pendingSharesCount {
            ZStack {
                Image(systemName: "person.2.fill")
                    .foregroundColor(.accentColor)
                
                if pendingSharesCount > 0 {
                    Circle()
                        .foregroundColor(.red)
                        .frame(width: 10, height: 10)
                        .offset(x: alertOffsetX, y: alertOffsetY)
                    
                    if pendingSharesCount < 10 {
                        Text(String(pendingSharesCount))
                            .foregroundStyle(.white)
                            .offset(x: alertOffsetX + textOffsetX, y: alertOffsetY + textOffsetY)
                            .font(.custom("pending shares", fixedSize: 10))
                    }
                }
            }
        } else {
            ProgressView()
        }
    }
}

class PendingSharesIconViewModel: ObservableObject {
    @Published var pendingSharesCount: Int? = nil
    
    private var cancellables: Set<AnyCancellable> = []
    private var homeState: HomeState
        
    init(homeState: HomeState) {
        self.homeState = homeState
        self.loadPendingSharesCount()
        
        AppState.lb.events.$metadataUpdated.sink { [weak self] pendingShares in
            self?.loadPendingSharesCount()
        }
        .store(in: &cancellables)
    }
    
    func loadPendingSharesCount() {
        DispatchQueue.global(qos: .userInteractive).async {
            let res = AppState.lb.getPendingShares()
            
            DispatchQueue.main.async {
                switch res {
                case .success(let shares):
                    self.pendingSharesCount = shares.count
                case .failure(let err):
                    AppState.shared.error = .lb(error: err)
                }
            }
        }
    }
}

#Preview("Pending Shares") {
    PendingSharesIcon(homeState: HomeState())
        .frame(width: 200, height: 200)
}
