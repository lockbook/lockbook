import SwiftUI
import SwiftLockbookCore

struct PrerequisiteInformation {
    let serverUsages: UsageMetrics
    let uncompressedUsage: UsageItemMetric
    var compressionRatio: String {
        let ratio = Double(uncompressedUsage.exact) / Double(serverUsages.serverUsage.exact)
        return "\( round(ratio*10) / 10.0 )x"
    }
}

struct UsageSettingsView: View {
    
    @ObservedObject var core: GlobalState
    
    @State var usages: PrerequisiteInformation?
    var progress: Double {
        switch usages {
        case .some(let usage):
            return min(1.0, Double(usage.serverUsages.serverUsage.exact) / Double(usage.serverUsages.dataCap.exact))
        case .none:
            return 0
        }
    }
    
    var body: some View {
        VStack (spacing: 20){
            HStack (alignment: .top) {
                Text("Server Utilization:")
                    .frame(maxWidth: 175, alignment: .trailing)
                if let usage = usages {
                    VStack {
                        if progress < 0.8 {
                            ProgressView(value: progress)
                        } else if progress < 0.9 {
                            ProgressView(value: progress)
                                .accentColor(Color.orange)
                        } else {
                            ProgressView(value: progress)
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
                if let usage = usages {
                    Text(usage.uncompressedUsage.readable)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            
            HStack (alignment: .top) {
                Text("Compression ratio:")
                    .frame(maxWidth: 175, alignment: .trailing)
                if let usage = usages {
                    Text(usage.compressionRatio)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                
            }
        }
        .padding(20)
        .onAppear(perform: calculateUsage)
    }
    
    func calculateUsage() {
        if self.usages == nil {
            DispatchQueue.main.async {
                switch core.api.getUsage() {
                case .success(let usages):
                    switch core.api.getUncompressedUsage() {
                    case .success(let uncompressedUsage):
                        self.usages = PrerequisiteInformation(serverUsages: usages, uncompressedUsage: uncompressedUsage)
                    case .failure(let err):
                        core.handleError(err)
                    }
                case .failure(let err):
                    core.handleError(err)
                }
            }
        }
    }
}
