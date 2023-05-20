import SwiftUI
import AlertToast

struct ManageSubscription: View {
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
                .padding(.top)
                .disabled(billing.purchaseResult == .some(.inFlow))
                
                Spacer()
            }
            
            if case .some(.inFlow) = billing.purchaseResult {
                loading
            } else if case .some(.failure) = billing.purchaseResult {
                error
            }

            legal
        }
        .padding(.horizontal)
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
                    DispatchQueue.global(qos: .userInitiated).async {
                        Thread.sleep(forTimeInterval: 2)
                        DispatchQueue.main.async {
                            
                            presentationMode.wrappedValue.dismiss()
                        }
                    }
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
    var usage: some View {
        VStack (alignment: .leading) {
            Text("Current Usage:")
            ColorProgressBar(value: settings.usageProgress)
        }
        .padding(.vertical, 15)
    }
    
    @ViewBuilder
    var trial: some View {
        VStack(alignment: .leading) {
            Text("If you upgraded, your usage would be:")
            ColorProgressBar(value: settings.premiumProgress)
        }
        .padding(.bottom, 10)
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

struct ManageSubscription_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            ManageSubscription()
                .mockDI()
        }
    }
}
