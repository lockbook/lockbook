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
                            MarkdownEditor(editorState: editorState, documentName: meta.name)
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
        return self.navigationTitle(name)
#else
        return self.navigationBarTitle(name, displayMode: .inline)
#endif
    }
}

struct MarkdownEditor: View {
    var editorState: EditorState
    var documentName: String
    
    @Environment(\.colorScheme) var colorScheme
    
    @State var isToolbarExpanded: Bool = true
    
    var body: some View {
        #if os(iOS)
        VStack {
            editor
                
            toolbar
        }
        .title(documentName)
        #else
        VStack {
            toolbar
            
            editor
        }
        .title(documentName)
        #endif
        
    }
    
    var editor: EditorView {
        EditorView(editorState)
    }
    
    var toolbar: some View {
        #if os(iOS)
        ScrollView(.horizontal) {
            HStack(spacing: 35) {
                Menu(content: {
                    Button("Heading 1") {
                        editor.header(headingSize: 1)
                    }
                    
                    Button("Heading 2") {
                        editor.header(headingSize: 2)
                    }
                    
                    Button("Heading 3") {
                        editor.header(headingSize: 3)
                    }
                    
                    Button("Heading 4") {
                        editor.header(headingSize: 4)
                    }
                }, label: {
                    Image(systemName: "h.square")
                        .imageScale(.large)
                })
                
                Button(action: {
                    editor.bold()
                }) {
                    Image(systemName: "bold")
                }
                
                Button(action: {
                    editor.italic()
                }) {
                    Image(systemName: "italic")
                }
                
                Button(action: {
                    editor.bulletedList()
                }) {
                    Image(systemName: "list.bullet")
                }
                
                Button(action: {
                    editor.numberedList()
                }) {
                    Image(systemName: "list.number")
                }
                
                Button(action: {
                    editor.checkedList()
                }) {
                    Image(systemName: "checklist")
                }
                
                Button(action: {
                    editor.tab()
                }) {
                    Image(systemName: "arrow.right.to.line")
                }
            }
            .padding()
        }
        .frame(height: 35)
        #else
        HStack(alignment: .center, spacing: 30) {
            Menu(content: {
                Button("Heading 1") {
                    editor.header(headingSize: 1)
                }
                
                Button("Heading 2") {
                    editor.header(headingSize: 2)
                }
                
                Button("Heading 3") {
                    editor.header(headingSize: 3)
                }
                
                Button("Heading 4") {
                    editor.header(headingSize: 4)
                }
            }, label: {
                Image(systemName: "h.square")
                    .imageScale(.large)
            })
            
            Button(action: {
                editor.bold()
            }) {
                Image(systemName: "bold")
                    .imageScale(.large)
            }
            
            Button(action: {
                editor.italic()
            }) {
                Image(systemName: "italic")
                    .imageScale(.large)
            }

            Button(action: {
                editor.bulletedList()
            }) {
                Image(systemName: "list.bullet")
                    .imageScale(.large)
            }

            Button(action: {
                editor.numberedList()
            }) {
                Image(systemName: "list.number")
                    .imageScale(.large)
            }

            Button(action: {
                editor.checkedList()
            }) {
                Image(systemName: "checklist")
                    .imageScale(.large)
            }
        }
        .padding(.top, 12)
        .padding(.bottom, 3)

        #endif
    }
}
