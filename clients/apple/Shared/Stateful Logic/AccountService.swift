import Foundation
import SwiftLockbookCore

class AccountService: ObservableObject {
    let core: LockbookApi
    
    @Published var account: Account? = nil
    
    init(_ core: LockbookApi) {
        self.core = core
        
        switch core.getAccount() {
        case .success(let account):
            self.account = account
        case .failure(let error):
            switch error.kind {
            case .UiError(let getAccountError):
                switch getAccountError {
                case .NoAccount:
                    account = nil
                }
            case .Unexpected(_):
                DI.errors.handleError(error)
            }
        }
    }
    
    func getAccount() {
        if account == nil {
            switch core.getAccount() {
            case .success(let account):
                self.account = account
            case .failure(let error):
                switch error.kind {
                case .UiError(let getAccountError):
                    switch getAccountError {
                    case .NoAccount:
                        print("account get unsuccessful")
                        self.account = nil
                    }
                case .Unexpected(_):
                    DI.errors.handleError(error)
                }
            }
        }
    }
}
