import SwiftUI
import SwiftLockbookCore

struct UsageSettingsView: View {
    
    @EnvironmentObject var settingsState: SettingsService
    @EnvironmentObject var billing: BillingService
        
    var body: some View {
        VStack (spacing: 20){
            HStack (alignment: .top) {
                Text("Server Utilization:")
                    .frame(maxWidth: 175, alignment: .trailing)
                if let usage = settingsState.usages {
                    VStack {
                        ColorProgressBar(value: settingsState.usageProgress)
                        Text("\(usage.serverUsages.serverUsage.readable) / \(usage.serverUsages.dataCap.readable)")
                    }
                } else {
                    Text("Calculating...")
                }
            }
            HStack (alignment: .top) {
                Text("Uncompressed usage:")
                    .frame(maxWidth: 175, alignment: .trailing)
                if let usage = settingsState.usages {
                    Text(usage.uncompressedUsage.readable)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
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
        .padding(20)
        .onAppear(perform: settingsState.calculateUsage)
    }
}
