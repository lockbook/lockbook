import SwiftUI
import SwiftLockbookCore

struct ImportAccountView: View {
    @ObservedObject var core: GlobalState
    @State var accountKey: String = ""
    @State var isScanning: Bool = false
    
    var body: some View {
        VStack(spacing: 40) {
            HStack {
                Text("Import an existing account")
                    .font(.title)
                    .bold()
                Spacer()
            }
            HStack {
                SecureField("Account String", text: self.$accountKey, onCommit: { handleImport() })
                    .disableAutocorrection(true)
                    .autocapitalization(.none)
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                Button(action: {
                    self.isScanning = true
                }) {
                    Image(systemName: "qrcode.viewfinder")
                }.frame(width: 40, height: 40)
            }
        }
        .padding(.horizontal)
        .sheet(isPresented: self.$isScanning, content: {
            CodeScannerView(codeTypes: [.qr], simulatedData: "This is simulated data", completion: handleScan)
        })
        
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
    
    func handleScan(result: Result<String, CodeScannerView.ScanError>) {
        self.isScanning = false
        switch result {
        case .success(let key):
            self.accountKey = key
        case .failure(let err):
            print(err) // TODO: Convert this to an ApplicationError
        }
    }
}

struct ImportView_Previews: PreviewProvider {
    static var previews: some View {
        HStack {
            ImportAccountView(core: GlobalState())
        }
    }
}
