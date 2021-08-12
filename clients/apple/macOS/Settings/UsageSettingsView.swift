import SwiftUI
import SwiftLockbookCore


struct UsageSettingsView: View {
    
    @ObservedObject var core: GlobalState
    @ObservedObject var settingsState: SettingsState
    
    var body: some View {
        VStack (spacing: 20){
            HStack (alignment: .top) {
                Text("Server Utilization:")
                    .frame(maxWidth: 175, alignment: .trailing)
                if let usage = settingsState.usages {
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
