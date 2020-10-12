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
        print("Trying to create \(username)")
        let res = self.core.api
            .createAccount(username: self.username, apiLocation: ConfigHelper.get(.apiLocation))
            .eraseError()
            .flatMap { _ in
                self.core.api.getAccount().eraseError()
            }
        
        print("Result \(res)")
        
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
