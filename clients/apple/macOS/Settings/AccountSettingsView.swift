import SwiftUI
import SwiftWorkspace

let buttonCornerRadius = 4.0

struct AccountSettingsButtonStyle: PrimitiveButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background(Color.blue)
            .foregroundColor(.white)
            .font(.body)
            .cornerRadius(buttonCornerRadius)
            .onTapGesture {
                configuration.trigger()
            }
    }
}

struct DestructiveButtonStyle: PrimitiveButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background(Color.red)
            .foregroundColor(.white)
            .font(.body)
            .cornerRadius(buttonCornerRadius)
            .onTapGesture {
                configuration.trigger()
            }
    }
}

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
                VStack (spacing: spacing) {
                    Button(action: settings.copyAccountString, label: {
                        HStack {
                            Spacer()
                            Text(settings.copyToClipboardText).frame(minHeight: spacing)
                            Spacer()
                        }
                    })
                    .buttonStyle(AccountSettingsButtonStyle())
                    .frame(maxWidth: buttonsWidth, alignment: .leading)
                    
                    Button(action: {codeRevealed.toggle()}, label: {
                        HStack {
                            Spacer()
                            Text(qrCodeText).frame(minHeight: spacing)
                            Spacer()
                        }
                    })
                    .buttonStyle(AccountSettingsButtonStyle())
                    .frame(maxWidth: buttonsWidth, alignment: .leading)
                }.frame(maxWidth: rightColumnWidth, alignment: .leading)
            }
            HStack (alignment: .top) {
                Text("Account:")
                    .frame(maxWidth: labelWidth, alignment: .trailing)
                VStack (spacing: spacing) {
                    Button(role: .destructive, action: {
                        deleteAccountConfirmation = true
                    }) {
                        HStack {
                            Spacer()
                            Text("Delete Account").frame(minHeight: spacing)
                            Spacer()
                        }
                    }
                    .buttonStyle(DestructiveButtonStyle())
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
                            Text("Logout").frame(minHeight: spacing)
                            Spacer()
                        }
                    }
                    .buttonStyle(DestructiveButtonStyle())
                    .frame(maxWidth: buttonsWidth, alignment: .leading)
                    
                }.frame(maxWidth: rightColumnWidth, alignment: .leading)
            }
            
            if codeRevealed {
                settings.accountCode()
            }
        }
        .padding(spacing)
    }
}
