import SwiftUI
import SwiftLockbookCore
import Combine

struct EditorView: View {
    @Environment(\.colorScheme) var colorScheme
    let meta: FileMetadata
    @State var text: String
    
    let changeCallback: (String) -> Void
    
    var body: some View {
        GeometryReader { geo in
            NotepadView(
                text: $text,
                frame: geo.frame(in: .local),
                theme: LockbookTheme,
                onTextChange: changeCallback
            )
        }
    }
}

struct EditorLoader: View {
    
    @ObservedObject var content: Content
    let meta: FileMetadata
    @State var editorContent: String = ""
    @State var title: String = ""
    let deleteChannel: PassthroughSubject<FileMetadata, Never>
    @State var deleted: FileMetadata?
    
    var body: some View {
        ZStack(alignment: .topTrailing) {
            switch content.text {
            /// We are forcing this view to hit the default case when it is in a transitionary stage!
            case .some(let c) where content.meta?.id == meta.id:
                if (deleted != meta) {
                    EditorView(meta: meta, text: c, changeCallback: content.updateText)
                    ActivityIndicator(status: $content.status)
                        .opacity(content.status == .WriteSuccess ? 1 : 0)
                } else {
                    Text("\(meta.name) file has been deleted")
                }
            default:
                ProgressView()
                    .onAppear {
                        content.openDocument(meta: meta)
                    }
            }
        }
        .onReceive(deleteChannel) { deletedMeta in
            if (deletedMeta.id == meta.id) {
                deleted = deletedMeta
            }
        }
    }
}

class Content: ObservableObject {
    @Published var text: String?
    @Published var meta: FileMetadata?
    var cancellables = Set<AnyCancellable>()
    @Published var status: Status = .Inactive
    let write: (UUID, String) -> FfiResult<SwiftLockbookCore.Empty, WriteToDocumentError>
    let read: (UUID) -> FfiResult<String, ReadDocumentError>
    var writeListener: () -> Void = {}
    
    init(write: @escaping (UUID, String) -> FfiResult<SwiftLockbookCore.Empty, WriteToDocumentError>, read: @escaping (UUID) -> FfiResult<String, ReadDocumentError>) {
        self.read = read
        self.write = write
        
        $text
            .debounce(for: .seconds(1), scheduler: DispatchQueue.main)
            .sink(receiveValue: {
                if let c = $0, let m = self.meta {
                    self.writeDocument(meta: m, content: c)
                }
            })
            .store(in: &cancellables)
    }
    
    func updateText(text: String) {
        self.text = text
        status = .Inactive
    }

    func writeDocument(meta: FileMetadata, content: String) {
        switch write(meta.id, content) {
        case .success(_):
            writeListener()
            withAnimation {
                status = .WriteSuccess
            }
        case .failure(let err):
            print(err)
        }
    }
    
    func openDocument(meta: FileMetadata) {
        DispatchQueue.main.async {
            switch self.read(meta.id) {
            case .success(let txt):
                self.meta = meta
                self.text = txt
            case .failure(let err):
                print(err)
            }
        }
    }
    
    func closeDocument(meta: FileMetadata) {
        self.meta = .none
        text = .none
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
            EditorLoader(content: GlobalState().openDocument, meta: FakeApi.fileMetas[0], deleteChannel: PassthroughSubject<FileMetadata, Never>())
        }
        .preferredColorScheme(.dark)
    }
}
