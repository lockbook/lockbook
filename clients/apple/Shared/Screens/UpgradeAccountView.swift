import SwiftUI
import SwiftWorkspace

struct UpgradeAccountView: View {
    @StateObject var settingsModel: SettingsViewModel
    @EnvironmentObject var billingState: BillingState
    
    var body: some View {
        VStack(alignment: .leading) {
            Text("Expand your storage to **30GB** for **\(billingState.subProduct?.displayPrice ?? "$2.99")** a month.")
                .padding(.bottom, 30)
            
            if let usage = settingsModel.usage {
                VStack (alignment: .leading, spacing: 15) {
                    Text("Current Usage:")
                    
                    ProgressView(value: Double(usage.serverUsedExact), total: Double(usage.serverCapExact))
                }
                .padding(.vertical, 15)
                
                VStack(alignment: .leading, spacing: 15) {
                    Text("If you upgraded, your usage would be:")
                    
                    ProgressView(value: Double(usage.serverUsedExact), total: BillingState.PREMIUM_DATA_CAP)
                }
                .padding(.vertical, 15)
            } else {
                ProgressView()
            }
            
            Spacer()
                        
            Button(action: {
                billingState.launchPurchasePremium()
            }, label: {
                Text("Subscribe")
                    .fontWeight(.semibold)
                    .frame(maxWidth: .infinity)
                    .frame(height: 30)
            })
            .buttonStyle(.borderedProminent)
            .padding(.bottom, 10)
            
            switch billingState.purchaseState {
            case .failure:
                
                Text("Failed to purchase. Please try again later.")
                    .foregroundStyle(.red)
                    .fontWeight(.bold)
                    .lineLimit(1, reservesSpace: false)
                    .frame(maxWidth: .infinity, alignment: .center)

            default:
                HStack {
                    Spacer()
                    
                    ProgressView()
                        .opacity(billingState.purchaseState == .pending ? 1 : 0)

                    Spacer()
                }
            }
            
            Text("Please review our [Terms of Service](https://lockbook.net/tos) and our [Privacy Policy](https://lockbook.net/privacy-policy).")
                .foregroundColor(.gray)
                .font(.caption)
                .padding(.top, 5)
                .frame(maxWidth: .infinity, alignment: .center)
        }
        .navigationTitle("Upgrade Account")
        .modifier(LargeNavigationTitleBar())
        .padding(.vertical, 10)
        .padding(.horizontal, 20)
    }
}


#Preview("Upgrade Account") {
    NavigationStack {
        UpgradeAccountView(settingsModel: .preview)
            .withCommonPreviewEnvironment()
    }
}

#Preview("Upgrade Account - Failure") {
    let billingState = BillingState()
    billingState.purchaseState = .failure
    
    return NavigationStack {
        UpgradeAccountView(settingsModel: .preview)
            .environmentObject(billingState)
            .withCommonPreviewEnvironment()
    }
}
