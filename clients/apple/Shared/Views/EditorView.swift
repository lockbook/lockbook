import SwiftUI
import SwiftLockbookCore
import Combine

struct EditorView: View {
    @Environment(\.colorScheme) var colorScheme
    let meta: ClientFileMetadata
    let text: String
    
    let changeCallback: (String) -> Void
    
    var body: some View {
        print("after \(text)")
        return GeometryReader { geo in
            NotepadView(
                text: text,
                frame: geo.frame(in: .local),
                theme: LockbookTheme,
                onTextChange: changeCallback
            )
        }
    }
}

struct EditorLoader: View {
    
    @EnvironmentObject var content: Content
    
    let meta: ClientFileMetadata
    @State var editorContent: String = ""
    @State var deleted: ClientFileMetadata?
    
    var body: some View {
        ZStack(alignment: .topTrailing) {
            switch content.text {
            /// We are forcing this view to hit the default case when it is in a transitionary stage!
            case .some(let c) where content.meta?.id == meta.id:
                if (deleted != meta) {
                    #if os(macOS)
                    EditorView(meta: meta, text: c, changeCallback: content.updateText)
                    #else
                    let _ = print("before \(c)")
                    EditorView(meta: meta, text: c, changeCallback: content.updateText)
                        .padding(.horizontal, 20)
                    #endif
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
        .navigationTitle(meta.name)
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
            EditorLoader(meta: FakeApi.fileMetas[0])
                .mockDI()
        }
        .preferredColorScheme(.dark)
    }
}
