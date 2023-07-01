import SwiftUI
import SwiftLockbookCore
import PencilKit
import SwiftEditor

struct DocumentView: View {
    
    @ObservedObject var model: DocumentLoadingInfo
    
#if os(iOS)
    @EnvironmentObject var toolbar: ToolbarModel
#endif
    
    var body: some View {
        Group {
            if model.loading {
                ProgressView()
                    .onAppear {
                        model.startLoading()
                    }
                    .title(model.meta.name) // No exact matches in reference to static method 'buildExpression'
            } else if model.error != "" {
                Text("errors while loading: \(model.error)")
            } else if model.deleted {
                Text("\(model.meta.name) was deleted.")
            } else {
                switch model.type {
                case .Image:
                    if let img = model.image {
                        ScrollView([.horizontal, .vertical]) {
                            img
                        }.title(model.meta.name)
                    }
#if os(iOS)
                case .Drawing:
                    DrawingView(
                        model: model,
                        toolPicker: toolbar
                    )
                    .navigationBarTitle(model.meta.name, displayMode: .inline)
                    .toolbar {
                        ToolbarItemGroup(placement: .bottomBar) {
                            Spacer()
                            DrawingToolbar(toolPicker: toolbar)
                            Spacer()
                        }
                    }
#endif
                    
                case .Markdown:
                    if let editorState = model.textDocument,
                       let toolbarState = model.textDocumentToolbar,
                       let nameState = model.textDocumentName {
                        Group {
                            MarkdownCompleteEditor(editorState: editorState, toolbarState: toolbarState, nameState: nameState)
                                .equatable()
                        }.title("")
                    }
                case .Unknown:
                    Text("\(model.meta.name) cannot be opened on this device.")
                        .title(model.meta.name)
                }
            }
        }
        .onDisappear {
            DI.files.refreshSuggestedDocs()
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
}

struct MarkdownCompleteEditor: View, Equatable {
    let editorState: EditorState
    let toolbarState: ToolbarState
    let nameState: NameState

    let fileId: UUID = DI.currentDoc.openDocuments.values.first!.meta.id
    
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
        
    var markdownTitle: MarkdownTitle {
        MarkdownTitle(nameState: nameState, id: fileId)
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
                
                // hack for the heading 1-4 shortcut since the shortcuts in the menu won't work unless opened
                Button(action: {
                    toolbarState.toggleHeading(1)
                }) {
                    EmptyView()
                }
                .frame(width: 0, height: 0)
                .keyboardShortcut("1", modifiers: [.command, .control])
                
                Button(action: {
                    toolbarState.toggleHeading(2)
                }) {
                    EmptyView()
                }
                .frame(width: 0, height: 0)
                .keyboardShortcut("2", modifiers: [.command, .control])
                
                Button(action: {
                    toolbarState.toggleHeading(3)
                }) {
                    EmptyView()
                }
                .frame(width: 0, height: 0)
                .keyboardShortcut("3", modifiers: [.command, .control])
                
                Button(action: {
                    toolbarState.toggleHeading(4)
                }) {
                    EmptyView()
                }
                .frame(width: 0, height: 0)
                .keyboardShortcut("4", modifiers: [.command, .control])
                
                Menu(content: {
                    Button("Heading 1") {
                        toolbarState.toggleHeading(1)
                    }
                    .keyboardShortcut("1", modifiers: [.command, .control])

                    Button("Heading 2") {
                        toolbarState.toggleHeading(2)
                    }
                    .keyboardShortcut("2", modifiers: [.command, .control])

                    Button("Heading 3") {
                        toolbarState.toggleHeading(3)
                    }
                    .keyboardShortcut("3", modifiers: [.command, .control])

                    Button("Heading 4") {
                        toolbarState.toggleHeading(4)
                    }
                    .keyboardShortcut("4", modifiers: [.command, .control])
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
                .keyboardShortcut("b", modifiers: .command)

                Button(action: {
                    toolbarState.toggleItalic()
                }) {
                    MarkdownEditorImage(systemImageName: "italic", isSelected: toolbarState.isItalicSelected)
                }
                .buttonStyle(.borderless)
                .keyboardShortcut("i", modifiers: .command)

                Button(action: {
                    toolbarState.toggleInlineCode()
                }) {
                    MarkdownEditorImage(systemImageName: "greaterthan.square", isSelected: toolbarState.isInlineCodeSelected)
                }
                .buttonStyle(.borderless)
                .keyboardShortcut("c", modifiers: [.command, .shift])

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
                .keyboardShortcut("7", modifiers: [.command, .shift])
                
                Button(action: {
                    toolbarState.toggleBulletList()
                }) {
                    MarkdownEditorImage(systemImageName: "list.bullet", isSelected: toolbarState.isBulletListSelected)
                }
                .buttonStyle(.borderless)
                .keyboardShortcut("8", modifiers: [.command, .shift])

                Button(action: {
                    toolbarState.toggleTodoList()
                }) {
                    MarkdownEditorImage(systemImageName: "checklist", isSelected: toolbarState.isTodoListSelected)
                }
                .buttonStyle(.borderless)
                .keyboardShortcut("9", modifiers: [.command, .shift])
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

struct MarkdownTitle: View {
    @ObservedObject var nameState: NameState
    let id: UUID
    
    @FocusState var focused: Bool
    @State var error: String?
    @State var hasBeenFocused = false
    
    var docInfo: DocumentLoadingInfo {
        get {
            DI.currentDoc.openDocuments[id]!
        }
    }
    
    let isOriginNameUUID: Bool
    
    init(nameState: NameState, id: UUID) {
        self.nameState = nameState
        self.id = id
        self.isOriginNameUUID = UUID(uuidString: DI.currentDoc.openDocuments[id]!.meta.name.replacingOccurrences(of: ".md", with: "")) != nil
    }
    
    var body: some View {
        VStack(alignment: .leading) {
            TextField("File name...", text: Binding(get: {
                return docInfo.meta.name.replacingOccurrences(of: ".md", with: "")
            }, set: { newValue, _ in
                hasBeenFocused = true
                docInfo.meta.name = newValue.toKebabCase()
            }))
            .focused($focused)
            .onChange(of: docInfo.meta.name, perform: { newValue in
                if let errorMsg = DI.files.renameFile(id: id, name: newValue + ".md") {
                    error = errorMsg
                } else {
                    error = nil
                    print("REFRESHING")
                    DI.files.refresh()
                }
            })
            .onChange(of: nameState.potentialTitle, perform: { newValue in
                print("potential name")
                if let potentialTitle = nameState.potentialTitle, !hasBeenFocused, isOriginNameUUID {
                    docInfo.meta.name = potentialTitle.toKebabCase()
                }
            })
            .textFieldStyle(.plain)
            .font(.largeTitle)
            .padding(.horizontal)
            .padding(.top)
            .onChange(of: focused, perform: { newValue in
                nameState.focusLocation = newValue ? .title : .editor
            })
            .onChange(of: nameState.focusLocation, perform: { newValue in
                focused = newValue == .title
            })
            
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
