import SwiftUI
import SwiftLockbookCore

struct SettingsView: View, Equatable {
    
    @EnvironmentObject var settingsState: SettingsService
    @EnvironmentObject var accounts: AccountService
    
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
                }
                Section(header: Text("PRIVATE KEY")) {
                    HStack {
                        Button(action: settingsState.copyAccountString) {
                            Text(settingsState.copyToClipboardText)
                        }
                    }
                    HStack {
                        NavigationLink(destination: settingsState.accountCode()) {
                            Text("Reveal QR")
                        }
                    }
                }
                Section(header:  Text("Usage")) {
                    if let usage = settingsState.usages {
                        VStack (alignment: .leading) {
                            HStack {
                                Text("Server Utilization:")
                                Spacer()
                                Text("\(usage.serverUsages.serverUsage.readable) / \(usage.serverUsages.dataCap.readable)")
                            }
                            if settingsState.usageProgress < 0.8 {
                                ProgressView(value: settingsState.usageProgress)
                            } else if settingsState.usageProgress < 0.9 {
                                ProgressView(value: settingsState.usageProgress)
                                    .accentColor(Color.orange)
                            } else {
                                ProgressView(value: settingsState.usageProgress)
                                    .accentColor(Color.red)
                            }
                            
                        }
                        HStack {
                            Text("Uncompressed usage:")
                            Spacer()
                            Text(usage.uncompressedUsage.readable)
                        }
                        HStack {
                            Text("Compression ratio:")
                            Spacer()
                            Text(usage.compressionRatio)
                                .frame(maxWidth: .infinity, alignment: .leading)
                            
                        }
                    } else {
                        Text("Calculating...")
                    }
                }.onAppear(perform: settingsState.calculateUsage)
            }.navigationBarTitle("Settings")
        }
    }
    
    static func == (lhs: SettingsView, rhs: SettingsView) -> Bool {
        true
    }
    
}

struct SettingsViewPreview: PreviewProvider {
    
    static var previews: some View {
        NavigationView {
            SettingsView()
                .mockDI()
                .onAppear {
                    
                    let info = PrerequisiteInformation(
                        serverUsages: UsageMetrics(
                            usages: [],
                            serverUsage: UsageItemMetric(
                                exact: 10,
                                readable: "10 bytes"
                            ),
                            dataCap: UsageItemMetric(
                                exact: 20,
                                readable: "20 bytes"
                            )
                        ),
                        uncompressedUsage: UsageItemMetric(
                            exact: 30,
                            readable: "30 bytes"
                        )
                    )
                    Mock.settings.usages = info
                }
        }
    }
}
