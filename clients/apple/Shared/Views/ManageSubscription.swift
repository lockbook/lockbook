import SwiftUI
import AlertToast

struct ManageSubscription: View {    
    @EnvironmentObject var billing: BillingService
    @EnvironmentObject var settings: SettingsService
    
    @Environment(\.presentationMode) var presentationMode
        
    @State var isPurchasing = false
    @State var hasPurchaseError = false
    
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
                    Task {
                        isPurchasing = true
                        let result = try await billing.purchasePremium()
                        if result == .failure {
                            hasPurchaseError = true
                        } else if result == .success || result == .pending {
                            settings.calculateUsage()
                            
                            DispatchQueue.global(qos: .userInitiated).async {
                                Thread.sleep(forTimeInterval: 2)
                                DispatchQueue.main.async {
                                    presentationMode.wrappedValue.dismiss()
                                }
                            }
                        }
                        
                        isPurchasing = false
                    }
                }
                .buttonStyle(.borderedProminent)
                .font(.title2)
                .padding(.top)
                .disabled(isPurchasing)
                
                Spacer()
            }
            
            if(isPurchasing) {
                loading
            }
            
            if(hasPurchaseError) {
                error
            }
            
            Spacer()
        }
            .padding()
            .navigationTitle("Premium")
            .toast(isPresenting: Binding(get: { billing.purchaseResult != nil }, set: { _ in billing.purchaseResult = nil }), duration: 2, tapToDismiss: true) {
                purchaseToast()
            }
    }
    
    func purchaseToast() -> AlertToast {
        if let result = billing.purchaseResult {
            switch result {
            case .success:
                return AlertToast(type: .regular, title: "You have successfully purchased premium!")
            case .pending:
                return AlertToast(type: .regular, title: "Your purchase is pending.")
            case .failure:
                return AlertToast(type: .regular, title: "ERROR")
            }
        } else {
            return AlertToast(type: .regular, title: "ERROR")
        }
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
