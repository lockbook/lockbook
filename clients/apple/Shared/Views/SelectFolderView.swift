import Foundation
import SwiftUI
import CLockbookCore
import SwiftLockbookCore
import UIKit

class SelectFolderViewModel: ObservableObject {
    @Published var searchInput: String = ""
    @Published var error: String? = nil
    
    @Published var folderPaths: [String]? = nil
    var filteredFolderPaths: [String] {
        if let folderPaths = folderPaths {
            if searchInput.isEmpty {
                return folderPaths
            } else {
                return folderPaths.filter { path in
                    path.localizedCaseInsensitiveContains(searchInput) && !ignorePrefixPaths.contains(where: { path.hasPrefix($0) })
                }
            }
        } else {
            return []
        }
    }
    
    @Published var selected = 0 {
        didSet {
            recomputeSelectedPath()
        }
    }
    @Published var selectedPath: String = ""
    
    @Published var ignorePrefixPaths: [String] = []
    @Published var ignoreParentIds: [UUID] = []
    
    func recomputeSelectedPath() {
        if filteredFolderPaths.count <= selected {
            selectedPath = ""
        } else {
            selectedPath = filteredFolderPaths[selected]
        }
    }
    
    func calculateFolderPaths() {
        DispatchQueue.global(qos: .userInitiated).async {
            switch DI.files.getFolderPaths() {
            case .none:
                DispatchQueue.main.async {
                    self.error = "Could not get folder paths."
                }
            case .some(let folderPaths):
                DispatchQueue.main.async {
                    self.folderPaths = folderPaths
                }
            }
        }
    }
    
    func filterFolders(action: SelectFolderAction) {
        DispatchQueue.global(qos: .userInitiated).async {
            if case .Move(let ids) = action {
                DispatchQueue.main.sync {
                    self.ignoreParentIds = ids
                }
                for id in ids {
                    switch DI.core.getPathById(id: id) {
                    case .success(let path):
                        DispatchQueue.main.sync {
                            self.ignorePrefixPaths.append(path)
                        }
                    case .failure(let cError):
                        DispatchQueue.main.sync {
                            self.error = cError.description
                        }
                    }
                }
            }
        }
    }
    
    func selectFolder(action: SelectFolderAction, path: String) -> Bool {
        switch DI.core.getFileByPath(path: path) {
        case .success(let parent):
            print("got the folder id selected: \(path) to \(parent.id)")
            return selectFolder(action: action, newParent: parent.id)
        case .failure(let cError):
            error = cError.description
            
            return false
        }
    }
    
    func selectFolder(action: SelectFolderAction, newParent: UUID) -> Bool {
        switch action {
        case .Move(let ids):
            for id in ids {
                if case .failure(let cError) = DI.core.moveFile(id: id, newParent: newParent) {
                    error = cError.description

                    return false
                }
            }
            
            DI.files.successfulAction = .move
            DI.files.refresh()
            
            return true
        case .Import(let paths):
            if case .failure(let cError) = DI.core.importFiles(sources: paths, destination: newParent) {
                error = cError.description
                
                return false
            }
            
            DI.files.successfulAction = .importFiles
            DI.files.refresh()
            
            return true
        case .AcceptShare((let name, let id)):
            if case .failure(let cError) = DI.core.createLink(name: name, dirId: newParent, target: id) {
                error = cError.description
                
                return false
            }
            
            DI.files.successfulAction = .acceptedShare
            DI.files.refresh()
            DI.share.calculatePendingShares()
            
            return true
        }
    }

}

struct SelectFolderView: View {
    @EnvironmentObject var core: CoreService
    @StateObject var viewModel = SelectFolderViewModel()

    let action: SelectFolderAction
    @State var mode: SelectFolderMode = .Tree
    
    @Environment(\.dismiss) private var dismiss

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
        Group {
            switch mode {
            case .List:
                folderListView
            case .Tree:
                folderTreeView
            }
        }
        .onAppear(perform: {
            viewModel.filterFolders(action: action)
        })
    }
    
    var folderListView: some View {
        VStack {
            VStack {
                HStack {
                    SelectFolderTextFieldWrapper(placeholder: "Search folder", onSubmit: {
                        if viewModel.selectFolder(action: action, path: viewModel.selectedPath.isEmpty ? "/" : viewModel.selectedPath) {
                            dismiss()
                        }
                    }, viewModel: viewModel)
                        .frame(height: 19)
                        .onChange(of: viewModel.searchInput) { _ in
                            viewModel.selected = 0
                        }
                    
                    if !viewModel.searchInput.isEmpty {
                        Button(action: {
                            viewModel.searchInput = ""
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
                if let error = viewModel.error {
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
            
            if viewModel.folderPaths != nil {
                List(viewModel.filteredFolderPaths, id: \.self) { path in
                    HStack {
                        Button(action: {
                            if viewModel.selectFolder(action: action, path: path.isEmpty ? "/" : path) {
                                dismiss()
                            }
                        }, label: {
                            HighlightedText(text: path.isEmpty ? "/" : path, pattern: viewModel.searchInput, textSize: 16)
                        })
                        
                        Spacer()
                    }
                    .modifier(SelectedItemModifier(item: path.isEmpty ? "/" : path, selected: viewModel.selectedPath))
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
            viewModel.calculateFolderPaths()
        }
    }
    
    var folderTreeView: some View {
        let root = DI.files.files.first(where: { $0.parent == $0.id })!
        let wc = WithChild(root, DI.files.files, { parent, meta in
            parent.id == meta.parent && parent.id != meta.id && meta.fileType == .Folder && !viewModel.ignoreParentIds.contains(where: { id in meta.id == id })
        })
        
        
        return VStack {
            HStack {
                if let error = viewModel.error {
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
            }
            .padding(.bottom, 10)
            .padding(.horizontal)
            
            
            ScrollView {
                NestedList(
                    node: wc,
                    row: { dest in
                        Button(action: {
                            if viewModel.selectFolder(action: action, newParent: dest.id) {
                                dismiss()
                            }
                        }, label: {
                            Label(dest.name, systemImage: FileService.metaToSystemImage(meta: dest))
                                .foregroundStyle(.foreground)
                        })
                    }
                )
                .padding(.bottom)
            }
            .padding(.leading)
        }
        .padding(.top)
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

//struct SelectFolderViewPreview: PreviewProvider {
//    
//    static var previews: some View {
//        Color.white
//            .sheet(isPresented: .constant(true), content: {
//                SelectFolderView(folderPaths: ["cookies", "apples", "cookies/android/apple/nice"], action: .Import([]))
//            })
//            .mockDI()
//    }
//}

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

struct SelectFolderTextFieldWrapper: UIViewRepresentable {
    var placeholder: String
    var onSubmit: () -> Void
        
    @StateObject var viewModel: SelectFolderViewModel
    
    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }
    
    func makeUIView(context: Context) -> SelectFolderTextField {
        let textField = SelectFolderTextField()
        textField.delegate = context.coordinator
        textField.placeholder = placeholder
        textField.returnKeyType = .done
        textField.viewModel = viewModel
        
        textField.becomeFirstResponder()
        
        textField.addTarget(context.coordinator, action: #selector(Coordinator.textFieldDidChange(_:)), for: .editingChanged)
        
        return textField
    }
    
    func updateUIView(_ uiView: SelectFolderTextField, context: Context) {
        uiView.text = viewModel.searchInput
    }
        
    class Coordinator: NSObject, UITextFieldDelegate {
        var parent: SelectFolderTextFieldWrapper
        
        init(parent: SelectFolderTextFieldWrapper) {
            self.parent = parent
        }

        @objc func textFieldDidChange(_ textField: UITextField) {
            parent.viewModel.searchInput = textField.text ?? ""
        }

        func textFieldShouldReturn(_ textField: UITextField) -> Bool {
            parent.onSubmit()
            return false
        }
    }
}

class SelectFolderTextField: UITextField {
    
    var viewModel: SelectFolderViewModel? = nil
    
    override var keyCommands: [UIKeyCommand]? {
        let selectedUp = UIKeyCommand(input: UIKeyCommand.inputUpArrow, modifierFlags: [], action: #selector(incrementSelected))
        let selectedDown = UIKeyCommand(input: UIKeyCommand.inputDownArrow, modifierFlags: [], action: #selector(decrementSelected))
        
        selectedUp.wantsPriorityOverSystemBehavior = true
        selectedDown.wantsPriorityOverSystemBehavior = true
                
        return [
            selectedUp,
            selectedDown,
        ]
    }
    
    @objc func incrementSelected() {
        if let viewModel = viewModel {
            print("decremented!")
            viewModel.selected = max(viewModel.selected - 1, 0)
        }
    }
    
    @objc func decrementSelected() {
        if let viewModel = viewModel {
            print("incremented!! from \(viewModel.selected) to min(\(viewModel.selected + 1) or \(viewModel.filteredFolderPaths.count - 1))")
            viewModel.selected = min(viewModel.selected + 1, viewModel.filteredFolderPaths.count - 1)
        }
    }
}

