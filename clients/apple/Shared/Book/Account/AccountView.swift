import SwiftUI
import SwiftLockbookCore

struct AccountView: View {
    @ObservedObject var core: Core
    let account: Account
    @State var showingCode: Bool = false
    @State var copiedString: Bool?
    @Environment(\.presentationMode) var presentationMode

    fileprivate func hideMessage() {
        withAnimation { copiedString = nil }
    }
    
    var body: some View {
        VStack(spacing: 50) {
            Text("\(account.username)'s Account")
                .font(.title)
            GroupBox(label: Text("Account String").padding(.bottom, 20)) {
                VStack(spacing: 20) {
                    Button(action: { showingCode.toggle() }) {
                        Label("Show QR Code", systemImage: "qrcode")
                    }
                    HStack {
                        Button(action: copyAccountString ) {
                            Label("Copy to Clipboard", systemImage: "pencil.and.ellipsis.rectangle")
                        }
                        copiedString.map { b in
                            Button(action: hideMessage ) {
                                if (b) {
                                    Label("Copied!", systemImage: "checkmark.square").foregroundColor(.green)
                                } else {
                                    Label("Failed", systemImage: "exclamationmark.square").foregroundColor(.red)
                                }
                            }
                            .onAppear { DispatchQueue.main.asyncAfter(deadline: .now() + 4, execute: hideMessage) }
                        }
                    }
                }
            }
            GroupBox(label: Text("Debug").padding(.bottom, 20)) {
                Button(action: purgeAndLogout) {
                    Label("Purge and Logout", systemImage: "person.crop.circle.badge.xmark")
                        .foregroundColor(.red)
                }
            }
        }
        .sheet(isPresented: $showingCode, content: {
            VStack {
                if let code = accountCode(), let cgCode = CIContext().createCGImage(code, from: code.extent) {
                    Image(cgCode, scale: 1.0, label: Text(""))
                } else {
                    Label("Could not export account!", systemImage: "person.crop.circle.badge.exclam")
                        .padding()
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
    
    func copyAccountString() {
        withAnimation {
            switch core.api.exportAccount() {
            case .success(let accountString):
                #if os(iOS)
                UIPasteboard.general.string = accountString
                #else
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(accountString, forType: .string)
                #endif
                copiedString = true
            case .failure(let err):
                copiedString = false
                core.displayError(error: err)
            }
        }
    }
    
    func purgeAndLogout() {
        presentationMode.wrappedValue.dismiss()
        DispatchQueue.global(qos: .userInteractive).async { core.self.purge() }
    }
}

struct AccountView_Previews: PreviewProvider {
    static var previews: some View {
        AccountView(core: Core(), account: Account(username: "test"))
    }
}
