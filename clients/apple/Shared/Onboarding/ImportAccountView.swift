import SwiftUI
import SwiftLockbookCore

struct ImportAccountView: View {
    @ObservedObject var core: Core
    @State var accountKey: String = ""
    @State var isScanning: Bool = false
    
    var body: some View {
        VStack {
            #if os(iOS)
            Button(action: {
                self.isScanning = true
            }) {
                Label("Scan", systemImage: "qrcode.viewfinder")
            }
            #endif
            TextField("Account String", text: self.$accountKey)
                .disableAutocorrection(true)
                .padding(.all, 40)
            NotificationButton(
                action: handleImport,
                label: Label("Import", systemImage: "rectangle.stack.person.crop"),
                successLabel: Label("Imported!", systemImage: "checkmark.square"),
                failureLabel: Label("Failure", systemImage: "exclamationmark.square")
            )
        }
        .sheet(isPresented: self.$isScanning, content: {
            #if os(iOS)
            CodeScannerView(codeTypes: [.qr], simulatedData: "OOF", completion: handleScan)
            #endif
        })
    }
    
    func handleImport() -> Result<Void, Error> {
        let res = self.core.api.importAccount(accountString: self.accountKey)
            .eraseError()
            .flatMap(transform: { _ in self.core.api.getAccount().eraseError() })
        switch res {
        case .success(let acc):
            self.core.account = acc
            self.core.sync()
            return .success(())
        case .failure(let err):
            hideKeyboard()
            self.core.handleError(err)
            return .failure(err)
        }
    }
    
    #if os(iOS)
    func handleScan(result: Result<String, CodeScannerView.ScanError>) {
        self.isScanning = false
        switch result {
        case .success(let key):
            self.accountKey = key
        case .failure(let err):
            print(err) // TODO: Convert this to an ApplicationError
        }
    }
    #endif
}

struct ImportView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            ImportAccountView(core: Core())
        }
    }
}
