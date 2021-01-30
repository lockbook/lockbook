import SwiftUI

struct CreateAccountView: View {
    @ObservedObject var core: GlobalState
    @State var username: String = ""
    
    var body: some View {
        VStack(spacing:40) {
            HStack {
                Text("Create a new account")
                    .font(.title)
                    .bold()
                Spacer()
            }
            HStack {
                TextField("Choose a username", text: self.$username)
                    .disableAutocorrection(true)
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                Button("Create", action: handleCreate).buttonStyle(BorderedButtonStyle())
            }
        }
        .padding(.horizontal)
        .autocapitalization(.none)
    }
    
    func handleCreate() {
        let res = self.core.api
            .createAccount(username: self.username, apiLocation: ConfigHelper.get(.apiLocation))
            .eraseError()
            .flatMap { _ in
                self.core.api.getAccount().eraseError()
            }
        
        switch res {
        case .success(let acc):
            self.core.account = acc
        case .failure(let err):
            hideKeyboard()
            self.core.handleError(err)
        }
    }
}

struct WithoutNavigationView: PreviewProvider {
    static var previews: some View {
        VStack {
            CreateAccountView(core: GlobalState())
        }
    }
}
