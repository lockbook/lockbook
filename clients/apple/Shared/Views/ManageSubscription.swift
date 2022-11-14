import SwiftUI

struct ManageSubscription: View {
    
    @EnvironmentObject var settingsState: SettingsService
    @EnvironmentObject var billingService: BillingService
    
    @Environment(\.presentationMode) var presentationMode
    
    @State var isLoading = false
    
    var body: some View {
        VStack(alignment: .leading) {
            VStack (alignment: .leading) {
                Text("Current Usage:")
                ColorProgressBar(value: settingsState.usageProgress)
            }
            .padding(.vertical)
                
            switch settingsState.tier {
            case .Trial: trial
            case .Premium: trial
            case .Unknown: trial
            }
            
            Text("Expand your storage to **30GB** for just **2.99** a month.")
                .padding(.vertical)
            
            HStack {
                Spacer()
                Button("Subscribe") {
//                    presentationMode.wrappedValue.dismiss()
                    
                    Task {
                        try await billingService.purchasePremium()!
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
            
            Spacer()
        }.padding()
            .navigationTitle("Premium")
    }
    
    @ViewBuilder
    var trial: some View {
        VStack(alignment: .leading) {
            Text("If you upgraded, your usage would be:")
            ColorProgressBar(value: settingsState.premiumProgress)
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
