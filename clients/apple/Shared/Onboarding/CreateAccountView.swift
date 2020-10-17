import SwiftUI

struct CreateAccountView: View {
    @ObservedObject var core: Core
    @State var username: String = ""
    
    var body: some View {
        let view = VStack(spacing: 40) {
            TextField("Username", text: self.$username)
                .disableAutocorrection(true)
                .textFieldStyle(RoundedBorderTextFieldStyle())
            Button(action: handleCreate, label: {
                Label("Create", systemImage: "person.crop.circle.badge.plus")
            })
        }
        .padding(.horizontal)

        #if os(iOS)
        return view
            .autocapitalization(.none)
        #else
        return view
        #endif
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

struct CreateAccountView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            CreateAccountView(core: Core())
        }
    }
}
