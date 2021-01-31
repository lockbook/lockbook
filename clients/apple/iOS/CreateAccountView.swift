import SwiftUI

struct CreateAccountView: View {
    @ObservedObject var core: GlobalState
    @ObservedObject var onboardingState: OnboardingState
    
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
    
    static var onboardingState = OnboardingState(core: GlobalState())
    static var previews: some View {
        VStack {
            CreateAccountView(core: GlobalState(), onboardingState: OnboardingState(core: GlobalState()))
        }
    }
}

struct WithoutNavigationViewWithError: PreviewProvider {
    
    static var onboardingState = OnboardingState(core: GlobalState())
    static var previews: some View {
        VStack {
            CreateAccountView(core: GlobalState(), onboardingState: onboardingState)
                .onAppear {
                    onboardingState.createAccountError = "An error occurred"
                }
        }
    }
}
