import SwiftUI
import AlertToast

struct ManageSubscriptionView: View {
    @EnvironmentObject var billing: BillingService
    @EnvironmentObject var settings: SettingsService
    
    @Environment(\.presentationMode) var presentationMode
    
    @State var cancelSubscriptionConfirmation = false
            
    var body: some View {
        VStack(spacing: 20) {
            tier
            
            if settings.tier == .Trial || settings.tier == .Unknown {
                usage
                trial
                
                Text("Expand your storage to **30GB** for **\(billing.maybeMonthlySubscription?.displayPrice ?? "$2.99")** a month.")
                    .frame(maxWidth: 300, alignment: .trailing)
                
                HStack {
                    Spacer()
                    Button("Subscribe")
                    {
                        billing.purchasePremium()
                    }
                    .buttonStyle(.borderedProminent)
                    .padding(.top)
                    .disabled(billing.purchaseResult == .some(.inFlow))
                    
                    Spacer()
                }
                
                if case .some(.inFlow) = billing.purchaseResult {
                    loading
                } else if case .some(.failure) = billing.purchaseResult {
                    error
                }
            } else {
                if billing.cancelSubscriptionResult != .appstoreActionRequired {
                    Button("Cancel Subscription", role: .destructive) {
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
            
            legal
        }
        .padding(.top, 20)
        .padding(.horizontal, 20)
            .navigationTitle("Premium")
            .toast(isPresenting: Binding(get: {
                billing.showPurchaseToast
            }, set: { _ in
                billing.showPurchaseToast = false
            }), duration: 2, tapToDismiss: true) {
                purchaseToast()
            }
            .onChange(of: billing.purchaseResult) { newValue in
                if case .some(.success) = newValue {
                    settings.calculateUsage()
                }
            }
            .onAppear {
                settings.calculateUsage()
                billing.launchBillingBackgroundTasks()
            }
    }
    
    func purchaseToast() -> AlertToast {
        switch billing.purchaseResult {
        case .some(.success):
            return AlertToast(type: .regular, title: "You have successfully purchased premium!")
        case .some(.pending):
            return AlertToast(type: .regular, title: "Your purchase is pending.")
        default:
            return AlertToast(type: .regular, title: "ERROR")
        }
    }
    
        
    @ViewBuilder
    var tier: some View {
        HStack (alignment: .top) {
            Text("Current tier:")
                .frame(maxWidth: 250, alignment: .trailing)
                
            switch settings.tier {
            case .Premium: Text("Premium")
                    .frame(maxWidth: .infinity, alignment: .leading)
            case .Trial: Text("Trial")
                    .frame(maxWidth: .infinity, alignment: .leading)
            case .Unknown: Text("Unknown")
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }
    
    @ViewBuilder
    var usage: some View {
        HStack(alignment: .top) {
            Text("Current Usage:")
                .frame(maxWidth: 250, alignment: .trailing)
            ColorProgressBar(value: settings.usageProgress)
        }
    }
    
    @ViewBuilder
    var trial: some View {
        HStack(alignment: .top) {
            Text("If you upgraded, your usage would be:")
                .frame(maxWidth: 250, alignment: .trailing)
            ColorProgressBar(value: settings.premiumProgress)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
    
    @ViewBuilder
    var error: some View {
        HStack {
            Text("Failed to complete purchase.")
                .padding(.vertical)
                .foregroundColor(.red)
                .frame(maxWidth: 250, alignment: .trailing)
        }
    }
    
    @ViewBuilder
    var loading: some View {
        HStack {
            ProgressView()
        }
    }
    
    @ViewBuilder
    var legal: some View {
        VStack {
            Spacer()
            Text("Please review our [Terms of Service](https://lockbook.net/tos) and our [Privacy Policy](https://lockbook.net/privacy-policy).")
                .foregroundColor(.gray)
                .font(.caption)
                .padding()
        }
    }
}
