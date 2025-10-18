import SwiftUI
import SwiftWorkspace

struct SettingsView: View {
    @StateObject var model = SettingsViewModel()
    
    var body: some View {
        NavigationStack {
            TabView {
                SettingsAccountView(model: model)
                    .tabItem {
                        Label("Account", systemImage: "person")
                    }
                
                SettingsUsageView(model: model)
                    .tabItem {
                        Label("Usage", systemImage: "externaldrive")
                    }
                
                SettingsDebugView(model: model)
                    .tabItem {
                        Label("Debug", systemImage: "hammer")
                    }
            }
            .navigationTitle("Settings")
        }
        .frame(width: 500, height: 400)
    }
}

struct SettingsAccountView: View {
    @ObservedObject var model: SettingsViewModel
    
    @State var confirmLogout = false
    @State var confirmDeleteAccount = false
    
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
                    Text("Reveal Account Keys")
                        .foregroundStyle(.foreground)
                    Spacer()
                    Image(systemName: "chevron.right")
                        .imageScale(.small)
                        .foregroundStyle(.gray)
                })
                .buttonStyle(.borderless)
                
                HStack {
                    Text("Logout")
                    Spacer()
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
            }
            
            Section("Privacy") {
                HStack {
                    Text("Privacy Policy")
                    Spacer()
                    Link("Open in browser", destination: URL(string: "https://lockbook.net/privacy-policy")!)
                }
                
                HStack {
                    Text("Terms of Service")
                    Spacer()
                    Link("Open in browser", destination: URL(string: "https://lockbook.net/tos")!)
                }
                
                HStack {
                    Text("Delete Account")
                    Spacer()
                    Button("Delete Account", role: .destructive) {
                        confirmDeleteAccount = true
                    }
                    .confirmationDialog("Are you sure you want to delete your account?", isPresented: $confirmDeleteAccount, titleVisibility: .visible) {
                        Button("Delete account", role: .destructive) {
                            model.deleteAccountAndExit()
                        }
                    }
                }
            }
            
            
        }
        .formStyle(.grouped)
        .navigationDestination(isPresented: $showAccountKeys, destination: {
            AccountKeysView()
                .toolbar {
                    ToolbarItem(placement: .principal) {
                        Text("Account Keys").font(.headline)
                    }
                }
        })
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
                                .foregroundColor(Color.accentColor)
                                .padding(4)
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
        .formStyle(.grouped)
    }
}

struct SettingsUsageView: View {
    @ObservedObject var model: SettingsViewModel
    
    @State var confirmCancelSubscription = false
    
    @AppStorage("usageBarMode") private var usageBarMode: UsageBarDisplayMode = .whenHalf
    
    var body: some View {
        Form {
            Section("Usage") {
                if let isPremium = model.isPremium {
                    HStack {
                        Text("Current Tier:")
                        Spacer()
                        Text(isPremium ? "Premium" : "Free")
                    }
                    
                    if !isPremium {
                        NavigationLink("Upgrade Now") {
                            VStack {
                                UpgradeAccountView(settingsModel: model)
                            }
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
                
                Picker("Display Mode", selection: $usageBarMode) {
                    ForEach(UsageBarDisplayMode.allCases) { mode in
                        Text(mode.label).tag(mode)
                    }
                }
                .pickerStyle(.menu)
                
                if model.isPremium == true {
                    HStack {
                        Text("Cancel Subscription")
                        Spacer()
                        Button("Cancel Subscription", role: .destructive) {
                            self.confirmCancelSubscription = true
                        }
                    }
                    .confirmationDialog("Are you sure you want to cancel your subscription?", isPresented: $confirmCancelSubscription, titleVisibility: .visible) {
                        Button("Confirm", role: .destructive) {
                            model.cancelSubscription()
                        }
                    }
                }
            }
        }
        .formStyle(.grouped)
    }
}

struct SettingsDebugView: View {
    @ObservedObject var model: SettingsViewModel

    var body: some View {
        Form {
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
                
                DebugView()
            }
        }
        .formStyle(.grouped)
    }
}


#Preview {
    SettingsView()
        .withCommonPreviewEnvironment()
}
