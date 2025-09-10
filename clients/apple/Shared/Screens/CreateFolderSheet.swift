import SwiftUI
import SwiftWorkspace

struct CreateFolderSheet: View {
    // MARK: Have to be updated manually whenever the view contents change. Vital for iPadOS and macOS
    #if os(iOS)
    static let FORM_WIDTH: CGFloat = 420
    static let FORM_HEIGHT: CGFloat = 190
    #else
    static let FORM_WIDTH: CGFloat = 420
    static let FORM_HEIGHT: CGFloat = 150
    #endif
    
    
    @Environment(\.dismiss) private var dismiss
    
    @StateObject var model: CreateFolderViewModel
    
    init(homeState: HomeState, parentId: UUID) {
        self._model = StateObject(wrappedValue: CreateFolderViewModel(homeState: homeState, parentId: parentId))
    }
    
    var body: some View {
        VStack(spacing: 10) {
            HStack {
                Text("New Folder")
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
                createAndDismiss()
            } label: {
                Text("Create")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(model.name.isEmpty)
        }
        .padding(.horizontal)
        .padding(.top, 3)
    }
    
    @ViewBuilder
    var textField: some View {
        #if os(iOS)
        AutoFocusTextField(text: $model.name, placeholder: "Folder name", returnKeyType: .done, borderStyle: .roundedRect) {
            createAndDismiss()
        }
        #else
        AutoFocusTextField(text: $model.name, placeholder: "Folder name", focusRingType: .none, isBordered: false) {
            createAndDismiss()
        }
        #endif
    }
    
    func createAndDismiss() {
        if model.createFolder() {
            dismiss()
        }
    }
}

class CreateFolderViewModel: ObservableObject {
    @Published var name: String = ""
    @Published var error: String = ""
    @Published var parentPath: String? = nil
    
    
    let parentId: UUID
    
    init(homeState: HomeState, parentId: UUID) {
        self.parentId = parentId
        
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.getPathById(id: parentId)
            
            DispatchQueue.main.async {
                switch res {
                case .success(let path):
                    self.parentPath = path
                case .failure(let err):
                    AppState.shared.error = .lb(error: err)
                }
            }
        }
    }
    
    func createFolder() -> Bool {
        guard !name.isEmpty else {
            return false
        }
        
        let res = AppState.lb.createFile(name: name, parent: parentId, fileType: .folder)
        
        switch res {
        case .success(_):
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
    @Previewable @State var sheetHeight: CGFloat = 0
    
    Color.accentColor
        .optimizedSheet(
            item: $file,
            constrainedSheetHeight: $sheetHeight,
            width: CreateFolderSheet.FORM_WIDTH,
            height: CreateFolderSheet.FORM_HEIGHT,
            presentedContent: { item in
                CreateFolderSheet(
                    homeState: HomeState(),
                    parentId: item.id
                )
            }
        )
}
#else
#Preview {
    CreateFolderSheet(homeState: HomeState(), parentId: (AppState.lb as! MockLb).file1.id)
        .frame(width: CreateFolderSheet.FORM_WIDTH, height: CreateFolderSheet.FORM_HEIGHT
        )
}
#endif
