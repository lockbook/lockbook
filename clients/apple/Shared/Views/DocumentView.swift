import SwiftUI
import SwiftLockbookCore
import PencilKit
import SwiftEditor


struct iOSDocumentViewWrapper: View {
    let id: UUID
    
    var body: some View {
        DocumentView(id: id, isiPhone: true)
            .onDisappear {
                DI.currentDoc.cleanupOldDocs(true)
            }
    }
}

struct DocumentView: View, Equatable {
    static func == (lhs: DocumentView, rhs: DocumentView) -> Bool {
        lhs.id == rhs.id
    }

    let id: UUID
    
    @ObservedObject var model: DocumentLoadingInfo
    
#if os(iOS)
    @EnvironmentObject var toolbar: ToolbarModel
#endif
    
    public init(id: UUID, isiPhone: Bool = false) {
        self.id = id
        self.model = DI.currentDoc.getDocInfoOrCreate(id: id, isiPhone: isiPhone)
    }
    
    var body: some View {        
        Group {
            if model.loading {
                VStack {
                    Spacer()
                    
                    ProgressView()
                        .onAppear {
                            model.startLoading()
                        }
                        .title(model.meta.name) // No exact matches in reference to static method 'buildExpression'
                    
                    Spacer()
                }
            } else if model.error != "" {
                VStack {
                    Spacer()
                    
                    Text("errors while loading: \(model.error)")
                    
                    Spacer()
                }
            } else if model.deleted {
                VStack {
                    Spacer()
                    
                    Text("\(model.meta.name) was deleted.")
                    
                    Spacer()
                }
            } else {
                switch model.type {
                case .Image:
                    if let img = model.image {
                        VStack {
                            DocumentTitle(nameState: model.documentNameState, id: model.meta.id)
                            
                            ScrollView([.horizontal, .vertical]) {
                                img
                            }
                        }.title("")
                        
                    }
#if os(iOS)
                case .Drawing:
                    VStack {
                        DocumentTitle(nameState: model.documentNameState, id: model.meta.id)
                        
                        DrawingView(
                            model: model,
                            toolPicker: toolbar
                        )
                        .toolbar {
                            ToolbarItemGroup(placement: .bottomBar) {
                                Spacer()
                                DrawingToolbar(toolPicker: toolbar)
                                Spacer()
                            }
                        }
                    }.title("")
#endif
                case .Markdown:
                    if let editorState = model.textDocument,
                       let toolbarState = model.textDocumentToolbar {
                        Group {
                            MarkdownCompleteEditor(editorState: editorState, toolbarState: toolbarState, nameState: model.documentNameState, fileId: model.meta.id)
                                .equatable()
                        }
                        .title("")
                        .markdownToolbar(isiPhone: model.isiPhone, meta: model.meta)
                    }
                case .Unknown:
                    Text("\(model.meta.name) cannot be opened on this device.")
                        .title(model.meta.name)
                }
            }
        }
        .onDisappear {
            DI.files.refresh()
        }
    }
}

extension View {
    func title(_ name: String) -> some View {
        #if os(macOS)
        return self
        #else
        return self.navigationTitle("").navigationBarTitleDisplayMode(.inline)
        #endif
    }
    
    @ViewBuilder
    func markdownToolbar(isiPhone: Bool, meta: File) -> some View {
        #if os(macOS)
        self.toolbar {
            Button(action: {
                exportFileAndShowShareSheet(meta: meta)
            }, label: {
                Label("Export", systemImage: "square.and.arrow.up.fill")
            })
            .foregroundColor(.blue)
            .padding(.horizontal, 10)
        }
        #else
        if isiPhone {
            self.toolbar {
                Button(action: {
                    exportFileAndShowShareSheet(meta: meta)
                }, label: {
                    Label("Export", systemImage: "square.and.arrow.up.fill")
                })
                .foregroundColor(.blue)
                .padding(.horizontal, 10)
            }
        } else {
            self
        }
        #endif
    }
}

struct MarkdownCompleteEditor: View, Equatable {
    let editorState: EditorState
    let toolbarState: ToolbarState
    let nameState: NameState

    let fileId: UUID
    
    var body: some View {
#if os(iOS)
            VStack {
                markdownTitle
                
                MarkdownEditor(editorState, toolbarState, nameState)
                
                ScrollView(.horizontal) {
                    markdownToolbar
                        .padding(.bottom, 8)
                        .padding(.horizontal)
                }
            }
#else
            VStack {
                markdownTitle
                
                markdownToolbar
                    .padding(.top, 9)
                    .padding(.horizontal)
                
                MarkdownEditor(editorState, toolbarState, nameState)
            }
#endif
    }
        
    var markdownTitle: DocumentTitle {
        DocumentTitle(nameState: nameState, id: fileId)
    }
    
    var markdownToolbar: MarkdownToolbar {
        MarkdownToolbar(toolbarState: toolbarState)
    }
    
    static func == (lhs: MarkdownCompleteEditor, rhs: MarkdownCompleteEditor) -> Bool {
        return true
    }
}

struct MarkdownToolbar: View {
    @ObservedObject var toolbarState: ToolbarState
    
    var body: some View {
        HStack(spacing: 20) {
            HStack(spacing: 0) {
                                
                Menu(content: {
                    Button("Heading 1") {
                        toolbarState.toggleHeading(1)
                    }
                    .help("⌘⌃1")

                    Button("Heading 2") {
                        toolbarState.toggleHeading(2)
                    }
                    .help("⌘⌃2")

                    Button("Heading 3") {
                        toolbarState.toggleHeading(3)
                    }
                    .help("⌘⌃3")

                    Button("Heading 4") {
                        toolbarState.toggleHeading(4)
                    }
                    .help("⌘⌃4")
                }, label: {
                    HStack {
                        Image(systemName: "h.square")
                            .foregroundColor(.primary)
                            .padding(.vertical, 2)
                            .padding(.leading, 2)
                        
                        Image(systemName: "chevron.down")
                            .imageScale(.small)
                            .foregroundColor(.primary)
                            .padding(.trailing, 2)
                    }
                    .contentShape(Rectangle())
                })
                .padding(3)
                .background(toolbarState.isHeadingSelected ? .gray.opacity(0.2) : .clear)
                .cornerRadius(5)
                .help("Heading 1: ⌘⌃1 \nHeading 2: ⌘⌃2 \nHeading 3: ⌘⌃3 \nHeading 4: ⌘⌃4")
            }

            Divider()
                .frame(height: 20)

            HStack(spacing: 15) {
                Button(action: {
                    toolbarState.toggleBold()
                }) {
                    MarkdownEditorImage(systemImageName: "bold", isSelected: toolbarState.isBoldSelected)
                }
                .buttonStyle(.borderless)
                .help("Bold: ⌘B")

                Button(action: {
                    toolbarState.toggleItalic()
                }) {
                    MarkdownEditorImage(systemImageName: "italic", isSelected: toolbarState.isItalicSelected)
                }
                .buttonStyle(.borderless)
                .help("Italic: ⌘I")

                Button(action: {
                    toolbarState.toggleInlineCode()
                }) {
                    MarkdownEditorImage(systemImageName: "greaterthan.square", isSelected: toolbarState.isInlineCodeSelected)
                }
                .buttonStyle(.borderless)
                .help("Inline Code: ⌘⇧C")
            }

            Divider()
                .frame(height: 20)

            HStack(spacing: 15) {
                Button(action: {
                    toolbarState.toggleNumberList()
                }) {
                    MarkdownEditorImage(systemImageName: "list.number", isSelected: toolbarState.isNumberListSelected)
                }
                .buttonStyle(.borderless)
                .help("Number List: ⌘⇧7")
                
                Button(action: {
                    toolbarState.toggleBulletList()
                }) {
                    MarkdownEditorImage(systemImageName: "list.bullet", isSelected: toolbarState.isBulletListSelected)
                }
                .buttonStyle(.borderless)
                .help("Bullet List: ⌘⇧8")

                Button(action: {
                    toolbarState.toggleTodoList()
                }) {
                    MarkdownEditorImage(systemImageName: "checklist", isSelected: toolbarState.isTodoListSelected)
                }
                .buttonStyle(.borderless)
                .help("Todo List: ⌘⇧9")
            }

            #if os(iOS)

            Divider()
                .frame(height: 20)

            HStack(spacing: 15) {

                Button(action: {
                    toolbarState.tab(false)
                }) {
                    MarkdownEditorImage(systemImageName: "arrow.right.to.line.compact", isSelected: false)
                }
                .buttonStyle(.borderless)

                Button(action: {
                    toolbarState.tab(true)
                }) {
                    MarkdownEditorImage(systemImageName: "arrow.left.to.line.compact", isSelected: false)
                }
                .buttonStyle(.borderless)
            }

            #endif

            Spacer()
        }
    }
}

struct DocumentTitle: View {
    @ObservedObject var nameState: NameState
    let id: UUID
    let fileSuffix: String
    
    @State var name: String
    @State var error: String?
    @State var hasBeenFocused = false
    
    var docInfo: DocumentLoadingInfo? {
        get {
            DI.currentDoc.openDocuments[id]
        }
    }
    
    let justCreatedDoc: Bool
    
    init(nameState: NameState, id: UUID) {
        let openDocName = DI.files.idsAndFiles[id]?.name ?? DI.currentDoc.openDocuments[id]!.meta.name
        var openDocNameWithoutExt = (openDocName as NSString).deletingPathExtension
        
        self.nameState = nameState
        self.id = id
        self.justCreatedDoc = DI.currentDoc.justCreatedDoc?.id == id
                
        self._name = State(initialValue: openDocName == openDocNameWithoutExt ? "" : openDocNameWithoutExt)
        
        if self.justCreatedDoc {
            DI.currentDoc.justCreatedDoc = nil
        }
        
        if openDocName == openDocNameWithoutExt {
            openDocNameWithoutExt.removeFirst()
            self.fileSuffix = openDocNameWithoutExt
        } else {
            self.fileSuffix = (openDocName as NSString).pathExtension
        }
    }
    
    func realFileName(_ unformattedName: String) -> String {
        return unformattedName.toKebabCase() + "." + fileSuffix
    }
    
    func stripName(_ formattedName: String) -> (fileName: String, fileExt: String) {
        let openDocExt = (formattedName as NSString).pathExtension
        let openDocNameWithoutExt = (formattedName as NSString).deletingPathExtension
        

        if formattedName == openDocNameWithoutExt {
            return (fileName: "", fileExt: openDocNameWithoutExt)
        } else {
            return (fileName: openDocNameWithoutExt, fileExt: openDocExt)
        }
    }
    
    func renameFile(_ newName: String) {
        let realName = realFileName(newName)
        name = newName
        
        if let errorMsg = DI.files.renameFileSync(id: id, name: realName) {
            withAnimation {
                error = errorMsg
            }
        } else {
            docInfo?.meta.name = realName
            withAnimation {
                error = nil
            }
        }
    }
    
    var body: some View {
        VStack(alignment: .leading) {
            TextField("File name...", text: Binding(get: {
                return name.toKebabCase()
            }, set: { newValue, _ in
                hasBeenFocused = true
                
                renameFile(newValue)
            }))
            .autocapitalization(.none)
            .onChange(of: nameState.potentialTitle, perform: { newValue in
                if let potentialTitle = nameState.potentialTitle, !hasBeenFocused, justCreatedDoc, !potentialTitle.isEmpty {
                    renameFile(potentialTitle)
                }
            })
            .onChange(of: docInfo?.meta, perform: { newValue in
                if let newName = newValue?.name {
                    let (fileName, _) = stripName(newName)
                    
                    if !fileName.isEmpty && fileName != name {
                        name = fileName
                    }
                }
                
            })
            .textFieldStyle(.plain)
            .font(.largeTitle)
            .padding(.horizontal)
            .padding(.top)
            
            if let errorMsg = error {
                Text(errorMsg)
                    .font(.body)
                    .foregroundColor(.red)
                    .padding(.horizontal, 20)
            }
            
            Divider()
        }
    }
}

struct MarkdownEditor: View {
    @ObservedObject var editorState: EditorState
    
    let editor: EditorView
    
    public init(_ editorState: EditorState, _ toolbarState: ToolbarState, _ nameState: NameState) {
        self.editorState = editorState
        self.editor = EditorView(editorState, toolbarState, nameState)
    }
        
    var body: some View {
        editor
    }
}

struct MarkdownEditorImage: View {
    let systemImageName: String
    var isSelected: Bool

    var body: some View {
        Image(systemName: systemImageName)
            .padding(5)
            .foregroundColor(.primary)
            .background(isSelected ? .gray.opacity(0.2) : .clear)
            .cornerRadius(5)
    }
}

extension String {
    func toKebabCase() -> String {
        self.lowercased().replacingOccurrences(of: " ", with: "-")
    }
}
