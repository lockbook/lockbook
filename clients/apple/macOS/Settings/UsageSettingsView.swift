import SwiftUI
import SwiftLockbookCore

struct UsageSettingsView: View {
    
    @EnvironmentObject var settingsState: SettingsService
    @EnvironmentObject var billing: BillingService
    
    @State var cancelSubscriptionConfirmation = false
    
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
                
            HStack (alignment: .top) {
                Text("Current tier")
                    .frame(maxWidth: 175, alignment: .trailing)
                    
                switch settingsState.tier {
                case .Premium: Text("Premium")
                        .frame(maxWidth: .infinity, alignment: .leading)
                case .Trial: Text("Trial")
                        .frame(maxWidth: .infinity, alignment: .leading)
                case .Unknown: Text("Unknown")
                        .frame(maxWidth: .infinity, alignment: .leading)
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
        }
        .padding(20)
        .onAppear(perform: settingsState.calculateUsage)
    }
}
