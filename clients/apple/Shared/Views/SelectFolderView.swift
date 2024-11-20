import Foundation
import SwiftUI
import SwiftWorkspace

struct SelectFolderView: View {
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
                            Button(action: {
                                if viewModel.selectFolder(action: action, path: path.isEmpty ? "/" : path) {
                                    dismiss()
                                }
                            }, label: {
                                HStack {
                                    HighlightedText(text: path.isEmpty ? "/" : path, pattern: viewModel.searchInput, textSize: 16)
                                        .foregroundStyle(.foreground)
                                        .multilineTextAlignment(.leading)
                                    
                                    Spacer()
                                }
                            })
                            .padding(.horizontal)
                            .padding(.vertical, 5)
                            .modifier(SelectedItemModifier(item: path.isEmpty ? "/" : path, selected: viewModel.selectedPath))
                            .modifier(PlatformSelectFolderButtonModifier())
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
        let wc = WithChild(DI.files.root!, DI.files.files, { (parent: File, meta: File) in
            parent.id == meta.parent && parent.id != meta.id && meta.type == .folder
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
                    row: { (dest: File) in
                        Button(action: {
                            if viewModel.selectFolder(action: action, newParent: dest.id) {
                                dismiss()
                            }
                        }, label: {
                            Label {
                                Text(dest.name)
                                    .foregroundStyle(.foreground)
                            } icon: {
                                Image(systemName: FileService.metaToSystemImage(meta: dest))
                                    .foregroundStyle(Color.blue)
                            }

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

struct SelectedItemModifier: ViewModifier {
    let isSelected: Bool
    
    init(item: String, selected: String) {
        isSelected = item == selected
    }
    
    func body(content: Content) -> some View {
        if isSelected {
            content.background(RoundedRectangle(cornerRadius: 5).fill(Color.gray.opacity(0.2)).padding(.horizontal, 10))
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

