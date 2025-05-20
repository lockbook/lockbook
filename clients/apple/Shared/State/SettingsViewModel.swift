import SwiftUI
import SwiftWorkspace

class SettingsViewModel: ObservableObject {
    @Published var account: Account? = nil
    @Published var usage: UsageMetrics? = nil
    @Published var isPremium: Bool? = nil
    @Published var error: String? = nil
    
    init() {
        self.loadAccount()
        self.loadTier()
        self.loadUsages()
    }
    
    func loadAccount() {
        switch AppState.lb.getAccount() {
        case .success(let account):
            self.account = account
        case .failure(let err):
            error = err.msg
        }
    }
    
    func loadTier() {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.getSubscriptionInfo()
            
            DispatchQueue.main.async {
                switch res {
                case .success(let info):
                    self.isPremium = info != nil
                case .failure(let err):
                    self.error = err.msg
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
                    self.error = err.msg
                }
            }
        }
    }
    
    func deleteAccountAndExit() {
        switch AppState.lb.deleteAccount() {
        case .success(_):
            exit(0)
        case .failure(let err):
            error = err.msg
        }
    }
}
