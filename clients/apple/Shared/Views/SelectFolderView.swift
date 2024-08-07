import Foundation
import SwiftUI
import CLockbookCore
import SwiftLockbookCore
import UIKit

struct SelectFolderView: View {
    @EnvironmentObject var core: CoreService
    
    @State var searchInput: String = ""
    @State var error: Bool = false
    
    @Environment(\.presentationMode) var presentationMode
    
    @State var folderPaths: [String]? = nil
    let action: SelectFolderAction
    @State var mode: SelectFolderMode = .Tree
    
    var filteredFolderPaths: [String] {
        if let folderPaths = folderPaths {
            if searchInput.isEmpty {
                return folderPaths
            } else {
                return folderPaths.filter { $0.localizedCaseInsensitiveContains(searchInput) }
            }
        } else {
            return []
        }
    }

    var actionMsg: String {
        switch action {
        case .Move(let ids):
            "Moving \(ids.count) \(ids.count == 1 ? "file" : "files")."
        case .Import(let ids):
            "Importing \(ids.count) \(ids.count == 1 ? "file" : "files")."
        case .AcceptShare((let name, _)):
            "Accepting share \"\(name)\"."
        }
    }
    
    var body: some View {
        switch mode {
        case .List:
            folderListView
        case .Tree:
            folderTreeView
        }
    }
    
    var folderListView: some View {
        VStack {
            VStack {
                HStack {
                    SelectFolderTextField(placeholder: "Search folder", onSubmit: {
                        guard let selectedFolder = filteredFolderPaths.first else {
                            return
                        }
                        
                        selectFolder(path: selectedFolder.isEmpty ? "/" : selectedFolder)
                    }, text: $searchInput)
                        .frame(height: 19)
                    
                    if !searchInput.isEmpty {
                        Button(action: {
                            searchInput = ""
                        }, label: {
                            Image(systemName: "xmark.circle.fill")
                                .foregroundStyle(.gray)
                        })
                        .padding(.leading)
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
                }
                .padding(.horizontal)
                .padding(.bottom, 4)
                .padding(.top)
                
                
                Divider()
            }
            
            
            HStack {
                if error {
                    Text("Something went wrong, please exit and try again.")
                        .foregroundStyle(.red)
                        .fontWeight(.bold)
                } else {
                    Text(actionMsg)
                        .fontWeight(.bold)
                }
                
                Spacer()
            }
            .padding(.horizontal)
            .padding(.vertical, 5)
            
            if folderPaths != nil {
                List(filteredFolderPaths, id: \.self) { path in
                    HStack {
                        Button(action: {
                            selectFolder(path: path.isEmpty ? "/" : path)
                        }, label: {
                            HighlightedText(text: path.isEmpty ? "/" : path, pattern: searchInput, textSize: 16)
                        })
                        
                        Spacer()
                    }
                    .modifier(SelectedItemModifier(item: path, selected: filteredFolderPaths.first ?? ""))
                    .listRowSeparator(.hidden)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 1)
                    
                }
                .listStyle(.inset)
            } else {
                ProgressView()
            }
            
            Spacer()
        }
        .onAppear {
            DispatchQueue.global(qos: .userInitiated).async {
                switch DI.files.getFolderPaths() {
                case .none:
                    DispatchQueue.main.async {
                        error = true
                    }
                case .some(let folderPaths):
                    DispatchQueue.main.async {
                        self.folderPaths = folderPaths
                    }
                }
            }
        }
    }
    
    var folderTreeView: some View {
        let root = DI.files.files.first(where: { $0.parent == $0.id })!
        let wc = WithChild(root, DI.files.files, { $0.id == $1.parent && $0.id != $1.id && $1.fileType == .Folder })
        
        
        return VStack {
            HStack {
                if error {
                    Text("Something went wrong, please exit and try again.")
                        .foregroundStyle(.red)
                        .fontWeight(.bold)
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
            }
            .padding(.bottom, 10)
            .padding(.horizontal)
            
            
            ScrollView {
                NestedList(
                    node: wc,
                    row: { dest in
                        Button(action: {
                            selectFolder(newParent: dest.id)
                        }, label: {
                            Label(dest.name, systemImage: FileService.metaToSystemImage(meta: dest))
                                .foregroundStyle(.foreground)
                        })
                    }
                )
            }
            .padding(.leading)
        }
        .padding(.top)
    }
    
    func selectFolder(path: String) {
        switch core.core.getFileByPath(path: path) {
        case .success(let parent):
            selectFolder(newParent: parent.id)
            print("got the folder id selected: \(path) to \(parent.id)")
        case .failure(_):
            error = true
        }
    }
    
    func selectFolder(newParent: UUID) {
        switch action {
        case .Move(let ids):
            for id in ids {
                if case .failure(_) = core.core.moveFile(id: id, newParent: newParent) {
                    error = true
                    return
                }
            }
            
            presentationMode.wrappedValue.dismiss()
            DI.files.successfulAction = .move
            DI.files.refresh()
        case .Import(let paths):
            if case .failure(_) = core.core.importFiles(sources: paths, destination: newParent) {
                error = true
            }
            
            presentationMode.wrappedValue.dismiss()
            DI.files.successfulAction = .importFiles
            DI.files.refresh()
        case .AcceptShare((let name, let id)):
            DI.share.acceptShare(targetName: name, targetId: id, parent: newParent)
            DI.files.successfulAction = .acceptedShare
            DI.files.refresh()
            DI.share.calculatePendingShares()
            presentationMode.wrappedValue.dismiss()
        }
    }
}

struct SelectedItemModifier: ViewModifier {
    
    let isSelected: Bool
    
    init(item: String, selected: String) {
        isSelected = item == selected
    }
    
    func body(content: Content) -> some View {
        if isSelected {
            content.listRowBackground(RoundedRectangle(cornerRadius: 5).fill(Color.gray.opacity(0.2)).padding(.horizontal, 15))
        } else {
            content
        }
    }
}

enum SelectFolderAction {
    case Move([UUID])
    case Import([String])
    case AcceptShare((String, UUID))
}

enum SelectFolderMode {
    case List
    case Tree
}

struct SelectFolderViewPreview: PreviewProvider {
    
    static var previews: some View {
        Color.white
            .sheet(isPresented: .constant(true), content: {
                SelectFolderView(folderPaths: ["cookies", "apples", "cookies/android/apple/nice"], action: .Import([]))
            })
            .mockDI()
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

struct SelectFolderTextField: UIViewRepresentable {
    var placeholder: String
    var onSubmit: () -> Void
    
    @Binding var text: String
    
    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }
    
    func makeUIView(context: Context) -> UITextField {
        let textField = UITextField()
        textField.delegate = context.coordinator
        textField.placeholder = placeholder
        textField.returnKeyType = .done
        textField.becomeFirstResponder()
        
        textField.addTarget(context.coordinator, action: #selector(Coordinator.textFieldDidChange(_:)), for: .editingChanged)
        
        return textField
    }
    
    func updateUIView(_ uiView: UITextField, context: Context) {
        uiView.text = text
    }
    
    class Coordinator: NSObject, UITextFieldDelegate {
        var parent: SelectFolderTextField
        
        init(parent: SelectFolderTextField) {
            self.parent = parent
        }

        @objc func textFieldDidChange(_ textField: UITextField) {
            parent.text = textField.text ?? ""
        }

        func textFieldShouldReturn(_ textField: UITextField) -> Bool {
            parent.onSubmit()
            return false
        }
    }
}
