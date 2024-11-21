import SwiftWorkspace
import SwiftUI

class OnboardingService: ObservableObject {
    
    let core: Lb
    
    @Published var anAccountWasCreatedThisSession = false
    @Published var working: Bool = false
    
    @Published var username: String = ""
    @Published var createAccountError: String = ""
    
    @Published var accountString: String = ""
    @Published var importAccountError: String = ""
    
    @Published var initialSyncing: Bool = false
    
    init(_ core: Lb) {
        self.core = core
    }
    
    func attemptCreate() {
        self.working = true
        self.createAccountError = ""
        self.importAccountError = ""
        DispatchQueue.global(qos: .userInitiated).async {
            let operation = self.core.createAccount(username: self.username, apiUrl: ConfigHelper.get(.apiLocation), welcomeDoc: true)
            DispatchQueue.main.async {
                self.working = false                
                switch operation {
                case .success:
                    DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(1)) { self.anAccountWasCreatedThisSession = true }
                    self.getAccountAndFinalize()
                    break
                case .failure(let err):
                    switch err.code {
                    case .accountExists:
                        self.createAccountError = "You already have an account! Please file a bug report!"
                    case .clientUpdateRequired:
                        self.createAccountError = "Please download the most recent version of Lockbook to create an account!"
                    case .serverUnreachable:
                        self.createAccountError = "Could not reach lockbook.net!"
                    case .usernameInvalid:
                        self.createAccountError = "That username is not valid!"
                    case .usernameTaken:
                        self.createAccountError = "That username is not available!"
                    case .serverDisabled:
                        self.createAccountError = "This server is not accepting any new accounts at this moment. Please try again another time."
                    default:
                        self.createAccountError = "Unexpected Error!"
                        DI.errors.showError(err)
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
            let res = self.core.importAccount(key: self.accountString, apiUrl: nil)
            DispatchQueue.main.async {
                self.working = false
                switch res {
                case .success:
                    self.initialSyncing = true
                    DI.sync.importSync()
                case .failure(let error):
                    switch error.code {
                    case .accountNonexistent:
                        self.importAccountError = "The account specified in the key does not exist on the server specified on the key!"
                    case .accountExists:
                        self.importAccountError = "An account exists already! Please file a bug report!"
                    case .accountStringCorrupted:
                        self.importAccountError = "This account string is corrupted!"
                    case .clientUpdateRequired:
                        self.importAccountError = "Lockbook must be updated before you can continue!"
                    case .serverUnreachable:
                        self.importAccountError = "Could not reach lockbook.net!"
                    case .usernamePublicKeyMismatch:
                        self.importAccountError = "That username does not match the public key stored on this server!"
                    default:
                        DI.errors.showError(error)
                    }
                }
            }
        }
    }
    
    func getAccountAndFinalize() {
        DI.accounts.getAccount()
        DI.files.refresh()
        print("finished!")
    }
}
