import Foundation
import SwiftLockbookCore

class DbStateService: ObservableObject {
    private let core: LockbookApi
    
    @Published var dbState: DbState?
    
    init(_ core: LockbookApi) {
        self.core = core
        
        switch core.getState() {
        case .success(let dbState):
            self.dbState = dbState
        case .failure(let error):
            DI.errors.handleError(error)
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
                let _ = DI.accounts.getAccount()
            case .failure(let error2):
                DI.errors.handleError(error2)
            }
        case .failure(let error):
            DI.errors.handleError(error)
        }
    }

}
