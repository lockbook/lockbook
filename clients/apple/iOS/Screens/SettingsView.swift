import SwiftUI
import SwiftWorkspace

struct SettingsView: View {
    @StateObject var model = SettingsViewModel()
    
    @State var confirmLogout = false
    @State var deleteAccountConfirmation = false
    
    @State var showAccountKeys = false
    
    var body: some View {
        Form {
            Section("Account") {
                if let account = model.account {
                    HStack {
                        Text("Username:")
                        Spacer()
                        Text(account.username)
                    }
                } else {
                    ProgressView()
                }
                
                Button(action: {
                    AuthHelper.authenticateWithBiometricsOrPasscode { success in
                        showAccountKeys = success
                    }
                }, label: {
                    Text("Reveal account keys")
                })
                
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
            
            Section("Usage") {
                if let isPremium = model.isPremium {
                    HStack {
                        Text("Current Tier:")
                        Spacer()
                        Text(isPremium ? "Premium" : "Free")
                    }
                    
                    if !isPremium {
                        NavigationLink("Upgrade now") {
                            UpgradeAccountView(settingsModel: model)
                        }
                    }
                }
                
                if let usage = model.usage {
                    VStack {
                        HStack {
                            Text("Server Utilization:")
                            Spacer()
                            Text("\(usage.serverUsedHuman) / \(usage.serverCapHuman)")
                        }
                        
                        ProgressView(value: Double(usage.serverUsedExact), total: Double(usage.serverCapExact))
                            .padding(.top, 10)
                            .padding(.bottom, 8)
                    }
                } else {
                    ProgressView()
                }
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
                        Text("Server:")
                            .padding(.trailing, 10)
                        Text(account.apiUrl)
                            .lineLimit(1)
                            .truncationMode(.head)
                            .frame(maxWidth: .infinity, alignment: .trailing)
                    }
                } else {
                    ProgressView()
                }
                
                NavigationLink(destination: DebugView()) {
                    Text("Debug Info")
                }
            }
        }
        .navigationDestination(isPresented: $showAccountKeys, destination: {
            AccountKeysView()
        })
        .navigationTitle("Settings")
        .navigationBarTitleDisplayMode(.large)
    }
}

struct AccountKeysView: View {
    let accountKey = (try? AppState.lb.exportAccountPrivateKey().get()) ?? "ERROR"

    var body: some View {
        Form {
            Section("Phrase") {
                AccountPhraseView(includeBackground: false)
            }

            Section("Compact") {
                VStack {
                    HStack {
                        Text(accountKey)
                            .font(.system(.body, design: .monospaced))
                            .lineLimit(1)
                            .truncationMode(.middle)
                            .padding(10)

                        Spacer()

                        Button(action: {
                            ClipboardHelper.copyToClipboard(accountKey)
                        }) {
                            Image(systemName: "doc.on.doc")
                                .foregroundColor(.blue)
                                .padding(8)
                        }
                    }

                    HStack {
                        Spacer()
                        QRView(text: accountKey)
                        Spacer()
                    }
                    .padding(.top, 5)
                }
                .padding(.vertical, 5)
            }
        }
        .navigationTitle("Account Keys")
        .navigationBarTitleDisplayMode(.large)
    }
}


#Preview {
    NavigationStack {
        SettingsView()
    }
    .environmentObject(BillingState())
}


