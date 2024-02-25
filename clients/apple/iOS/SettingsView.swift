import SwiftUI
import SwiftLockbookCore


struct SettingsView: View, Equatable {
    
    @EnvironmentObject var billing: BillingService
    @EnvironmentObject var settingsState: SettingsService
    @EnvironmentObject var accounts: AccountService
    
    @Environment(\.presentationMode) var presentationMode
    
    @State private var showingLogoutConfirmation = false
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
                    .sheet(isPresented: $showingLogoutConfirmation) {
                        LogoutConfirmationView().environmentObject(DI.settings)
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
                                Text("\(usage.serverUsages.serverUsage.readable) / \(usage.serverUsages.dataCap.readable)")
                            }
                            ColorProgressBar(value: settingsState.usageProgress)
                        }
                        HStack {
                            Text("Uncompressed usage:")
                            Spacer()
                            Text(usage.uncompressedUsage?.readable ?? "Loading...")
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

                    Text("[End User License Agreement](https://lockbook.net/eula)")
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
            }.navigationBarTitle("Settings")
        }
    }
    
    static func == (lhs: SettingsView, rhs: SettingsView) -> Bool {
        true
    }
    
}

struct Loading: PreviewProvider {
    
    static var previews: some View {
        NavigationView {
            SettingsView()
                .mockDI()
        }
    }
}


struct FreeUser: PreviewProvider {
    
    static var previews: some View {
        NavigationView {
            SettingsView()
                .mockDI()
                .onAppear {
                    let info = PrerequisiteInformation(
                        serverUsages: UsageMetrics(
                            usages: [],
                            serverUsage: UsageItemMetric(
                                exact: 3,
                                readable: "3 bytes"
                            ),
                            dataCap: UsageItemMetric(
                                exact: 1000000,
                                readable: "10 Mb"
                            )
                        ),
                        uncompressedUsage: UsageItemMetric(
                            exact: 60,
                            readable: "60 bytes"
                        )
                    )
                    
                    Mock.settings.usages = info
                }
        }
    }
}

struct FreeUserRunningOutOfSpace: PreviewProvider {
    
    static var previews: some View {
        NavigationView {
            SettingsView()
                .mockDI()
                .onAppear {
                    let info = PrerequisiteInformation(
                        serverUsages: UsageMetrics(
                            usages: [],
                            serverUsage: UsageItemMetric(
                                exact: 850000,
                                readable: "8.5 Mb"
                            ),
                            dataCap: UsageItemMetric(
                                exact: 1000000,
                                readable: "10 Mb"
                            )
                        ),
                        uncompressedUsage: UsageItemMetric(
                            exact: 60,
                            readable: "60 bytes"
                        )
                    )
                    
                    Mock.settings.usages = info
                }
        }
    }
}

struct PremiumUser: PreviewProvider {
    
    static var previews: some View {
        NavigationView {
            SettingsView()
                .mockDI()
                .onAppear {
                    let info = PrerequisiteInformation(
                        serverUsages: UsageMetrics(
                            usages: [],
                            serverUsage: UsageItemMetric(
                                exact: 17,
                                readable: "17 bytes"
                            ),
                            dataCap: UsageItemMetric(
                                exact: 3000000000,
                                readable: "30 Gb"
                            )
                        ),
                        uncompressedUsage: UsageItemMetric(
                            exact: 60,
                            readable: "60 bytes"
                        )
                    )
                    
                    Mock.settings.usages = info
                }
        }
    }
}
