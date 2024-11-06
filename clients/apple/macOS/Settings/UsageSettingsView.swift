import SwiftUI

struct UsageSettingsView: View {
    
    @EnvironmentObject var settingsState: SettingsService
    @EnvironmentObject var billing: BillingService
        
    var body: some View {
        VStack (spacing: 20) {
            if settingsState.offline {
                Text("You are offline.")
            } else {
                HStack (alignment: .top) {
                    Text("Server Utilization:")
                        .frame(maxWidth: 175, alignment: .trailing)
                    if let usage = settingsState.usages {
                        VStack {
                            ColorProgressBar(value: settingsState.usageProgress)
                            Text("\(usage.serverUsages.serverUsedHuman) / \(usage.serverUsages.serverCapHuman)")
                        }
                    } else {
                        Text("Calculating...")
                    }
                }
                if let usage = settingsState.usages {
                    if let uncompressedUsage = usage.uncompressedUsage {
                        HStack (alignment: .top) {
                            Text("Uncompressed usage:")
                                .frame(maxWidth: 175, alignment: .trailing)
                            
                            Text(uncompressedUsage.humanMsg)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                        
                        HStack (alignment: .top) {
                            Text("Compression ratio:")
                                .frame(maxWidth: 175, alignment: .trailing)
                            if let usage = settingsState.usages {
                                Text(usage.compressionRatio)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                            }
                        }
                    }
                }
            }
        }
        .padding(20)
        .onAppear(perform: {
            settingsState.calculateUsage(calcUncompressed: true)
        })
    }
}
