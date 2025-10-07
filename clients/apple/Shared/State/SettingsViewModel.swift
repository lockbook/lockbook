import SwiftUI
import SwiftWorkspace

class SettingsViewModel: ObservableObject {
    @Published var account: Account? = nil
    @Published var usage: UsageMetrics? = nil
    @Published var isPremium: Bool? = nil
    
    init(initalUsageComputation: Bool = true) {
        self.loadAccount()
        self.loadTier()
        
        if initalUsageComputation {
            self.loadUsages()
        }
    }
    
    func loadAccount() {
        switch AppState.lb.getAccount() {
        case .success(let account):
            self.account = account
        case .failure(let err):
            if err.code != .serverUnreachable {
                AppState.shared.error = .lb(error: err)
            }
        }
    }
    
    func loadTier() {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.getSubscriptionInfo()
            
            DispatchQueue.main.async {
                switch res {
                case .success(let info):
                    self.isPremium = info?.isPremium() ?? false
                case .failure(let err):
                    if err.code != .serverUnreachable {
                        AppState.shared.error = .lb(error: err)
                    }
                }
            }
        }
    }
    
    func loadUsages() {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.getUsage()
            DispatchQueue.main.async {
                switch res {
                case .success(let usage):
                    self.usage = usage
                case .failure(let err):
                    if err.code != .serverUnreachable {
                        AppState.shared.error = .lb(error: err)
                    }
                }
            }
        }
    }
    
    func cancelSubscription() {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.cancelSubscription()
            
            DispatchQueue.main.async {
                if case .failure(let err) = res {
                    AppState.shared.error = .lb(error: err)
                }
            }
        }
    }
    
    func deleteAccountAndExit() {
        switch AppState.lb.deleteAccount() {
        case .success(_):
            exit(0)
        case .failure(let err):
            AppState.shared.error = .lb(error: err)
        }
    }
}

#if DEBUG
extension SettingsViewModel {
    static var preview: SettingsViewModel {
        return SettingsViewModel()
    }
}
#endif
