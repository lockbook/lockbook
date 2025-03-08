import SwiftUI
import SwiftWorkspace

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
                        .frame(width: 12, height: 12)
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
    
    init(homeState: HomeState) {
        DispatchQueue.global(qos: .userInteractive).async {
            let res = AppState.lb.getPendingShares()
            
            DispatchQueue.main.async {
                switch res {
                case .success(let shares):
                    self.pendingSharesCount = shares.count
                case .failure(let err):
                    homeState.error = .lb(error: err)
                }
            }
        }
    }
}

#Preview("Pending Shares") {
    PendingSharesIcon(homeState: HomeState())
        .frame(width: 200, height: 200)
}
