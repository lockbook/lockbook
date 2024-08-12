import Foundation
import SwiftUI
import CLockbookCore
import SwiftLockbookCore

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
                    path.localizedCaseInsensitiveContains(searchInput)
                }
            }
        } else {
            return []
        }
    }
    
    @Published var selected = 0
    var selectedPath: String {
        get {
            if filteredFolderPaths.count <= selected {
                return ""
            }
            
            return filteredFolderPaths[selected].isEmpty ? "/" : filteredFolderPaths[selected]
        }
    }
    
    var exit: Bool = false
    
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
                    switch cError.kind {
                    case .UiError(.FolderMovedIntoItself):
                        error = "You cannot move a folder into itself."
                    case .UiError(.InsufficientPermission):
                        error = "You do not have the permission to do that."
                    case .UiError(.LinkInSharedFolder):
                        error = "You cannot move a link into a shared folder."
                    case .UiError(.TargetParentHasChildNamedThat):
                        error = "A child with that name already exists in that folder."
                    default:
                        error = cError.description
                    }

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
        .onChange(of: viewModel.exit) { newValue in
            if newValue {
                dismiss()
            }
        }
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
                        .modifier(PlatformSelectFolderButtonModifier())
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
                    .modifier(PlatformSelectFolderButtonModifier())
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
                ScrollViewReader { scrollHelper in
                    ScrollView {
                        ForEach(viewModel.filteredFolderPaths, id: \.self) { path in
                            HStack {
                                Button(action: {
                                    if viewModel.selectFolder(action: action, path: path.isEmpty ? "/" : path) {
                                        dismiss()
                                    }
                                }, label: {
                                    HighlightedText(text: path.isEmpty ? "/" : path, pattern: viewModel.searchInput, textSize: 16)
                                        .foregroundStyle(.foreground)
                                        .multilineTextAlignment(.leading)
                                })
                                .modifier(PlatformSelectFolderButtonModifier())
                                
                                Spacer()
                            }
                            .padding(.horizontal)
                            .padding(.vertical, 5)
                            .modifier(SelectedItemModifier(item: path.isEmpty ? "/" : path, selected: viewModel.selectedPath))
                        }
                    }
                    .onChange(of: viewModel.selected) { newValue in
                        if viewModel.selected < viewModel.filteredFolderPaths.count {
                            withAnimation {
                                scrollHelper.scrollTo(viewModel.selectedPath, anchor: .center)
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
            viewModel.calculateFolderPaths()
        }
    }
    
    var folderTreeView: some View {
        let wc = WithChild(DI.files.root!, DI.files.files, { parent, meta in
            parent.id == meta.parent && parent.id != meta.id && meta.fileType == .Folder
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
                .modifier(PlatformSelectFolderButtonModifier())
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
                        .modifier(PlatformSelectFolderButtonModifier())
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
            content.background(RoundedRectangle(cornerRadius: 5).fill(Color.gray.opacity(0.2)).padding(.horizontal, 5))
        } else {
            content
        }
    }
}

struct PlatformSelectFolderButtonModifier: ViewModifier {
    func body(content: Content) -> some View {
        #if os(iOS)
        content
        #else
        content.buttonStyle(.plain)
        #endif
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

#if os(iOS)
import UIKit

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
        let selectedUp = UIKeyCommand(input: UIKeyCommand.inputUpArrow, modifierFlags: [], action: #selector(selectedUp))
        let selectedDown = UIKeyCommand(input: UIKeyCommand.inputDownArrow, modifierFlags: [], action: #selector(selectedDown))
        let exit = UIKeyCommand(input: UIKeyCommand.inputEscape, modifierFlags: [], action: #selector(exit))
        
        selectedUp.wantsPriorityOverSystemBehavior = true
        selectedDown.wantsPriorityOverSystemBehavior = true
        exit.wantsPriorityOverSystemBehavior = true
                
        return [
            selectedUp,
            selectedDown,
            exit
        ]
    }
    
    @objc func selectedUp() {
        if let viewModel = viewModel {
            viewModel.selected = max(viewModel.selected - 1, 0)
        }
    }
    
    @objc func selectedDown() {
        if let viewModel = viewModel {
            viewModel.selected = min(viewModel.selected + 1, viewModel.filteredFolderPaths.count - 1)
        }
    }
    
    @objc func exit() {
        viewModel?.exit = true
    }
}

#else

struct SelectFolderTextFieldWrapper: NSViewRepresentable {
    var placeholder: String
    var onSubmit: () -> Void

    let viewModel: SelectFolderViewModel
    
    public func makeNSView(context: NSViewRepresentableContext<SelectFolderTextFieldWrapper>) -> SelectFolderTextField {
        let textField = SelectFolderTextField()

        textField.isBordered = false
        textField.focusRingType = .none
        textField.delegate = context.coordinator
        textField.placeholderString = placeholder
        textField.onSubmit = onSubmit
        textField.viewModel = viewModel
        
        textField.becomeFirstResponder()
        
        return textField
    }
    
    public func updateNSView(_ nsView: SelectFolderTextField, context: NSViewRepresentableContext<SelectFolderTextFieldWrapper>) {
        if nsView.currentEditor() == nil {
            nsView.becomeFirstResponder()
        }
    }
    
    public func makeCoordinator() -> SelectFolderTextFieldDelegate {
        SelectFolderTextFieldDelegate(self)
    }
    
    public class SelectFolderTextFieldDelegate: NSObject, NSTextFieldDelegate {
        var parent: SelectFolderTextFieldWrapper

        public init(_ parent: SelectFolderTextFieldWrapper) {
            self.parent = parent
        }

        public func controlTextDidChange(_ obj: Notification) {
            if let textField = obj.object as? NSTextField {
                parent.viewModel.searchInput = textField.stringValue
            }
        }
    }

}

class SelectFolderTextField: NSTextField {
    
    var viewModel: SelectFolderViewModel? = nil
    var onSubmit: (() -> Void)? = nil
    
    public override func performKeyEquivalent(with event: NSEvent) -> Bool {
        switch event.keyCode {
        case 126: // up arrow
            selectedUp()
            return true
        case 125: // down arrow
            selectedDown()
            return true
        case 36: // return
            onSubmit?()
            return true
        default:
            return super.performKeyEquivalent(with: event)
        }
    }
    
    func selectedUp() {
        if let viewModel = viewModel {
            viewModel.selected = max(viewModel.selected - 1, 0)
        }
    }
    
    func selectedDown() {
        if let viewModel = viewModel {
            viewModel.selected = min(viewModel.selected + 1, viewModel.filteredFolderPaths.count - 1)
        }
    }
    
    func exit() {
        viewModel?.exit = true
    }
}


#endif
