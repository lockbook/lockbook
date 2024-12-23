import SwiftUI


struct SettingsView: View, Equatable {
    
    @EnvironmentObject var billing: BillingService
    @EnvironmentObject var settingsState: SettingsService
    @EnvironmentObject var accounts: AccountService
    
    @Environment(\.presentationMode) var presentationMode
    
    @State var showingLogoutConfirmation = false
    @State var cancelSubscriptionConfirmation = false
    @State var deleteAccountConfirmation = false
    
    var body: some View {
        switch accounts.account {
        case .none:
            Text("You need an account for settings!")
        case .some(let account):
            Form {
                Section(header: Text("ACCOUNT")) {
                    HStack {
                        Text("Username")
                        Spacer()
                        Text(account.username).font(.system(.body, design: .monospaced))
                    }
                    
                    HStack {
                        Text("Server Location")
                        Spacer()
                        NavigationLink(
                            destination: Text(account.apiUrl)
                                .font(.system(.body, design: .monospaced))
                                .padding()) {
                                    Text(account.apiUrl).font(.system(.body, design: .monospaced)).lineLimit(1).truncationMode(.tail)
                                }
                    }
                    
                    Button(action: {
                        showingLogoutConfirmation = true
                    }) {
                        HStack {
                            Text("Logout")
                            Spacer()
                        }
                    }
                    .fullScreenCover(isPresented: $showingLogoutConfirmation) {
                        let screenWidth = UIScreen.main.bounds.width
                        let buttonWidth = screenWidth > 767 ? screenWidth * 0.5 : screenWidth * 0.9
                        LogoutConfirmationView(
                            h1: 22,
                            h2: 18,
                            buttonWidth: buttonWidth)
                    }
                }
                Section(header: Text("PRIVATE KEY")) {
                    HStack {
                        Button(action: settingsState.copyAccountString) {
                            Text(settingsState.copyToClipboardText)
                        }
                    }
                    NavigationLink(destination: settingsState.accountCode()) {
                        Text("Reveal QR")
                    }
                }
                Section(header:  Text("USAGE")) {
                    if settingsState.offline {
                        Text("You are offline.")
                    } else if let usage = settingsState.usages {
                        VStack (alignment: .leading) {
                            HStack {
                                Text("Server Utilization:")
                                Spacer()
                                Text("\(usage.serverUsages.serverUsedHuman) / \(usage.serverUsages.serverCapHuman)")
                            }
                            ColorProgressBar(value: settingsState.usageProgress)
                        }
                        HStack {
                            Text("Uncompressed usage:")
                            Spacer()
                            Text(usage.uncompressedUsage?.humanMsg ?? "Loading...")
                        }
                        HStack {
                            Text("Compression ratio:")
                            Spacer()
                            Text(usage.compressionRatio)
                            
                        }
                        HStack {
                            Text("Current tier:")
                            Spacer()
                            switch settingsState.tier {
                            case .Premium: Text("Premium")
                            case .Trial: Text("Trial")
                            case .Unknown: Text("Unknown")
                            }
                        }
                        if settingsState.tier == .Trial {
                            NavigationLink(destination: ManageSubscription()) {
                                switch settingsState.tier {
                                case .Premium: Text("Manage Subscription")
                                default: Text("Upgrade to premium")
                                }
                            }
                        }
                        
                        if settingsState.tier == .Premium {
                            if billing.cancelSubscriptionResult != .appstoreActionRequired {
                                Button("Cancel", role: .destructive) {
                                    cancelSubscriptionConfirmation = true
                                }
                                .confirmationDialog("Are you sure you want to cancel your subscription", isPresented: $cancelSubscriptionConfirmation) {
                                    Button("Cancel subscription", role: .destructive) {
                                        billing.cancelSubscription()
                                    }
                                }
                            } else {
                                Text("Please cancel your subscription via the App Store.")
                            }
                        }
                    } else {
                        Text("Calculating...")
                    }
                }
                .onAppear(perform: {
                    settingsState.calculateUsage(calcUncompressed: true)
                })
                
                Section(header: Text("PRIVACY")) {
                    Text("[Privacy Policy](https://lockbook.net/privacy-policy)")
                        .foregroundColor(.blue)

                    Text("[Terms of Service](https://lockbook.net/tos)")
                        .foregroundColor(.blue)

                    Button("Delete Account", role: .destructive) {
                        deleteAccountConfirmation = true
                    }
                    .foregroundColor(.red)
                    .confirmationDialog("Are you sure you want to delete your account?", isPresented: $deleteAccountConfirmation) {
                        Button("Delete account", role: .destructive) {
                            accounts.deleteAccount()
                        }
                    }
                }
                
                Section(header: Text("DEBUG")) {
                    NavigationLink(destination: DebugView()) {
                        Text("See Debug Info")
                    }
                }
            }.navigationBarTitle("Settings")
        }
    }
    
    static func == (lhs: SettingsView, rhs: SettingsView) -> Bool {
        true
    }
    
}
