import SwiftUI
import SwiftWorkspace

struct RenameFileSheet: View {
    // MARK: Have to be updated manually whenever the view contents change. Vital for iPadOS and macOS
    #if os(iOS)
    static let FORM_WIDTH: CGFloat = 420
    static let FORM_HEIGHT: CGFloat = 190
    #else
    static let FORM_WIDTH: CGFloat = 420
    static let FORM_HEIGHT: CGFloat = 150
    #endif
    
    @StateObject var model: RenameFileViewModel
    @Environment(\.dismiss) private var dismiss
    
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    
    init(homeState: HomeState, id: UUID, name: String) {
        self._model = StateObject(wrappedValue: RenameFileViewModel(homeState: homeState, id: id, name: name))
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
        if model.renameFile(workspaceInput: workspaceInput) {
            dismiss()
        }
    }
}

class RenameFileViewModel: ObservableObject {
    @Published var newName: String
    @Published var error: String = ""
    @Published var parentPath: String? = nil
        
    let id: UUID
    let name: String
    
    init(homeState: HomeState, id: UUID, name: String) {
        self.id = id
        self.name = name
        self._newName = Published(initialValue: name)
        
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.getPathById(id: id)
            
            DispatchQueue.main.async {
                switch res {
                case .success(let path):
                    self.parentPath = path.nameAndPath().1
                case .failure(let err):
                    AppState.shared.error = .lb(error: err)
                }
            }
        }
    }
    
    func renameFile(workspaceInput: WorkspaceInputState) -> Bool {
        guard name != newName else {
            return true
        }
        
        let res = AppState.lb.renameFile(id: id, newName: newName)
        
        switch res {
        case .success(_):
            workspaceInput.fileOpCompleted(fileOp: .Rename(id: id, newName: newName))
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
            compactSheetHeight: .constant(200),
            width: RenameFileSheet.FORM_WIDTH,
            height: RenameFileSheet.FORM_HEIGHT,
            presentedContent: { item in
                RenameFileSheet(
                    homeState: HomeState(workspaceOutput: .preview, filesModel: .preview),
                    id: item.id,
                    name: item.name
                )
            }
        )
}
#else
#Preview {
    RenameFileSheet(homeState: HomeState(workspaceOutput: .preview, filesModel: .preview), id: (AppState.lb as! MockLb).file1.id, name: (AppState.lb as! MockLb).file1.name)
        .frame(width: RenameFileSheet.FORM_WIDTH, height: RenameFileSheet.FORM_HEIGHT
        )
}
#endif
