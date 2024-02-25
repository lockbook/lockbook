import SwiftLockbookCore
import SwiftUI
import AlertToast

var buttonTextPadding = 16.0
var cornerRadius = 10.0

struct LargeButtonStyle: PrimitiveButtonStyle {
    var width: CGFloat
    var enabled: Bool
    
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .padding(.all, buttonTextPadding)
            .frame(width: width)
            .background(Color.blue)
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

struct LogoutButtonStyle: PrimitiveButtonStyle {
    var width: CGFloat
    var enabled: Bool

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .padding(.all, buttonTextPadding)
            .frame(width: width)
            .background(Color.red)
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
            .padding(.all, buttonTextPadding)
            .frame(width: width)
            .background(Color.gray)
            .foregroundColor(.white)
            .font(.headline)
            .cornerRadius(cornerRadius)
            .onTapGesture {
                configuration.trigger()
            }
    }
}

struct LogoutConfirmationView: View {
    var h1: CGFloat
    var h2: CGFloat
    var buttonWidth: CGFloat
    
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var account: AccountService
    
    @Environment(\.presentationMode) var presentationMode
    @State var backedUp = false
    @State var understandDelete = false
    @State var understandImportance = false

    var body: some View {
        VStack(alignment: .center) {
            Text("Delete lockbook files on this device?")
                .padding(.top, 32)
                .font(.system(size: h1, weight: .semibold, design: .default))
            Text("Tap all buttons to log out")
                .padding(.top, 8)
                .font(.system(size: h2, weight: .regular, design: .default))
            Button("My private key is saved somewhere safe") {
                backedUp = true
            }
            .buttonStyle(LargeButtonStyle(
                width: buttonWidth,
                enabled: true))
            .padding(.all, 8)
            .padding(.top, 32)
            .frame(width: buttonWidth)
            Button("I understand logout will delete my lockbook files on this device") {
                understandDelete = true
            }
            .buttonStyle(LargeButtonStyle(
                width: buttonWidth,
                enabled: backedUp))
            .padding(.all, 8)
            .disabled(!backedUp)
            .frame(width: buttonWidth)
            Button("I understand my files will NOT be recoverable if I lose my private key") {
                understandImportance = true
            }
            .buttonStyle(LargeButtonStyle(
                width: buttonWidth,
                enabled: backedUp && understandDelete))
            .padding(.all, 8)
            .disabled(!backedUp || !understandDelete)
            .frame(width: buttonWidth)
            Button("Logout") {
                DI.accounts.logout()
            }
            .buttonStyle(LogoutButtonStyle(
                width: buttonWidth,
                enabled: backedUp && understandDelete && understandImportance))
            .padding(.all, 8)
            .disabled(!backedUp || !understandDelete || !understandImportance)
            .frame(minWidth: buttonWidth)
            Button("Cancel") {
                dismiss()
            }
            .buttonStyle(CancelButtonStyle(
                width: buttonWidth))
            .padding(.all, 8)
            .frame(width: buttonWidth)
        }
    }
}
