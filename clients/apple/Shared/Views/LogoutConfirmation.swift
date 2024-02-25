import SwiftLockbookCore
import SwiftUI
import AlertToast

var cornerRadius = 10.0
var largeButtonWidth = 512.0

struct LargeButtonStyle: PrimitiveButtonStyle {
    var width: CGFloat
    var enabled: Bool
    
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .padding()
            .frame(width: width)
            .background(enabled ? Color.blue : Color.gray)
            .foregroundColor(.white)
            .font(.headline)
            .cornerRadius(cornerRadius)
            .opacity(enabled ? 1 : 0.5)
            .onTapGesture {
                if enabled {
                    configuration.trigger()
                }
            }
    }
}

struct CancelButtonStyle: PrimitiveButtonStyle {
    var width: CGFloat

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .padding()
            .frame(width: width)
            .background(Color.red)
            .foregroundColor(.white)
            .font(.headline)
            .cornerRadius(cornerRadius)
            .onTapGesture {
                configuration.trigger()
            }
    }
}

struct LogoutConfirmationView: View {
    @Environment(\.openURL) var openURL
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var account: AccountService
    
    @Environment(\.presentationMode) var presentationMode
    @State var backedUp = false
    @State var understandDelete = false
    @State var understandImportance = false
    
    func resetState() {
        backedUp = false
        understandDelete = false
        understandImportance = false
    }
    
    var body: some View {
        VStack(alignment: .center) {
            Text("Delete lockbook files on this device and log out?")
                .padding()
                .font(/*@START_MENU_TOKEN@*/.title/*@END_MENU_TOKEN@*/)
            Button("My private key is saved somewhere safe") {
                backedUp = true
            }
            .buttonStyle(LargeButtonStyle(
                width: largeButtonWidth,
                enabled: true))
            .padding()
            .frame(width: largeButtonWidth)
            Button("I understand logout will delete my lockbook files on this device") {
                understandDelete = true
            }
            .buttonStyle(LargeButtonStyle(
                width: largeButtonWidth,
                enabled: backedUp))
            .padding()
            .disabled(!backedUp)
            .frame(width: largeButtonWidth)
            Button("I understand my files will NOT be recoverable if I lose my private key") {
                understandImportance = true
            }
            .buttonStyle(LargeButtonStyle(
                width: largeButtonWidth,
                enabled: backedUp && understandDelete))
            .padding()
            .disabled(!backedUp || !understandDelete)
            .frame(width: largeButtonWidth)
            Button("Logout") {
                resetState()
                DI.accounts.logout()
            }
            .buttonStyle(LargeButtonStyle(
                width: largeButtonWidth,
                enabled: backedUp && understandDelete && understandImportance))
            .padding()
            .disabled(!backedUp || !understandDelete || !understandImportance)
            .frame(minWidth: largeButtonWidth)
            Button("Cancel") {
                resetState()
                dismiss()
            }
            .buttonStyle(CancelButtonStyle(
                width: largeButtonWidth))
            .padding()
            .frame(width: largeButtonWidth)
        }
        .onAppear {
            resetState()
        }
        .onDisappear {
            resetState()
        }
    }
}
