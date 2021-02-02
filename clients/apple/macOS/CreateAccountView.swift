import SwiftUI

struct CreateAccountView: View {
    @ObservedObject var core: GlobalState
    @ObservedObject var createAccountState: OnboardingState
    
    var body: some View {
        VStack(spacing:40) {
            HStack {
                Text("Create a new account")
                    .font(.title)
                    .bold()
                Spacer()
            }
            HStack {
                TextField("Choose a username", text: $createAccountState.username)
                    .disableAutocorrection(true)
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                Button("Create", action: createAccountState.attemptCreate).buttonStyle(BorderedButtonStyle())
                    .disabled(createAccountState.working)
            }
            
            Text(createAccountState.createAccountError)
                .foregroundColor(.red)
                .bold()
            
        }
        .padding(.horizontal)
        .autocapitalization(.none)
    }
}
