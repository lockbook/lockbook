import SwiftUI
import SwiftLockbookCore

struct SettingsView: View, Equatable {
    
    @EnvironmentObject var settingsState: SettingsService
    
    let account: Account
    
    var body: some View {
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
                if let serverUsage = settingsState.serverUsages, let uncompressedUsage = settingsState.uncompressedUsage {
                    VStack (alignment: .leading) {
                        HStack {
                            Text("Server Utilization:")
                            Spacer()
                            Text("\(serverUsage.serverUsage.readable) / \(serverUsage.dataCap.readable)")
                        }
                        if settingsState.usageProgress > 0.95 {
                            ProgressView(value: settingsState.usageProgress)
                                .accentColor(Color.red)
                        } else if settingsState.usageProgress > 0.85 {
                            ProgressView(value: settingsState.usageProgress)
                                .accentColor(Color.orange)
                        } else if settingsState.usageProgress > 0.80 {
                            ProgressView(value: settingsState.usageProgress)
                                .accentColor(Color.yellow)
                        } else {
                            ProgressView(value: settingsState.usageProgress)
                                .accentColor(Color.accentColor)
                        }
                        
                    }
                    HStack {
                        Text("Uncompressed usage:")
                        Spacer()
                        Text(uncompressedUsage.readable)
                    }
                    HStack {
                        Text("Compression ratio:")
                        Spacer()
                        Text(settingsState.compressionRatio)
                    }
                } else {
                    Text("Calculating...")
                }
            }.onAppear(perform: settingsState.calculateUsage)
        }
        .navigationBarTitle("Settings")
    }
    
    static func == (lhs: SettingsView, rhs: SettingsView) -> Bool {
        true
    }
    
}

struct SettingsViewPreview: PreviewProvider {
        
    static var previews: some View {
        NavigationView {
            SettingsView(account: Mock.accounts.account!)
                .mockDI()
        }
    }
}
