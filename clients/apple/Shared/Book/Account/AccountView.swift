import SwiftUI
import SwiftLockbookCore

struct AccountView: View {
    @ObservedObject var core: Core
    let account: Account
    @State var showingUsage: Bool = false
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
            GroupBox(label: Text("Account").padding(.bottom, 20)) {
                VStack(spacing: 20) {
                    Button(action: { showingCode.toggle() }) {
                        Label("Show QR Code", systemImage: "qrcode")
                    }
                    NotificationButton(
                        action: copyAccountString,
                        label: Label("Copy String to Clipboard", systemImage: "pencil.and.ellipsis.rectangle"),
                        successLabel: Label("Copied!", systemImage: "checkmark.square"),
                        failureLabel: Label("Failed", systemImage: "exclamationmark.square")
                    )
                    DisclosureGroup(
                        isExpanded: $showingUsage,
                        content: { () -> AnyView in
                            let usages = (try? core.api.getUsage().get()) ?? []
                            let bytes = usages.map { $0.byteSecs }.reduce(0, +)
                            return AnyView(UsageIndicator(numerator: bytes*8/10, denominator: bytes, suffix: "Bytes").foregroundColor(.accentColor))
                        },
                        label: {
                            HStack {
                                Spacer()
                                Button(action: {
                                    withAnimation(.linear) { showingUsage.toggle() }
                                }, label: {
                                    Label("Current Usage", systemImage: "circle.grid.hex.fill")
                                })
                                Spacer()
                            }
                        }
                    )
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
    
    func copyAccountString() -> Result<Void, Error> {
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
                return .success(())
            case .failure(let err):
                copiedString = false
                core.displayError(error: err)
                return .failure(err)
            }
        }
    }
    
    func getUsage() {
        showingUsage = true
<<<<<<< HEAD
=======
//        switch core.api.getUsage() {
//        case .success(let usage):
//            usage.forEach { print("\($0.fileId): \($0.byteSecs)/\($0.secs) byte/secs") }
//        case .failure(let err):
//            core.displayError(error: err)
//        }
>>>>>>> 9c94ee1d... working commit of some usage stuff
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
