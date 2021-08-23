import Foundation
import SwiftLockbookCore

class DbStateService: ObservableObject {
    private let core: LockbookApi
    private let account: AccountService
    private let unexpectedErrors: UnexpectedErrorService
    
    @Published var dbState: DbState?
    
    init(_ core: LockbookApi, _ account: AccountService, _ unexpectedErrors: UnexpectedErrorService) {
        self.core = core
        self.account = account
        self.unexpectedErrors = unexpectedErrors
        
        switch core.getState() {
        case .success(let dbState):
            self.dbState = dbState
        case .failure(let error):
            unexpectedErrors.handleError(error)
        }
    }
    
    
    // TODO do asyncronously
    func migrate() {
        let migrate = core.migrateState()
        
        switch migrate {
        case .success(_):
            switch core.getState() {
            case .success(let state):
                self.dbState = state
                let _ = self.account.getAccount()
            case .failure(let error2):
                unexpectedErrors.handleError(error2)
            }
        case .failure(let error):
            unexpectedErrors.handleError(error)
        }
    }

}
