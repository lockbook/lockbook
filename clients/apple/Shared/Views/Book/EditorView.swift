import SwiftUI
import SwiftLockbookCore
import Combine

struct EditorView: View, Equatable {
    /// Define an always equality so that this view doesn't reload once it's initialized
    static func == (lhs: EditorView, rhs: EditorView) -> Bool {
        lhs.meta.id == rhs.meta.id
    }
    
    @ObservedObject var core: Core
    @ObservedObject var contentBuffer: ContentBuffer
    let meta: FileMetadata
    
    var body: some View {
        return VStack(spacing: 0) {
            TitleTextField(text: $contentBuffer.title, doneEditing: {
                if (meta.name != contentBuffer.title) {
                    switch core.api.renameFile(id: meta.id, name: contentBuffer.title) {
                    case .success(_):
                        core.updateFiles()
                        contentBuffer.status = .RenameSuccess
                    case .failure(let err):
                        core.handleError(err)
                        contentBuffer.status = .RenameFailure
                    }
                }
            })
            
            let baseEditor = ContentEditor(text: $contentBuffer.content)
                .font(.system(.body, design: .monospaced))
                .disabled(!contentBuffer.succeeded)
                .onAppear {
                    switch core.api.getFile(id: meta.id) {
                    case .success(let decrypted):
                        contentBuffer.content = decrypted.secret
                        contentBuffer.succeeded = true
                    case .failure(let err):
                        core.handleError(err)
                        contentBuffer.succeeded = false
                    }
                }
                .onDisappear {
                    switch contentBuffer.save() {
                    case .success(_):
                        contentBuffer.succeeded = true
                    case .failure(let err):
                        core.handleError(err)
                        contentBuffer.succeeded = false
                    }
                }
            #if os(iOS)
            baseEditor
                .navigationBarItems(trailing: EditorStatus(status: contentBuffer.status))
            #else
            baseEditor
                .toolbar(content: {
                    ToolbarItem(placement: .automatic) {
                        EditorStatus(status: contentBuffer.status)
                            .font(.title)
                    }
                })
            #endif
        }
    }
    
    init(core: Core, meta: FileMetadata) {
        self.core = core
        self.meta = meta
        self.contentBuffer = ContentBuffer(meta: meta, initialContent: "loading...", core: core)
    }
}

struct EditorStatus: View {
    let status: ContentBuffer.Status
    var body: some View {
        switch status {
        case .BufferDied:
            return Image(systemName: "lock.fill")
                .foregroundColor(.red)
                .opacity(0.6)
        case .WriteSuccess:
            return Image(systemName: "text.badge.checkmark")
                .foregroundColor(.green)
                .opacity(0.3)
        case .WriteFailure:
            return Image(systemName: "text.badge.xmark")
                .foregroundColor(.red)
                .opacity(0.6)
        case .RenameSuccess:
            return Image(systemName: "checkmark.circle")
                .foregroundColor(.green)
                .opacity(0.3)
        case .RenameFailure:
            return Image(systemName: "xmark.circle")
                .foregroundColor(.red)
                .opacity(0.6)
        case .Inactive:
            return Image(systemName: "ellipsis")
                .foregroundColor(.secondary)
                .opacity(0.3)
        }
    }
}

#if os(macOS)
/// Gets rid of the highlight border on a textfield
extension NSTextField {
    open override var focusRingType: NSFocusRingType {
        get { .none }
        set { }
    }
}
#endif

struct EditorView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            EditorView(core: Core(), meta: FakeApi().fileMetas[0])
        }
    }
}
