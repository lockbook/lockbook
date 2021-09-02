import SwiftLockbookCore
import SwiftUI

class OnboardingService: ObservableObject {
    
    let core: LockbookApi
    
    @Published var working: Bool = false
    
    @Published var username: String = ""
    @Published var createAccountError: String = ""
    
    @Published var accountString: String = ""
    @Published var importAccountError: String = ""
    
    @Published var initialSyncing: Bool = false
    
    init(_ core: LockbookApi) {
        self.core = core
    }
    
    func attemptCreate() {
        self.working = true
        self.createAccountError = ""
        self.importAccountError = ""
        DispatchQueue.global(qos: .userInitiated).async {
            let operation = self.core.createAccount(username: self.username, apiLocation: ConfigHelper.get(.apiLocation))
            DispatchQueue.main.async {
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
                        DI.errors.handleError(err)
                    }
                    break
                }
            }
            
        }
    }
    
    func handleImport() {
        self.working = true
        self.createAccountError = ""
        self.importAccountError = ""
        DispatchQueue.global(qos: .userInitiated).async {
            let res = self.core.importAccount(accountString: self.accountString)
            DispatchQueue.main.async {
                self.working = false
                switch res {
                case .success:
                    self.initialSyncing = true
                    DispatchQueue.global(qos: .userInteractive).async {
                        switch self.core.syncAll() {
                        case .success:
                            self.getAccountAndFinalize()
                        case .failure(let err):
                            DI.errors.handleError(err)
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
                        DI.errors.handleError(error)
                    }
                }
            }
        }
    }
    
    func getAccountAndFinalize() {
        let _ = DI.accounts.getAccount()
        DI.files.refresh()
    }
}
