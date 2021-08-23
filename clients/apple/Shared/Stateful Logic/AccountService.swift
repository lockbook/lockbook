import Foundation
import SwiftLockbookCore

class AccountService: ObservableObject {
    let core: LockbookApi
    let errors: UnexpectedErrorService
    
    @Published var account: Account? = nil
    
    init(_ core: LockbookApi, _ error: UnexpectedErrorService) {
        self.core = core
        self.errors = error
        
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
                errors.handleError(error)
            }
        }
    }
    
    func getAccount() -> Account? {
        if account == nil {
            switch core.getAccount() {
            case .success(let account):
                self.account = account
            case .failure(let error):
                switch error.kind {
                case .UiError(let getAccountError):
                    switch getAccountError {
                    case .NoAccount:
                        self.account = nil
                    }
                case .Unexpected(_):
                    errors.handleError(error)
                }
            }
        }
        
        return account
    }
}
