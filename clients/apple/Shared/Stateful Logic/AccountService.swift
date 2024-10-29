import Foundation
import SwiftWorkspace

//#if os(macOS)
//import AppKit
//import Combine
//#endif

class AccountService: ObservableObject {
    let core: Lb
    
    @Published var account: Account? = nil
    var calculated = false
        
    init(_ core: Lb) {
        self.core = core
        switch core.getAccount() {
        case .success(let account):
            print("got account.")
            self.account = account
        case .failure(let error):
            print("did not get account.")
            if error.code == .accountNonexistent {
                account = nil
            } else {
                DI.errors.handleError(error)
            }
        }
        
        calculated = true
    }
        
    func getAccount() {
        if account == nil {
            switch core.getAccount() {
            case .success(let account):
                self.account = account
            case .failure(let error):
                if error.code == .accountNonexistent {
                    account = nil
                } else {
                    DI.errors.handleError(error)
                }
            }
        }
    }
    
    func logout() {
        DI.freshState()
        core.logoutAndExit()
    }
    
    func deleteAccount() {
        switch core.deleteAccount() {
        case .success(_):
            DI.freshState()
        case .failure(let error):
            DI.errors.handleError(error)
        }
    }
}
