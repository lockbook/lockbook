import Foundation
import SwiftUI
import SwiftWorkspace

struct ShareFileSheet: View {
    
    // MARK: Have to be updated manually whenever the view contents change. Vital for iPadOS and macOS
    #if os(iOS)
    static let FORM_WIDTH: CGFloat = 500
    static let FORM_HEIGHT: CGFloat = 365
    #else
    static let FORM_WIDTH: CGFloat = 500
    static let FORM_HEIGHT: CGFloat = 320
    #endif
        
    @Environment(\.colorScheme) var colorScheme
    @Environment(\.dismiss) private var dismiss
    
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    @StateObject var model: ShareFileViewModel
    
    let showExitButton: Bool
    
    init(id: UUID, name: String, shares: [Share], showExitButton: Bool) {
        self._model = StateObject(wrappedValue: ShareFileViewModel(id: id, name: name, shares: shares))
        self.showExitButton = showExitButton
    }
    
    var body: some View {
        VStack(spacing: 10) {
            HStack {
                Text("Share File")
                    .font(.title2)
                    .bold()
                
                Spacer()
                
                if showExitButton {
                    ExitSheetButton()
                }
            }
            
            HStack {
                Text("File:")
                    .font(.callout)
                
                Text(model.name)
                    .font(.system(.callout, design: .monospaced))
                
                Spacer()
            }
            
            textField
            
            Picker("Flavor", selection: $model.mode) {
                Text("Write").tag(ShareMode.write)
                Text("Read").tag(ShareMode.read)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            
            HStack {
                Text(model.error)
                    .foregroundStyle(.red)
                    .fontWeight(.bold)
                    .lineLimit(1, reservesSpace: true)
                
                Spacer()
            }
            
            Button {
                shareAndDismiss()
            } label: {
                Text("Share")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(model.username.isEmpty)
            
            HStack {
                Text("Share Permissions")
                    .bold()
                
                Spacer()
            }
            .padding(.top)
            
            HStack {
                Text(model.readAccessUsers.isEmpty ? "No users have read access." : "Read:")
                
                if !model.readAccessUsers.isEmpty {
                    ScrollView(.horizontal) {
                        HStack(spacing: 10) {
                            ForEach(model.readAccessUsers, id: \.self) { username in
                                Text(username)
                                    .padding(3)
                                    .cardBackground(background: userCardBackground)
                            }
                        }
                        .padding(.horizontal)
                    }
                } else {
                    Spacer()
                }
            }
            .frame(height: 25)
            
            HStack {
                Text(model.writeAccessUsers.isEmpty ? "No users have write access." : "Read and Write:")
                
                if !model.writeAccessUsers.isEmpty {
                    ScrollView(.horizontal) {
                        HStack(spacing: 10) {
                            ForEach(model.writeAccessUsers, id: \.self) { username in
                                Text(username)
                                    .padding(3)
                                    .cardBackground(background: userCardBackground)
                            }
                        }
                        .padding(.horizontal)
                    }
                } else {
                    Spacer()
                }
            }
            .frame(height: 25)
        }
        .padding(.horizontal)
        .padding(.top, 3)
    }
    
    @ViewBuilder
    var textField: some View {
        #if os(iOS)
        AutoFocusTextField(text: $model.username, placeholder: "Username", returnKeyType: .done, borderStyle: .roundedRect, autocorrect: false) {
            shareAndDismiss()
        }
        #else
        AutoFocusTextField(text: $model.username, placeholder: "Username", focusRingType: .none, isBordered: false) {
            shareAndDismiss()
        }
        #endif
    }
    
    var userCardBackground: Color {
        #if os(iOS)
        Color(UIColor.tertiarySystemBackground)
        #else
        colorScheme == .dark ? Color(nsColor: .windowBackgroundColor) : Color(nsColor: .controlBackgroundColor)
        #endif
    }
    
    func shareAndDismiss() {
        if model.shareFile(workspaceInput: workspaceInput) {
            dismiss()
        }
    }
}

class ShareFileViewModel: ObservableObject {
    let id: UUID
    let name: String
    let shares: [Share]
    let readAccessUsers: [String]
    let writeAccessUsers: [String]
        
    @Published var username: String = ""
    @Published var mode: ShareMode = .write
    @Published var error: String = ""
    
    init(id: UUID, name: String, shares: [Share]) {
        self.id = id
        self.name = name
        self.shares = shares
        self.readAccessUsers = shares.filter({ $0.mode == .read }).map({ $0.with })
        self.writeAccessUsers = shares.filter({ $0.mode == .write }).map({ $0.with })
    }

    func shareFile(workspaceInput: WorkspaceInputState) -> Bool {
        guard !name.isEmpty else {
            return false
        }
        
        let res = AppState.lb.shareFile(id: id, username: username, mode: mode)
        
        switch res {
        case .success():
            workspaceInput.requestSync()
            return true
        case .failure(let err):
            error = err.msg
            return false
        }
    }
}

struct ShareFileTextField: ViewModifier {
    func body(content: Content) -> some View {
        #if os(iOS)
        content
            .textFieldStyle(.roundedBorder)
        #else
        content
            .textFieldStyle(.plain)
        #endif
    }
}

#if os(iOS)
@available(iOS 17.0, *)
#Preview {
    @Previewable @State var file: File? = (AppState.lb as! MockLb).file1
    @Previewable @State var sheetHeight: CGFloat = 0
    
    Color.accentColor
        .optimizedSheet(
            item: $file,
            compactSheetHeight: $sheetHeight,
            width: ShareFileSheet.FORM_WIDTH,
            height: ShareFileSheet.FORM_HEIGHT,
            presentedContent: { item in
                ShareFileSheet(
                    id: item.id,
                    name: item.name,
                    shares: item.shares,
                    showExitButton: true
                )
            }
        )
}
#else
#Preview {
    ShareFileSheet(
        id: (AppState.lb as! MockLb).file1.id,
        name: (AppState.lb as! MockLb).file1.name,
        shares: (AppState.lb as! MockLb).file1.shares,
        showExitButton: true
    )
        .frame(width: ShareFileSheet.FORM_WIDTH, height: ShareFileSheet.FORM_HEIGHT
        )
}
#endif
