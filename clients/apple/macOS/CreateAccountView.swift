import SwiftUI

struct CreateAccountView: View {
    @EnvironmentObject var onboardingState: OnboardingService
    
    var body: some View {
        VStack(spacing:40) {
            HStack {
                Text("Create account")
                    .font(.title2)
                Spacer()
            }
            HStack {
                TextField("Choose a username", text: $onboardingState.username)
                    .disableAutocorrection(true)
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                Button("Create", action: onboardingState.attemptCreate).buttonStyle(BorderedButtonStyle())
                    .disabled(onboardingState.working)
            }
            
            Text(onboardingState.createAccountError)
                .foregroundColor(.red)
                .bold()
            
        }
        .padding(.horizontal)
        .autocapitalization(.none)
    }
}
