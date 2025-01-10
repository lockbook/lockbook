import SwiftUI
import SwiftWorkspace

struct RenameFileSheet: View {
    let id: UUID
    let name: String
    let parentPath: String
    
    @State var newName: String = ""
    @State var error: String = ""
    
    @EnvironmentObject var workspaceState: WorkspaceState
    @Environment(\.dismiss) private var dismiss
    
    init(id: UUID, name: String, parentPath: String) {
        self.id = id
        self.name = name
        self.parentPath = parentPath
        self._newName = State(initialValue: name)
    }
    
    var body: some View {
        VStack(spacing: 10) {
            HStack {
                Text("Rename File")
                    .bold()
                
                Spacer()
            }
            
            HStack {
                Text("Parent Folder:")
                    .font(.callout)
                
                Text(parentPath)
                    .lineLimit(2)
                    .font(.system(.callout, design: .monospaced))
                
                Spacer()
            }
            
            textField
                                    
            HStack {
                Text(error)
                    .foregroundStyle(.red)
                    .fontWeight(.bold)
                    .lineLimit(1, reservesSpace: true)
                
                Spacer()
            }
                                    
            Button {
                renameFile()
            } label: {
                Text("Rename")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(name == newName)

        }
        .padding(.horizontal)
        .padding(.top, 3)
    }
    
    @ViewBuilder
    var textField: some View {
        #if os(iOS)
        AutoFocusTextField(text: $newName, placeholder: "File name", returnKeyType: .done, borderStyle: .roundedRect) {
            guard name != newName else {
                dismiss()
                return
            }
            
            renameFile()
        }
        #else
        AutoFocusTextField(text: $newName, placeholder: "File name", focusRingType: .none, isBordered: false) {
            guard name != newName else {
                dismiss()
                return
            }
            
            renameFile()
        }
        #endif
    }
    
    func renameFile() {
        let res = AppState.lb.renameFile(id: id, newName: newName)
        
        switch res {
        case .success(_):
            workspaceState.fileOpCompleted = .Rename(id: id, newName: newName)
            dismiss()
        case .failure(let err):
            error = err.msg
        }
    }
}
