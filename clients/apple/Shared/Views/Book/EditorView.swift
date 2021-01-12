import SwiftUI
import SwiftLockbookCore
import HighlightedTextEditor
import Combine

struct EditorView: View {
    
    @ObservedObject var core: Core
    let meta: FileMetadata
    @State var text: String
    
    var body: some View {
        VStack(spacing: 0) {
            if meta.name.hasSuffix(".md") {
                HighlightedTextEditor(text: $text, highlightRules: [])
            } else {
                TextEditor(text: $text)
            }
        }
    }
}

struct EditorLoader: View {
    @ObservedObject var core: Core
    let meta: FileMetadata
    @ObservedObject var content: Content
    
    var body: some View {
        if content.text == nil {
            ProgressView()
        } else {
            EditorView(core: core, meta: meta, text: content.text!)
        }
    }
    
    init (core: Core, meta: FileMetadata) {
        self.core = core
        self.meta = meta
        self.content = Content(core: core, meta: meta)
    }
}

class Content: ObservableObject {
    @ObservedObject var core: Core
    @Published var text: String?
    
    let meta: FileMetadata
    init(core: Core, meta: FileMetadata) {
        self.core = core
        self.meta = meta
        
        DispatchQueue.main.async { [weak self] in
            switch core.api.getFile(id: meta.id) {
            case .success(let decrypted):
                self?.text = decrypted
            case .failure(let err):
                print(err)
                core.handleError(err)
            }
        }
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

//struct EditorView_Previews: PreviewProvider {
//    static var previews: some View {
//        NavigationView {
//            EditorView(core: Core(), meta: FakeApi().fileMetas[0])
//        }
//    }
//}
