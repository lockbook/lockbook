import SwiftUI
import SwiftLockbookCore
import Combine

struct EditorView: View {
    @ObservedObject var core: Core
    let meta: FileMetadata
    @State var succeeded: Bool = false
        
    var body: some View {
        let contentBinder = Binding(
            get: { core.currentEdits[meta.id] ?? "" },
            set: {
                core.editStream = (meta.id, $0)
                core.saver = .Inactive
            }
        )
        
        return TextEditor(text: contentBinder)
            .padding(0.1)
            .navigationTitle(meta.name)
            .toolbar(content: {
                ToolbarItem(placement: .automatic) {
                    Image(systemName: "checkmark.circle")
                        .foregroundColor(saverColor(saver: core.saver))
                        .opacity(0.4)
                }
            })
            .disabled(!succeeded)
            .onAppear {
                switch core.api.getFile(id: meta.id) {
                case .success(let decrypted):
                    core.currentEdits[meta.id] = decrypted.secret
                    self.succeeded = true
                case .failure(let err):
                    core.currentEdits[meta.id] = nil
                    core.displayError(error: err)
                    self.succeeded = false
                }
            }
            .onDisappear {
//                core.currentEdits[meta.id] = nil
            }
    }
    
    init(core: Core, meta: FileMetadata) {
        self.core = core
        self.meta = meta
    }
    
    func saverColor(saver: SaveStatus) -> Color {
        switch saver {
        case .Inactive:
            return .secondary
        case .Succeeded:
            return .green
        case .Failed:
            return .red
        }
    }
}

struct EditorView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            EditorView(core: Core(), meta: FakeApi().fileMetas[0])
        }
    }
}
