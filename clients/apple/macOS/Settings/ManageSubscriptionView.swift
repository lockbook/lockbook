import SwiftUI
import AlertToast

struct ManageSubscriptionView: View {
    @EnvironmentObject var billing: BillingService
    @EnvironmentObject var settings: SettingsService
    
    @Environment(\.presentationMode) var presentationMode
            
    var body: some View {
        VStack(alignment: .leading) {
            usage
            trial
            
            Text("Expand your storage to **30GB** for **\(billing.maybeMonthlySubscription?.displayPrice ?? "$2.99")** a month.")
                .padding(.vertical)
            
            HStack {
                Spacer()
                Button("Subscribe")
                {
                    billing.purchasePremium()
                }
                .buttonStyle(.borderedProminent)
                .font(.title2)
                .padding(.top)
                .disabled(billing.purchaseResult == .some(.inFlow))
                
                Spacer()
            }
            
            if case .some(.inFlow) = billing.purchaseResult {
                loading
            } else if case .some(.failure) = billing.purchaseResult {
                error
            }

            
            Spacer()
        }
            .padding()
            .navigationTitle("Premium")
    }
        
    @ViewBuilder
    var usage: some View {
        VStack (alignment: .leading) {
            Text("Current Usage:")
            ColorProgressBar(value: settings.usageProgress)
        }
        .padding(.vertical)
    }
    
    @ViewBuilder
    var trial: some View {
        VStack(alignment: .leading) {
            Text("If you upgraded, your usage would be:")
            ColorProgressBar(value: settings.premiumProgress)
        }
    }
    
    @ViewBuilder
    var error: some View {
        HStack {
            Spacer()
            Text("Failed to complete purchase.")
                .padding(.vertical)
                .foregroundColor(.red)
            Spacer()
        }
    }
    
    @ViewBuilder
    var loading: some View {
        HStack {
            Spacer()
            ProgressView()
            Spacer()
        }
    }
}

struct ManageSubscription_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            ManageSubscription()
                .mockDI()
        }
    }
}
