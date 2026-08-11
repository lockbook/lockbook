import Foundation
import SwiftWorkspace

@Observable class SettingsModel {
    var usage: UsageMetrics? = nil
    var isPremium: Bool? = nil

    func load() {
        loadTier()
        loadUsages()
    }

    func loadTier() {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.getSubscriptionInfo()

            DispatchQueue.main.async {
                switch res {
                case let .success(info):
                    self.isPremium = info?.isPremium() ?? false
                case let .failure(err):
                    self.report(err)
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
                    self.report(err)
                }
            }
        }
    }

    func cancelSubscription() {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.cancelSubscription()

            DispatchQueue.main.async {
                switch res {
                case .success:
                    self.loadTier()
                case let .failure(err):
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

    private func report(_ err: LbError) {
        if err.code != .serverUnreachable {
            AppState.shared.error = .lb(error: err)
        }
    }
}

extension SettingsModel {
    static var preview: SettingsModel {
        SettingsModel()
    }
}
