import Foundation
import SwiftWorkspace

struct SubAccountKeys: Hashable {
    let username: String
    let key: String
    let phrase: String
}

@Observable class SubAccountsModel {
    static let shared = SubAccountsModel()

    var subAccounts: [SubAccount] = []
    var loaded = false
    var loadError: String? = nil
    var working = false

    func load() {
        guard AppState.shared.account?.isSubAccount == false else {
            return
        }

        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.listSubAccounts()

            DispatchQueue.main.async {
                switch res {
                case let .success(subAccounts):
                    self.subAccounts = subAccounts
                    self.loaded = true
                    self.loadError = nil
                case let .failure(err):
                    self.loadError = err.msg
                }
            }
        }
    }

    func create(label: String, completion: @escaping (Result<String, LbError>) -> Void) {
        run({ AppState.lb.createSubAccount(label: label) }, completion: completion)
    }

    func delete(username: String, completion: @escaping (Result<Void, LbError>) -> Void) {
        run({ AppState.lb.deleteSubAccount(username: username) }) { res in
            if case .success = res {
                self.subAccounts.removeAll { $0.username == username }
            }

            completion(res)
        }
    }

    func exportKeys(username: String, completion: @escaping (Result<SubAccountKeys, LbError>) -> Void) {
        run({
            AppState.lb.exportSubAccountPrivateKey(username: username).flatMap { key in
                AppState.lb.exportSubAccountPhrase(username: username).map { phrase in
                    SubAccountKeys(username: username, key: key, phrase: phrase)
                }
            }
        }, reload: false, completion: completion)
    }

    private func run<T>(
        _ work: @escaping () -> Result<T, LbError>, reload: Bool = true,
        completion: @escaping (Result<T, LbError>) -> Void
    ) {
        working = true

        DispatchQueue.global(qos: .userInitiated).async {
            let res = work()

            DispatchQueue.main.async {
                self.working = false

                if reload, case .success = res {
                    self.load()
                }

                completion(res)
            }
        }
    }
}
