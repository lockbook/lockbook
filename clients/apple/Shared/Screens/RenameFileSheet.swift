import SwiftUI
import SwiftWorkspace

struct RenameFileSheet: View {
    // MARK: Have to be updated manually whenever the view contents change. Vital for iPadOS and macOS
    static let FORM_WIDTH: CGFloat = 420
    static let FORM_HEIGHT: CGFloat = 190

    @StateObject var model: RenameFileViewModel
    @Environment(\.dismiss) private var dismiss
    
    init(homeState: HomeState, workspaceState: WorkspaceState, id: UUID, name: String) {
        self._model = StateObject(wrappedValue: RenameFileViewModel(homeState: homeState, workspaceState: workspaceState, id: id, name: name))
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
                
                Text(model.parentPath ?? "...")
                    .lineLimit(2)
                    .font(.system(.callout, design: .monospaced))
                
                Spacer()
            }
            
            textField
                                    
            HStack {
                Text(model.error)
                    .foregroundStyle(.red)
                    .fontWeight(.bold)
                    .lineLimit(1, reservesSpace: true)
                
                Spacer()
            }
                                    
            Button {
                renameAndDismiss()
            } label: {
                Text("Rename")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(model.name == model.newName)

        }
        .padding(.horizontal)
        .padding(.top, 3)
    }
    
    @ViewBuilder
    var textField: some View {
        #if os(iOS)
        AutoFocusTextField(text: $model.newName, placeholder: "File name", returnKeyType: .done, borderStyle: .roundedRect) {
            renameAndDismiss()
        }
        #else
        AutoFocusTextField(text: $model.newName, placeholder: "File name", focusRingType: .none, isBordered: false) {
            renameAndDismiss()
        }
        #endif
    }
    
    func renameAndDismiss() {
        if model.renameFile() {
            dismiss()
        }
    }
}

class RenameFileViewModel: ObservableObject {
    @Published var newName: String
    @Published var error: String = ""
    @Published var parentPath: String? = nil
    
    let workspaceState: WorkspaceState
    
    let id: UUID
    let name: String
    
    init(homeState: HomeState, workspaceState: WorkspaceState, id: UUID, name: String) {
        self.workspaceState = workspaceState
        self.id = id
        self.name = name
        self._newName = Published(initialValue: name)
        
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.getPathById(id: id)
            
            DispatchQueue.main.async {
                switch res {
                case .success(let path):
                    self.parentPath = path
                case .failure(let err):
                    homeState.error = .lb(error: err)
                }
            }
        }
    }
    
    func renameFile() -> Bool {
        guard name != newName else {
            return true
        }
        
        let res = AppState.lb.renameFile(id: id, newName: newName)
        
        switch res {
        case .success(_):
            workspaceState.fileOpCompleted = .Rename(id: id, newName: newName)
            return true
        case .failure(let err):
            error = err.msg
            return false
        }
    }
}

#if os(iOS)
@available(iOS 17.0, *)
#Preview {
    @Previewable @State var file: File? = (AppState.lb as! MockLb).file1
    
    Color.accentColor
        .optimizedSheet(
            item: $file,
            constrainedSheetHeight: .constant(200),
            width: RenameFileSheet.FORM_WIDTH,
            height: RenameFileSheet.FORM_HEIGHT,
            presentedContent: { item in
                RenameFileSheet(
                    homeState: HomeState(),
                    workspaceState: WorkspaceState(),
                    id: item.id,
                    name: item.name
                )
            }
        )
}
#endif
