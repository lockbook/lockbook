import SwiftUI
import SwiftLockbookCore

struct ImportAccountView: View {
    @ObservedObject var core: GlobalState
    @ObservedObject var onboardingState: OnboardingState
    
    var body: some View {
        VStack(spacing: 40) {
            HStack {
                Text("Import an existing account")
                    .font(.title)
                    .bold()
                Spacer()
            }
            HStack {
                SecureField("Account String", text: $onboardingState.accountString)
                    .disableAutocorrection(true)
                    .autocapitalization(.none)
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                Button("Import", action: onboardingState.handleImport)
                    .buttonStyle(BorderedButtonStyle())
                    .disabled(onboardingState.working)
            }
            
            Text(onboardingState.importAccountError)
                .foregroundColor(.red)
                .bold()
        }
        .padding(.horizontal)
        
    }
}
