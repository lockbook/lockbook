import Foundation
import SwiftUI
import SwiftWorkspace

struct SelectFolderSheet: View {
    @Environment(\.dismiss) private var dismiss
    
    @StateObject var model: SelectFolderViewModel
    @ObservedObject var filesModel: FilesViewModel
    
    @State var mode: SelectFolderMode = .Tree
    
    let action: SelectFolderAction
    
    init(homeState: HomeState, filesModel: FilesViewModel, action: SelectFolderAction) {
        self._model = StateObject(wrappedValue: SelectFolderViewModel(homeState: homeState, filesModel: filesModel))
        self._filesModel = ObservedObject(wrappedValue: filesModel)
        self.action = action
    }
    
    var actionMsg: String {
        switch action {
        case .move(let files):
            if files.count == 1 {
                "Moving \"\(files[0].name)\"."
            } else {
                "Moving \(files.count) \(files.count == 1 ? "file" : "files")."
            }
        case .externalImport(let paths):
            "Importing \(paths.count) \(paths.count == 1 ? "file" : "files")."
        case .acceptShare(let name, _):
            "Accepting share \"\(name)\"."
        }
    }
    
    var body: some View {
        Group {
            switch mode {
            case .List:
                folderList
            case .Tree:
                folderTree
            }
        }
        .onChange(of: model.exit) { newValue in
            if newValue {
                dismiss()
            }
        }
    }
    
    var folderList: some View {
        VStack {
            VStack {
                HStack {
                    SelectFolderTextFieldWrapper(placeholder: "Search folder", onSubmit: {
                        selectFolderAndDismiss(path: model.selectedPath.isEmpty ? "/" : model.selectedPath)
                    }, model: model)
                        .frame(height: 19)
                        .onChange(of: model.searchInput) { _ in
                            model.selected = 0
                        }
                    
                    if !model.searchInput.isEmpty {
                        Button(action: {
                            model.searchInput = ""
                        }, label: {
                            Image(systemName: "xmark.circle.fill")
                                .foregroundStyle(.gray)
                        })
                        .padding(.leading)
                        .selectFolderButton()
                    }
                    
                    Button(action: {
                        withAnimation {
                            mode = .Tree
                        }
                    }, label: {
                        Image(systemName: "chevron.right")
                            .foregroundStyle(.foreground)
                    })
                    .padding(.leading)
                    .selectFolderButton()
                }
                .padding(.horizontal)
                .padding(.bottom, 4)
                .padding(.top)
                
                
                Divider()
            }
            
            HStack {
                if let error = model.error {
                    Text(error)
                        .foregroundStyle(.red)
                        .fontWeight(.bold)
                        .lineLimit(2, reservesSpace: false)
                } else {
                    Text(actionMsg)
                        .fontWeight(.bold)
                }
                
                Spacer()
            }
            .padding(.horizontal)
            .padding(.vertical, 5)
            
            if model.folderPaths != nil {
                ScrollViewReader { scrollHelper in
                    ScrollView {
                        ForEach(model.filteredFolderPaths, id: \.self) { path in
                            Button(action: {
                                selectFolderAndDismiss(path: path.isEmpty ? "/" : path)
                            }, label: {
                                HStack {
                                    HighlightedText(text: path.isEmpty ? "/" : path, pattern: model.searchInput, textSize: 16)
                                        .foregroundStyle(.foreground)
                                        .multilineTextAlignment(.leading)
                                    
                                    Spacer()
                                }
                            })
                            .padding(.horizontal)
                            .padding(.vertical, 5)
                            .selectedItem(item: path.isEmpty ? "/" : path, selected: model.selectedPath)
                            .selectFolderButton()
                        }
                    }
                    .onChange(of: model.selected) { newValue in
                        if model.selected < model.filteredFolderPaths.count {
                            withAnimation {
                                scrollHelper.scrollTo(model.selectedPath, anchor: .center)
                            }
                        }
                    }
                }
            } else {
                ProgressView()
                    .controlSize(.small)
            }
            
            Spacer()
        }
        .onAppear {
            model.calculateFolderPaths()
        }
    }
    
    @ViewBuilder
    var folderTree: some View {
        if let root = model.filesModel.root {
            VStack {
                HStack {
                    if let error = model.error {
                        Text(error)
                            .foregroundStyle(.red)
                            .fontWeight(.bold)
                            .lineLimit(2, reservesSpace: false)
                    } else {
                        Text(actionMsg)
                            .fontWeight(.bold)
                    }
                    
                    Spacer()

                    Button(action: {
                        withAnimation {
                            mode = .List
                        }
                    }, label: {
                        Image(systemName: "magnifyingglass")
                            .foregroundStyle(.foreground)
                    })
                    .padding(.leading)
                    .selectFolderButton()
                }
                .padding(.bottom, 10)
                .padding(.horizontal)
                
                
                ScrollView {
                    SelectFolderNestedList(
                        node: WithChild(root, model.filesModel.files, { (parent: File, meta: File) in
                            parent.id == meta.parent && parent.id != meta.id && meta.type == .folder
                        }),
                        row: { (dest: File) in
                            Button(action: {
                                
                            }, label: {
                                Label {
                                    Text(dest.name)
                                        .foregroundStyle(.foreground)
                                } icon: {
                                    Image(systemName: FileIconHelper.fileToSystemImageName(file: dest))
                                        .foregroundStyle(Color.blue)
                                }

                            })
                            .selectFolderButton()
                        }
                    )
                    .padding(.bottom)
                }
                .padding(.leading)
            }
            .padding(.top)
        } else {
            ProgressView()
        }
    }
    
    func selectFolderAndDismiss(parent: UUID) {
        if model.selectFolder(action: action, parent: parent) {
            dismiss()
        }
    }
    
    func selectFolderAndDismiss(path: String) {
        if model.selectFolder(action: action, path: path) {
            dismiss()
        }
    }
}

struct HighlightedText: View {
    let text: AttributedString
    
    init(text: String, pattern: String, textSize: CGFloat) {
        var attribText = AttributedString(text)
        
        let range = attribText.range(of: pattern, options: .caseInsensitive) ?? (attribText.startIndex..<attribText.startIndex)
        
        attribText.font = .systemFont(ofSize: textSize)
        attribText[range].font = .bold(Font.system(size: textSize))()
        
        self.text = attribText
    }

    var body: some View {
        Text(text)
    }
}

#Preview("Select Folder - Move") {
    Color.accentColor
        .sheet(isPresented: .constant(true)) {
            SelectFolderSheet(
                homeState: HomeState(),
                filesModel: FilesViewModel(workspaceState: WorkspaceState()),
                action: .move(files: [(AppState.lb as! MockLb).file1])
            )
        }
}

#Preview("Select Folder - Accept Share") {
    Color.accentColor
        .sheet(isPresented: .constant(true)) {
            SelectFolderSheet(
                homeState: HomeState(),
                filesModel: FilesViewModel(workspaceState: WorkspaceState()),
                action: .acceptShare(name: "work.md", id: UUID())
            )
        }
}

#Preview("Select Folder - Import Files") {
    Color.accentColor
        .sheet(isPresented: .constant(true)) {
            SelectFolderSheet(
                homeState: HomeState(),
                filesModel: FilesViewModel(workspaceState: WorkspaceState()),
                action: .externalImport(paths: ["/path/to/file.txt", "/path/to/file2.txt"])
            )
        }
}
