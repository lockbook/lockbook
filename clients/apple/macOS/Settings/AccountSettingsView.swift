import SwiftUI
import SwiftLockbookCore

struct AccountSettingsView: View {
    
    let account: Account
    
    @EnvironmentObject var settings: SettingsService
    
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
                    Button(action: settings.copyAccountString, label: {
                        Text(settings.copyToClipboardText)
                    }).frame(maxWidth: .infinity, alignment: .leading)
                    
                    Button(action: {codeRevealed.toggle()}, label: {
                        Text(qrCodeText)
                    }).frame(maxWidth: .infinity, alignment: .leading)
                    
                }.frame(maxWidth: .infinity, alignment: .leading)
            }
            
            if codeRevealed {
                settings.accountCode()
            }
        }.padding(20)
    }
}
