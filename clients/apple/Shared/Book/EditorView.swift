import SwiftUI
import SwiftLockbookCore
import Combine

#if os(macOS)
/// Gets rid of the highlight border on a textfield
extension NSTextField {
    open override var focusRingType: NSFocusRingType {
        get { .none }
        set { }
    }
}
#endif

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
                switch core.api.renameFile(id: meta.id, name: contentBuffer.title) {
                case .success(_):
                    core.updateFiles()
                    contentBuffer.status = .Succeeded
                case .failure(let err):
                    core.handleError(err)
                    contentBuffer.status = .Failed
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
                .navigationBarItems(trailing: makeStatus())
            #else
            baseEditor
                .toolbar(content: {
                    ToolbarItem(placement: .automatic) {
                        makeStatus()
                            .font(.title)
                    }
                })
            #endif
        }
    }
    
    func makeStatus() -> some View {
        switch contentBuffer.status {
        case .Inactive:
            return Image(systemName: "slash.circle")
                .foregroundColor(.secondary)
                .opacity(0.4)
        case .Succeeded:
            return Image(systemName: "checkmark.circle")
                .foregroundColor(.green)
                .opacity(0.6)
        case .Failed:
            return Image(systemName: "xmark.circle")
                .foregroundColor(.red)
                .opacity(0.6)
        }
    }
    
    init(core: Core, meta: FileMetadata) {
        self.core = core
        self.meta = meta
        self.contentBuffer = ContentBuffer(meta: meta, initialContent: "loading...", core: core)
    }
}

class ContentBuffer: ObservableObject {
    let meta: FileMetadata
    private var cancellables: Set<AnyCancellable> = []
    let core: Core
    @Published var content: String
    @Published var succeeded: Bool = false
    @Published var status: SaveStatus = .Inactive
    @Published var title: String
    
    init(meta: FileMetadata, initialContent: String, core: Core) {
        self.meta = meta
        self.core = core
        self.content = initialContent
        self.title = meta.name
        
        $content
            .debounce(for: 0.2, scheduler: RunLoop.main)
            .sink { _ in
                self.status = .Inactive
            }
            .store(in: &cancellables)
        
        $content
            .debounce(for: 1, scheduler: DispatchQueue.global(qos: .background))
            .filter({ _ in self.succeeded })
            .flatMap { _ in
                Future<Void, Error> { promise in
                    promise(self.save())
                }
            }
            .eraseToAnyPublisher()
            .receive(on: RunLoop.main)
            .sink(receiveCompletion: { (err) in
                self.status = .Failed
            }, receiveValue: { (input) in
                self.status = .Succeeded
            })
            .store(in: &cancellables)
    }
    
    func save() -> Result<Void, Error> {
        core.serialQueue.sync {
            switch core.api.updateFile(id: meta.id, content: content) {
            case .success(_):
                return .success(())
            case .failure(let err):
                return .failure(err)
            }
        }
    }
}

enum SaveStatus {
    case Succeeded
    case Failed
    case Inactive
}

struct EditorView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            EditorView(core: Core(), meta: FakeApi().fileMetas[0])
        }
    }
}
