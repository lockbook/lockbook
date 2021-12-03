import SwiftUI
import SwiftLockbookCore


struct UsageSettingsView: View {
    
    @EnvironmentObject var settingsState: SettingsService
    
    var body: some View {
        VStack (spacing: 20){
            HStack (alignment: .top) {
                Text("Server Utilization:")
                    .frame(maxWidth: 175, alignment: .trailing)
                if let usage = settingsState.serverUsages {
                    VStack {
                        if settingsState.usageProgress < 0.8 {
                            ProgressView(value: settingsState.usageProgress)
                        } else if settingsState.usageProgress < 0.9 {
                            ProgressView(value: settingsState.usageProgress)
                                .accentColor(Color.orange)
                        } else {
                            ProgressView(value: settingsState.usageProgress)
                                .accentColor(Color.red)
                        }
                        Text("\(usage.serverUsage.readable) / \(usage.dataCap.readable)")
                    }
                } else {
                    Text("Calculating...")
                }
            }
            HStack (alignment: .top) {
                Text("Uncompressed usage:")
                    .frame(maxWidth: 175, alignment: .trailing)
                if let usage = settingsState.uncompressedUsage {
                    Text(usage.readable)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            
            HStack (alignment: .top) {
                Text("Compression ratio:")
                    .frame(maxWidth: 175, alignment: .trailing)
                Text(settingsState.compressionRatio)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .padding(20)
        .onAppear(perform: settingsState.calculateUsage)
    }
    
    
}
