import SwiftUI
import SwiftWorkspace
import Combine

struct PendingSharesIcon: View {
    @StateObject var model: PendingSharesIconViewModel
    
    #if os(iOS)
    let textSize: CGFloat = 10
    let circleSize: CGFloat = 10
    let alertOffsetX: CGFloat = 10
    let alertOffsetY: CGFloat = 5
    let controlSize = ControlSize.regular
    #else
    let textSize: CGFloat = 9
    let circleSize: CGFloat = 9
    let alertOffsetX: CGFloat = 8
    let alertOffsetY: CGFloat = 3
    let controlSize = ControlSize.small
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
                    Text(pendingSharesCount < 10 ? String(pendingSharesCount) : "")
                        .foregroundStyle(.white)
                        .font(.custom("PendingShares", fixedSize: textSize))
                        .background(
                            Circle()
                                .foregroundColor(.red)
                                .frame(width: circleSize, height: circleSize)
                        )
                        .offset(x: alertOffsetX, y: alertOffsetY)
                }
            }
        } else {
            ProgressView()
                .controlSize(controlSize)
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
