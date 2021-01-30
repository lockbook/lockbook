import SwiftUI
import SwiftLockbookCore

struct ImportAccountView: View {
    @ObservedObject var core: GlobalState
    @State var accountKey: String = ""
    
    var body: some View {
        VStack(spacing: 40) {
            HStack {
                Text("Import an existing account")
                    .font(.title)
                    .bold()
                Spacer()
            }
            HStack {
                SecureField("Account String", text: self.$accountKey)
                    .disableAutocorrection(true)
                    .autocapitalization(.none)
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                Button("Import", action: { handleImport() }).buttonStyle(BorderedButtonStyle())
            }
        }
        .padding(.horizontal)
        
    }
    
    func handleImport() -> Result<Void, Error> {
        let res = self.core.api.importAccount(accountString: self.accountKey)
            .eraseError()
            .flatMap(transform: { _ in self.core.api.getAccount().eraseError() })
        switch res {
        case .success(let acc):
            self.core.account = acc
            self.core.syncing = true
            return .success(())
        case .failure(let err):
            hideKeyboard()
            self.core.handleError(err)
            return .failure(err)
        }
    }
}
