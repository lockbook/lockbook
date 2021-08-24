import SwiftUI

struct CreateAccountView: View {
    @EnvironmentObject var onboardingState: OnboardingState
    
    var body: some View {
        VStack(spacing:40) {
            HStack {
                Text("Create a new account")
                    .font(.title)
                    .bold()
                Spacer()
            }
            TextField("Choose a username", text: self.$onboardingState.username, onCommit: self.onboardingState.attemptCreate)
            .disableAutocorrection(true)
            .textFieldStyle(RoundedBorderTextFieldStyle())
            
            Text(onboardingState.createAccountError)
                .foregroundColor(.red)
                .bold()
        }
        .padding(.horizontal)
        .autocapitalization(.none)
    }
}

struct WithoutNavigationView: PreviewProvider {
    
    static var previews: some View {
        VStack {
            CreateAccountView()
                .mockDI()
        }
    }
}

struct WithoutNavigationViewWithError: PreviewProvider {
    
    static var previews: some View {
        VStack {
            CreateAccountView()
                .mockDI()
                .onAppear {
                    Mock.onboarding.createAccountError = "An error occurred"
                }
        }
    }
}
