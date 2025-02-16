import SwiftUI
import SwiftWorkspace

struct SettingsView: View {
    @StateObject var model = SettingsViewModel()
    
    @State var confirmLogout = false
    @State var deleteAccountConfirmation = false
    
    @State var showCompactAccountKey = false
    
    @State var sheetHeight: CGFloat = 0.0
    
    var body: some View {
        Form {
            Section("Account") {
                if let account = model.account {
                    HStack {
                        Text("Username")
                        Spacer()
                        Text(account.username)
                    }
                } else {
                    ProgressView()
                }
                
                Button(role: .destructive, action: {
                    self.confirmLogout = true
                }, label: {
                    Text("Logout")
                })
                .confirmationDialog("Are you sure? Please make sure your key is backed up.", isPresented: $confirmLogout, titleVisibility: .visible) {
                    Button("Logout", role: .destructive) {
                        AppState.lb.logoutAndExit()
                    }
                }
            }
            
            Section("Account Key") {
                Button(action: {
                    self.showCompactAccountKey = true
                }, label: {
                    Text("Reveal account key")
                })
                .optimizedSheet(isPresented: $showCompactAccountKey, constrainedSheetHeight: $sheetHeight, presentedContent: {
                    AccountKeyView()
                })
                
                Button(action: {
                    self.showCompactAccountKey = true
                }, label: {
                    Text("Show account phrase")
                })
            }
            
            Section(header: Text("Privacy")) {
                Text("[Privacy Policy](https://lockbook.net/privacy-policy)")
                    .foregroundColor(.blue)

                Text("[Terms of Service](https://lockbook.net/tos)")
                    .foregroundColor(.blue)

                Button("Delete Account", role: .destructive) {
                    deleteAccountConfirmation = true
                }
                .confirmationDialog("Are you sure you want to delete your account?", isPresented: $deleteAccountConfirmation, titleVisibility: .visible) {
                    Button("Delete account", role: .destructive) {
                        model.deleteAccountAndExit()
                    }
                }
            }

            
            Section("Debug") {
                if let account = model.account {
                    HStack {
                        Text("Server")
                        Spacer()
                        Text(account.apiUrl)
                    }
                } else {
                    ProgressView()
                }
                
                NavigationLink(destination: DebugView()) {
                    Text("Debug info")
                }
            }
        }
        .navigationTitle("Settings")
    }
}

struct AccountKeyView: View {
    let accountKey = (try? AppState.lb.exportAccountPrivateKey().get()) ?? "ERROR"
    
    var body: some View {
        VStack(spacing: 10) {
            HStack {
                Text("Account Key")
                    .bold()
                
                Spacer()
            }

            Text(accountKey)
                .monospaced()
            
            QRView(text: accountKey)
        }
        .padding(.horizontal)
        .padding(.top, 3)
    }
}

class SettingsViewModel: ObservableObject {
    var account: Account? = nil
    var error: String? = nil
    
    init() {
        self.loadAccount()
    }
    
    func loadAccount() {
        switch AppState.lb.getAccount() {
        case .success(let account):
            self.account = account
        case .failure(let err):
            error = err.msg
        }
    }
    
    func getAccountPhrase() -> String {
        (try? AppState.lb.exportAccountPhrase().get()) ?? "ERROR"
    }
    
    func deleteAccountAndExit() {
        AppState.lb.deleteAccount()
        exit(0)
    }

}

#Preview {
    NavigationStack {
        SettingsView()
    }
}


