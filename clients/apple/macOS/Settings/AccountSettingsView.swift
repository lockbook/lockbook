import SwiftUI
import SwiftLockbookCore

struct AccountSettingsView: View {
    
    @ObservedObject var core: GlobalState
    @ObservedObject var settingsState: SettingsState
    let account: Account
    
    // MARK: Copy Button Things
    @State var copied: Bool = false {
        didSet {
            DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
                self.copied = false
            }
        }
    }
    var copyToClipboardText: String {
        if copied {
            return "Copied"
        } else {
            return "Copy to clipboard"
        }
    }
    
    // MARK: QR Code things
    @State var codeRevealed: Bool = false
    var qrCodeText: String {
        if codeRevealed {
            return "Hide QR Code"
        } else {
            return "Reveal as QR"
        }
    }
    
    var body: some View {
        VStack (spacing: 20){
            HStack (alignment: .top) {
                Text("Username:")
                    .frame(maxWidth: 175, alignment: .trailing)
                Text(account.username)
                    .font(.system(.body, design: .monospaced))
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            HStack (alignment: .top) {
                Text("Server Location:")
                    .frame(maxWidth: 175, alignment: .trailing)
                Text(account.apiUrl)
                    .font(.system(.body, design: .monospaced))
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            HStack (alignment: .top) {
                Text("Account Secret:")
                    .frame(maxWidth: 175, alignment: .trailing)
                VStack {
                    Button(action: copyAccountString, label: {
                        Text(copyToClipboardText)
                    }).frame(maxWidth: .infinity, alignment: .leading)
                    
                    Button(action: {codeRevealed.toggle()}, label: {
                        Text(qrCodeText)
                    }).frame(maxWidth: .infinity, alignment: .leading)
                    
                }.frame(maxWidth: .infinity, alignment: .leading)
            }
            
            if codeRevealed {
                settingsState.accountCode()
            }
        }.padding(20)
    }
    
    func copyAccountString() {
        switch core.api.exportAccount() {
        case .success(let accountString):
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(accountString, forType: .string)
            copied = true
        case .failure(let err):
            core.handleError(err)
        }
    }

}
