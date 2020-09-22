import SwiftUI
import SwiftLockbookCore

struct AccountView: View {
    @ObservedObject var core: Core
    let account: Account
    @State var showingCode: Bool = false
    @State var copiedString: Bool = false
    
    var body: some View {
        VStack {
            Button(action: { showingCode.toggle() }) {
                Label("Account Code", systemImage: "qrcode")
            }
            .padding()
            Button(action: {
                switch core.api.exportAccount() {
                case .success(let accountString):
                    #if os(iOS)
                    UIPasteboard.general.string = accountString
                    #else
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(accountString, forType: .string)
                    #endif
                    copiedString.toggle()
                case .failure(let err):
                    core.displayError(error: err)
                }
            }) {
                Label("Account String", systemImage: "pencil.and.ellipsis.rectangle")
            }
            .alert(isPresented: $copiedString, content: {
                Alert(title: Text("Copied account string to clipboard!"))
            })
            .padding()
            Button(action: self.core.purge) {
                Label("Purge Account", systemImage: "person.crop.circle.badge.xmark")
            }
            .padding()
        }
        .navigationTitle("\(account.username)")
        .sheet(isPresented: $showingCode, content: {
            VStack {
                if let code = accountCode(), let cgCode = CIContext().createCGImage(code, from: code.extent) {
                    Image(cgCode, scale: 1.0, label: Text(""))
                } else {
                    Image(systemName: "person.crop.circle.badge.exclam")
                }
                Button("Dismiss", action: { showingCode.toggle() })
            }
        })
    }
    
    func accountCode() -> CIImage? {
        switch core.api.exportAccount() {
        case .success(let accountString):
            let data = accountString.data(using: String.Encoding.ascii)
            if let filter = CIFilter(name: "CIQRCodeGenerator") {
                filter.setValue(data, forKey: "inputMessage")
                let transform = CGAffineTransform(scaleX: 3, y: 3)
                if let output = filter.outputImage?.transformed(by: transform) {
                    return output
                }
            }
        case .failure(let err):
            core.displayError(error: err)
        }
        return nil
    }
}

struct AccountView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            AccountView(core: Core(), account: Account(username: "test"))
        }
    }
}
