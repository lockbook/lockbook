import SwiftLockbookCore
import SwiftUI
import AlertToast

struct LogoutConfirmationView: View {
    @Environment(\.openURL) var openURL
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var account: AccountService
    
    @Environment(\.presentationMode) var presentationMode
    @State var backedUp = false
    @State var understandDelete = false
    @State var understandImportance = false
            
    var body: some View {
        VStack(alignment: .leading) {
            HStack {
                Spacer()
                Text("Delete lockbook files on this device and log out?")
                    .padding(.vertical)
                Spacer()
            }
            HStack {
                Spacer()
                Button("My private key is saved somewhere safe") {
                    backedUp = true
                }
                .buttonStyle(.borderedProminent)
                .padding(.top)
                Spacer()
            }
            HStack {
                Spacer()
                Button("I understand log out will delete my lockbook files on this device") {
                    understandDelete = true
                }
                .buttonStyle(.borderedProminent)
                .padding(.top)
                .disabled(!backedUp)
                Spacer()
            }
            HStack {
                Spacer()
                Button("I understand my files will NOT be recoverable if I lose my private key") {
                    understandImportance = true
                }
                .buttonStyle(.borderedProminent)
                .padding(.top)
                .disabled(!backedUp || !understandDelete)
                Spacer()
            }
            HStack {
                Spacer()
                Button("Logout") {
                    DI.accounts.logout()
                }
                .buttonStyle(.borderedProminent)
                .padding(.vertical)
                .disabled(!backedUp || !understandDelete || !understandImportance)
                Spacer()
            }
            HStack {
                Spacer()
                Button("Cancel") {
                    // TODO: do this when pressing X to close window also?
                    backedUp = false
                    understandDelete = false
                    understandImportance = false
                    dismiss()
                }
                .padding(.vertical)
                Spacer()
            }
        }
    }
}
