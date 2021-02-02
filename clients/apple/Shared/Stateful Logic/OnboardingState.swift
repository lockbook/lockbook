import Foundation
import SwiftLockbookCore
import SwiftUI

class OnboardingState: ObservableObject {
    
    @ObservedObject var core: GlobalState
    
    @Published var working: Bool = false
    
    @Published var username: String = ""
    @Published var createAccountError: String = ""
    
    @Published var accountString: String = ""
    @Published var importAccountError: String = ""
    
    @Published var initialSyncing: Bool = false
    
    func attemptCreate() {
        self.working = true
        self.createAccountError = ""
        self.importAccountError = ""
        DispatchQueue.main.async {
            let operation = self.core.api
                .createAccount(username: self.username, apiLocation: ConfigHelper.get(.apiLocation))
            self.working = false
            
            switch operation {
            case .success:
                self.getAccountAndFinalize()
                break
            case .failure(let err):
                switch err.kind {
                case .UiError(let uiError):
                    switch uiError {
                    case .AccountExistsAlready:
                        self.createAccountError = "You already have an account! Please file a bug report!"
                    case .ClientUpdateRequired:
                        self.createAccountError = "Please download the most recent version of Lockbook to create an account!"
                    case .CouldNotReachServer:
                        self.createAccountError = "Could not reach \(ConfigHelper.get(.apiLocation))!"
                    case .InvalidUsername:
                        self.createAccountError = "That username is not valid!"
                    case .UsernameTaken:
                        self.createAccountError = "That username is not available!"
                    }
                    break;
                case .Unexpected:
                    self.createAccountError = "Unexpected Error!"
                    self.core.handleError(err)
                }
                break
            }
        }
    }
    
    func handleImport() {
        self.working = true
        self.createAccountError = ""
        self.importAccountError = ""
        DispatchQueue.main.async {
            let res = self.core.api.importAccount(accountString: self.accountString)
            self.working = false
            switch res {
            case .success:
                self.initialSyncing = true
                DispatchQueue.global(qos: .userInteractive).async {
                    switch self.core.api.syncAll() {
                    case .success:
                        self.getAccountAndFinalize()
                    case .failure(let err):
                        self.core.handleError(err)
                    }
                }
            case .failure(let error):
                switch error.kind {
                case .UiError(let importError):
                    switch importError {
                    case .AccountDoesNotExist:
                        self.importAccountError = "The account specified in the key does not exist on the server specified on the key!"
                    case .AccountExistsAlready:
                        self.importAccountError = "An account exists already! Please file a bug report!"
                    case .AccountStringCorrupted:
                        self.importAccountError = "This account string is corrupted!"
                    case .ClientUpdateRequired:
                        self.importAccountError = "Lockbook must be updated before you can continue!"
                    case .CouldNotReachServer:
                        self.importAccountError = "Could not reach \(ConfigHelper.get(.apiLocation))!"
                    case .UsernamePKMismatch:
                        self.importAccountError = "That username does not match the public key stored on this server!"
                    }
                case .Unexpected:
                    self.core.handleError(error)
                }
            }
        }
    }
    
    func getAccountAndFinalize() {
        DispatchQueue.main.async {
            switch self.core.api.getAccount() {
            case .success(let account):
                self.core.account = account
                self.core.updateFiles()
            case .failure(let err):
                self.core.handleError(err)
            }
        }
    }
    
    init(core: GlobalState) {
        self.core = core
    }
}
