import SwiftUI

struct CreateAccountView: View {
    @ObservedObject var core: Core
    @State var username: String = ""
    
    var body: some View {
        VStack {
            TextField("Username", text: self.$username)
                .disableAutocorrection(true)
                .padding(.all, 40)
            Button(action: handleCreate, label: {
                Label("Create", systemImage: "person.crop.circle.badge.plus")
            })
        }
    }
    
    func handleCreate() {
        switch self.core.api.createAccount(username: self.username, apiLocation: ConfigHelper.get(.apiLocation)) {
        case .success(let acc):
            self.core.account = acc
        case .failure(let err):
            hideKeyboard()
            self.core.displayError(error: err)
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
