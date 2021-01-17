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
    @Environment(\.colorScheme) var colorScheme
    
    var deleted: Bool {
        core.files.filter({$0.id == meta.id}).isEmpty
    }
    
    var body: some View {
        ZStack(alignment: .topTrailing) {
            if content.text == nil && !deleted {
                ProgressView()
            } else {
                if deleted {
                    Text("\(meta.name) file has been deleted")
                } else {
                    EditorView(core: core, meta: meta, text: content.text!, changeCallback: content.updateText)
                        .onDisappear {
                            if !deleted {
                                content.finalize()
                            }
                        }
                }
            }
            
            if content.status == .WriteSuccess {
                ZStack {
                    Rectangle()
                        .foregroundColor(.textEditorBackground(isDark: colorScheme == .dark))
                        .frame(width: 30, height: 30, alignment: .center)
                        .cornerRadius(5)
                        .opacity(0.9)
                    Image(systemName: "externaldrive.fill.badge.checkmark")
                        .foregroundColor(.green)
                        .opacity(0.5)
                }
                .padding(.top, 2.0)
                .padding(.trailing, 20)
                .animation(.easeInOut(duration: 0.5))
                .onAppear(perform: {
                    DispatchQueue.main.asyncAfter(deadline: .now() + 2, execute: {
                        withAnimation {
                            content.status = .Inactive
                        }
                    })
                })
            }
        }
        .navigationTitle(meta.name)
    }
    
    
    init (core: Core, meta: FileMetadata) {
        self.core = core
        self.meta = meta
        print("init called")
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
                    print(err)
                    core.handleError(err)
                }
            }
        }
        
        // Save
        $text
            .debounce(for: .seconds(1), scheduler: DispatchQueue.main)
            .compactMap({$0})
            .compactMap { content in
                Future<FfiResult<SwiftLockbookCore.Empty, WriteToDocumentError>, Never> { promise in
                    promise(.success(self.save(content: content!)))
                }
            }
            .sink(receiveValue: { print($0)})
            .store(in: &cancellables)
    }
    
    func save(content: String) -> FfiResult<SwiftLockbookCore.Empty, WriteToDocumentError> {
        return core.serialQueue.sync {
            switch core.api.updateFile(id: meta.id, content: content) {
            case .success(let e):
                print("File saved successfully")
                withAnimation {
                    self.status = .WriteSuccess
                }
                return .success(e)
            case .failure(let err):
                return .failure(err)
            }
        }
    }
    
    func finalize() {
        switch core.api.updateFile(id: meta.id, content: text!) {
        case .success:
            print("File finalized successfully")
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
