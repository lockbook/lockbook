import SwiftUI
import SwiftLockbookCore
import PencilKit
import SwiftEditor

struct DocumentView: View {
    
    let meta: File
    
    @EnvironmentObject var model: DocumentLoader
#if os(iOS)
    @EnvironmentObject var toolbar: ToolbarModel
    @EnvironmentObject var current: CurrentDocument
#endif
    
    var body: some View {
        Group {
            if meta != model.meta || model.loading {
                ProgressView()
                    .onAppear {
                        model.startLoading(meta)
                    }
                    .title(meta.name)
            } else if model.error != "" {
                Text("errors while loading: \(model.error)")
            } else if model.deleted {
                Text("\(meta.name) was deleted.")
            } else {
                if let type = model.type {
                    switch type {
                    case .Image:
                        if let img = model.image {
                            ScrollView([.horizontal, .vertical]) {
                                img
                            }.title(meta.name)
                        }
#if os(iOS)
                    case .Drawing:
                        DrawingView(
                            model: model,
                            toolPicker: toolbar
                        )
                        .navigationBarTitle(meta.name, displayMode: .inline)
                        .toolbar {
                            ToolbarItemGroup(placement: .bottomBar) {
                                Spacer()
                                DrawingToolbar(toolPicker: toolbar)
                                Spacer()
                            }
                        }
#endif

                    case .Markdown:
                        if let editorState = model.textDocument {
                            VStack {
                                MarkdownTitle(editorState: editorState, name: meta.name)
                                
                                MarkdownEditor(editorState)
                                    .equatable()
                            }.title(meta.name)
                        }
                    case .Unknown:
                        Text("\(meta.name) cannot be opened on this device.")
                            .title(meta.name)
                    }
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
        return self
#endif
    }
}

struct MarkdownTitle: View {
    @ObservedObject var editorState: EditorState
    @State var name: String?
    
    @State var isTitleEditable = false
    @FocusState var isEditableTitleFocused: Bool
    
    var body: some View {
            TextField("Type your file name here...", text: Binding(get: {
                editorState.potentialTitle ?? "unset name"
            }, set: { newValue, _ in
                name = newValue
            }))
            .focused($isEditableTitleFocused)
            .onChange(of: isEditableTitleFocused, perform: { newValue in
                print("THIS IS TOGGLED from \(isTitleEditable) to \(!isTitleEditable)")
                isTitleEditable.toggle()
            })
            .textFieldStyle(.plain)
            .font(.largeTitle)
            .padding(.horizontal)
    }
}

struct MarkdownEditor: View, Equatable {
    
    @ObservedObject var editorState: EditorState
    let editor: EditorView
    
    public init(_ editorState: EditorState) {
        self.editorState = editorState

        self.editor = EditorView(editorState)
        self.editor.automaticTitleComputation(computeTitle: true)
    }
    
    @Environment(\.colorScheme) var colorScheme
    
    var body: some View {
        #if os(iOS)
        VStack {
            editor
                
            ScrollView(.horizontal) {
                toolbar
                    .padding(.bottom, 8)
                    .padding(.horizontal)
            }
        }
        #else
        VStack {
            toolbar
                .padding(.top, 9)
                .padding(.horizontal)

            editor
        }
        #endif
    }
    
    var toolbar: some View {
        HStack(spacing: 20) {
            HStack(spacing: 0) {
                
                // hack for the heading 1-4 shortcut since the shortcuts in the menu won't work unless opened
                Button(action: {
                    editor.header(headingSize: 1)
                }) {
                    EmptyView()
                }
                .frame(width: 0, height: 0)
                .keyboardShortcut("1", modifiers: [.command, .control])
                
                Button(action: {
                    editor.header(headingSize: 2)
                }) {
                    EmptyView()
                }
                .frame(width: 0, height: 0)
                .keyboardShortcut("2", modifiers: [.command, .control])
                
                Button(action: {
                    editor.header(headingSize: 3)
                }) {
                    EmptyView()
                }
                .frame(width: 0, height: 0)
                .keyboardShortcut("3", modifiers: [.command, .control])
                
                Button(action: {
                    editor.header(headingSize: 4)
                }) {
                    EmptyView()
                }
                .frame(width: 0, height: 0)
                .keyboardShortcut("4", modifiers: [.command, .control])
                
                Menu(content: {
                    Button("Heading 1") {
                        editor.header(headingSize: 1)
                    }
                    .keyboardShortcut("1", modifiers: [.command, .control])

                    Button("Heading 2") {
                        editor.header(headingSize: 2)
                    }
                    .keyboardShortcut("2", modifiers: [.command, .control])

                    Button("Heading 3") {
                        editor.header(headingSize: 3)
                    }
                    .keyboardShortcut("3", modifiers: [.command, .control])

                    Button("Heading 4") {
                        editor.header(headingSize: 4)
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
                .background(editorState.isHeadingSelected ? .gray.opacity(0.2) : .clear)
                .cornerRadius(5)
            }

            Divider()
                .frame(height: 20)

            HStack(spacing: 15) {
                Button(action: {
                    editor.bold()
                }) {
                    MarkdownEditorImage(systemImageName: "bold", isSelected: editorState.isBoldSelected)
                }
                .buttonStyle(.borderless)
                .keyboardShortcut("b", modifiers: .command)

                Button(action: {
                    editor.italic()
                }) {
                    MarkdownEditorImage(systemImageName: "italic", isSelected: editorState.isItalicSelected)
                }
                .buttonStyle(.borderless)
                .keyboardShortcut("i", modifiers: .command)

                Button(action: {
                    editor.inlineCode()
                }) {
                    MarkdownEditorImage(systemImageName: "greaterthan.square", isSelected: editorState.isInlineCodeSelected)
                }
                .buttonStyle(.borderless)
                .keyboardShortcut("c", modifiers: [.command, .shift])

            }

            Divider()
                .frame(height: 20)

            HStack(spacing: 15) {
                Button(action: {
                    editor.numberedList()
                }) {
                    MarkdownEditorImage(systemImageName: "list.number", isSelected: editorState.isNumberListSelected)
                }
                .buttonStyle(.borderless)
                .keyboardShortcut("7", modifiers: [.command, .shift])
                
                Button(action: {
                    editor.bulletedList()
                }) {
                    MarkdownEditorImage(systemImageName: "list.bullet", isSelected: editorState.isBulletListSelected)
                }
                .buttonStyle(.borderless)
                .keyboardShortcut("8", modifiers: [.command, .shift])

                Button(action: {
                    editor.todoList()
                }) {
                    MarkdownEditorImage(systemImageName: "checklist", isSelected: editorState.isTodoListSelected)
                }
                .buttonStyle(.borderless)
                .keyboardShortcut("9", modifiers: [.command, .shift])
            }

            #if os(iOS)

            Divider()
                .frame(height: 20)

            HStack(spacing: 15) {

                Button(action: {
                    editor.tab(deindent: false)
                }) {
                    MarkdownEditorImage(systemImageName: "arrow.right.to.line.compact", isSelected: false)
                }
                .buttonStyle(.borderless)

                Button(action: {
                    editor.tab(deindent: true)
                }) {
                    MarkdownEditorImage(systemImageName: "arrow.left.to.line.compact", isSelected: false)
                }
                .buttonStyle(.borderless)
            }

            #endif

            Spacer()
        }
    }
    
    static func == (lhs: MarkdownEditor, rhs: MarkdownEditor) -> Bool {
        return true
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
