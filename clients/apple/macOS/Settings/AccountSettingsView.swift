import SwiftUI
import SwiftLockbookCore

struct AccountSettingsView: View {
    
    let account: Account
    
    @EnvironmentObject var settings: SettingsService
    @EnvironmentObject var accounts: AccountService
    
    @State var showingLogoutConfirmation = false
    @State var deleteAccountConfirmation = false
    @State var deleteAccount = false
    
    let labelWidth = 175.0
    let rightColumnWidth = 256.0
    let buttonsWidth = 175.0
    let spacing = 20.0
    
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
        VStack (spacing: spacing) {
            HStack (alignment: .top) {
                Text("Username:")
                    .frame(maxWidth: labelWidth, alignment: .trailing)
                Text(account.username)
                    .font(.system(.body, design: .monospaced))
                    .frame(maxWidth: rightColumnWidth, alignment: .leading)
            }
            HStack (alignment: .top) {
                Text("Server Location:")
                    .frame(maxWidth: labelWidth, alignment: .trailing)
                Text(account.apiUrl)
                    .font(.system(.body, design: .monospaced))
                    .frame(maxWidth: rightColumnWidth, alignment: .leading)
            }
            HStack (alignment: .top) {
                Text("Private Key:")
                    .frame(maxWidth: labelWidth, alignment: .trailing)
                VStack {
                    Button(action: settings.copyAccountString, label: {
                        HStack {
                            Spacer()
                            Text(settings.copyToClipboardText)
                            Spacer()
                        }
                    }).frame(maxWidth: buttonsWidth, alignment: .leading)
                    
                    Button(action: {codeRevealed.toggle()}, label: {
                        HStack {
                            Spacer()
                            Text(qrCodeText)
                            Spacer()
                        }
                    }).frame(maxWidth: buttonsWidth, alignment: .leading)
                }.frame(maxWidth: rightColumnWidth, alignment: .leading)
            }
            HStack (alignment: .top) {
                Text("Account:")
                    .frame(maxWidth: labelWidth, alignment: .trailing)
                VStack {
                    Button(role: .destructive, action: {
                        deleteAccountConfirmation = true
                    }) {
                        HStack {
                            Spacer()
                            Text("Delete Account")
                            Spacer()
                        }
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
                    .frame(maxWidth: buttonsWidth, alignment: .leading)
                    
                    Button(action: {
                        WindowManager.shared.openLogoutConfirmationWindow()
                    }) {
                        HStack {
                            Spacer()
                            Text("Logout")
                            Spacer()
                        }
                    }
                    .frame(maxWidth: buttonsWidth, alignment: .leading)
                    .padding(.top, spacing)
                    
                }.frame(maxWidth: rightColumnWidth, alignment: .leading)
            }
            
            if codeRevealed {
                settings.accountCode()
            }
        }
        .padding(spacing)
    }
}
