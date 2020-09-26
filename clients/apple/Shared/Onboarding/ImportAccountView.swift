import SwiftUI

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
            Button(action: handleImport, label: {
                Label("Import", systemImage: "rectangle.stack.person.crop")
            })
        }
        .sheet(isPresented: self.$isScanning, content: {
            #if os(iOS)
            CodeScannerView(codeTypes: [.qr], simulatedData: "OOF", completion: handleScan)
            #endif
        })
    }
    
    func handleImport() {
        switch self.core.api.importAccount(accountString: self.accountKey) {
        case .success(let acc):
            self.core.account = acc
            self.core.sync()
        case .failure(let err):
            hideKeyboard()
            self.core.displayError(error: err)
        }
    }
    
    #if os(iOS)
    func handleScan(result: Result<String, CodeScannerView.ScanError>) {
        self.isScanning = false
        switch result {
        case .success(let key):
            self.accountKey = key
        case .failure(let err):
            print(err)
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
