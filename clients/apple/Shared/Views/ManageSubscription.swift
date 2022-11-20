import SwiftUI
import AlertToast

struct ManageSubscription: View {    
    @EnvironmentObject var billing: BillingService
    @EnvironmentObject var settings: SettingsService
    
    @Environment(\.presentationMode) var presentationMode
        
    @State var isLoading = false
    @State var error = false
    
    var body: some View {
        VStack(alignment: .leading) {
            VStack (alignment: .leading) {
                Text("Current Usage:")
                ColorProgressBar(value: settings.usageProgress)
            }
            .padding(.vertical)
                
            switch settings.tier {
            case .Trial: trial
            case .Premium: trial
            case .Unknown: trial
            }
            
            Text("Expand your storage to **30GB** for just **2.99** a month.")
                .padding(.vertical)
            
            HStack {
                Spacer()
                Button("Subscribe")
                {
                    Task {
                        isLoading = true
                        let result = try await billing.purchasePremium()
                        if result == .failure {
                            error = true
                        } else if result == .success || result == .pending {
                            DispatchQueue.global(qos: .userInitiated).async {
                                Thread.sleep(forTimeInterval: 1)
                                DispatchQueue.main.async {
                                    presentationMode.wrappedValue.dismiss()
                                    billing.refreshSubscriptionStatus()
                                }
                            }
                        }
                        
                        isLoading = false
                        
                    }
                }
                .buttonStyle(.borderedProminent)
                .font(.title2)
                .padding(.top)
                .disabled(isLoading)
                
                Spacer()
            }
            
            if(isLoading) {
                HStack {
                    Spacer()
                    ProgressView()
                    Spacer()
                }
            }
            
            if(error) {
                HStack {
                    Spacer()
                    Text("Failed to complete purchase.")
                        .padding(.vertical)
                        .foregroundColor(.red)
                    Spacer()
                }
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
    var trial: some View {
        VStack(alignment: .leading) {
            Text("If you upgraded, your usage would be:")
            ColorProgressBar(value: settings.premiumProgress)
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
