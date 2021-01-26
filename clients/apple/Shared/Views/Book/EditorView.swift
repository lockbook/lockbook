import SwiftUI
import SwiftLockbookCore
import HighlightedTextEditor
import Combine

struct EditorView: View {

    @ObservedObject var core: Core
    let meta: FileMetadata
    @State var text: String
    
    let changeCallback: (String) -> Void
    
    var highlightRules: [HighlightRule] {
        if meta.name.hasSuffix(".md") {
            return .lockbookMarkdown
        } else {
            return []
        }
    }
    
    var body: some View {
        #if os(iOS)
        HighlightedTextEditor(text: $text, highlightRules: highlightRules, onTextChange: changeCallback)
        #else
        HighlightedTextEditor(text: $text, highlightRules: highlightRules, onTextChange: changeCallback)
        #endif
        
    }
}

struct EditorLoader: View {

    @ObservedObject var core: Core
    let meta: FileMetadata
    @ObservedObject var content: Content
    @State var editorContent: String = ""
    @State var title: String = ""
    
    var deleted: Bool {
        core.files.filter({$0.id == meta.id}).isEmpty
    }
    
    var body: some View {
        ZStack(alignment: .topTrailing) {
            switch content.text {
            case .some(let c):
                if deleted {
                    Text("\(meta.name) file has been deleted")
                } else {
                    EditorView(core: core, meta: meta, text: c, changeCallback: content.updateText)
                }
            case .none:
                ProgressView()
            }

            if content.status == .WriteSuccess {
                ActivityIndicator(status: $content.status)
            }
        }
        .navigationTitle(meta.name)
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
    var cancellables = Set<AnyCancellable>()
    @Published var succeeded: Bool = false
    @Published var status: Status = .Inactive
    
    let meta: FileMetadata
    
    func updateText(text: String) {
        self.text = text
        self.status = .Inactive
    }
    init(core: Core, meta: FileMetadata) {
        self.core = core
        self.meta = meta
        
        // Load
        DispatchQueue.main.async { [weak self] in
            if !core.files.filter({$0.id == meta.id}).isEmpty {
                switch core.api.getFile(id: meta.id) {
                case .success(let decrypted):
                    self?.text = decrypted
                case .failure(let err):
                    core.handleError(err)
                }
            }
        }
        
        // Save
        $text
            .debounce(for: .seconds(1), scheduler: DispatchQueue.main)
            .sink(receiveValue: {
                self.save(content: $0!)
            })
            .store(in: &cancellables)
    }
    
    func save(content: String) {
        switch core.api.updateFile(id: meta.id, content: content) {
        case .success(_):
            withAnimation {
                self.status = .WriteSuccess
            }
        case .failure(let err):
            core.handleError(err)
        }
    }
}

enum Status {
    case WriteSuccess
    case WriteFailure
    case Inactive
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
            EditorLoader(core: Core(), meta: FakeApi().fileMetas[0])
        }
        .preferredColorScheme(.dark)
    }
}
