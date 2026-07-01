import SwiftUI
import SwiftWorkspace

class SettingsViewModel: ObservableObject {
    @Published var account: Account? = nil
    @Published var usage: UsageMetrics? = nil
    @Published var isPremium: Bool? = nil

    init(initalUsageComputation: Bool = true) {
        loadAccount()
        loadTier()

        if initalUsageComputation {
            loadUsages()
        }
    }

    func loadAccount() {
        switch AppState.lb.getAccount() {
        case let .success(account):
            self.account = account
        case let .failure(err):
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
                case let .success(info):
                    self.isPremium = info?.isPremium() ?? false
                case let .failure(err):
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
                case let .success(usage):
                    self.usage = usage
                case let .failure(err):
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
                if case let .failure(err) = res {
                    AppState.shared.error = .lb(error: err)
                }
            }
        }
    }

    func deleteAccountAndExit() {
        switch AppState.lb.deleteAccount() {
        case .success:
            exit(0)
        case let .failure(err):
            AppState.shared.error = .lb(error: err)
        }
    }
}

extension SettingsViewModel {
    static var preview: SettingsViewModel {
        SettingsViewModel()
    }
}
