import SwiftUI
import SwiftLockbookCore
import Combine

struct EditorView: View {
    @ObservedObject var core: Core
    let meta: FileMetadata
    @State var succeeded: Bool = false
        
    var body: some View {
        let contentBinder = Binding(
            get: { core.currentEdit.map({ $0.1 }) ?? "" },
            set: {
                core.currentEdit = (meta.id, $0)
                core.saver = nil
            }
        )
        
        return TextEditor(text: contentBinder)
            .navigationTitle(meta.name)
            .toolbar(content: {
                Image(systemName: "checkmark.circle")
                    .foregroundColor(core.saver ?? .secondary)
                    .opacity(0.4)
            })
            .disabled(!succeeded)
            .onAppear {
                switch core.api.getFile(id: meta.id) {
                case .success(let decrypted):
                    core.currentEdit = (meta.id, decrypted.secret)
                    self.succeeded = true
                case .failure(let err):
                    core.currentEdit = nil
                    core.displayError(error: err)
                    self.succeeded = false
                }
            }
            .onDisappear {
                core.currentEdit = nil
            }
    }
    
    init(core: Core, meta: FileMetadata) {
        self.core = core
        self.meta = meta
    }
}

struct EditorView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            EditorView(core: Core(), meta: FakeApi().fileMetas[0])
        }
    }
}
