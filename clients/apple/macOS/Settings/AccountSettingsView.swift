import SwiftUI
import SwiftLockbookCore

struct AccountSettingsView: View {
    
    let account: Account
    
    @EnvironmentObject var settings: SettingsService
    @EnvironmentObject var accounts: AccountService
    
    @State var deleteAccountConfirmation = false
    @State var deleteAccount = false
    
    // MARK: QR Code things
    @State var codeRevealed: Bool = false
    var qrCodeText: String {
        if codeRevealed {
            return "Hide QR Code"
        } else {
            return "Reveal as QR"
        }
    }
    
    var body: some View {
        VStack (spacing: 20){
            HStack (alignment: .top) {
                Text("Username:")
                    .frame(maxWidth: 175, alignment: .trailing)
                Text(account.username)
                    .font(.system(.body, design: .monospaced))
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            HStack (alignment: .top) {
                Text("Server Location:")
                    .frame(maxWidth: 175, alignment: .trailing)
                Text(account.apiUrl)
                    .font(.system(.body, design: .monospaced))
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            HStack (alignment: .top) {
                Text("Private Key:")
                    .frame(maxWidth: 175, alignment: .trailing)
                VStack {
                    Button(action: settings.copyAccountString, label: {
                        Text(settings.copyToClipboardText)
                    }).frame(maxWidth: .infinity, alignment: .leading)
                    
                    Button(action: {codeRevealed.toggle()}, label: {
                        Text(qrCodeText)
                    }).frame(maxWidth: .infinity, alignment: .leading)
                    
                    Button("Delete Account", role: .destructive) {
                        deleteAccountConfirmation = true
                    }
                    .foregroundColor(.red)
                    .confirmationDialog("Are you sure you want to delete your account?", isPresented: $deleteAccountConfirmation) {
                        Button("Delete account", role: .destructive) {
                            accounts.deleteAccount()
                            deleteAccount = true
                        }
                        .onDisappear {
                            if deleteAccount {
                                NSApplication.shared.keyWindow?.close()
                            }
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.top, 20)
                    
                }.frame(maxWidth: .infinity, alignment: .leading)
            }
            
            if codeRevealed {
                settings.accountCode()
            }
        }.padding(20)
    }
}
