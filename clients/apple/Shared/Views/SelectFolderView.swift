//
//  SelectFolderView.swift
//  Lockbook
//
//  Created by Smail Barkouch on 7/31/24.
//

import Foundation
import SwiftUI
import CLockbookCore
import SwiftLockbookCore

struct SelectFolderView: View {
    @EnvironmentObject var core: CoreService
    
    @State var searchInput: String = ""
    @State var error: Bool = false
    
    @Environment(\.presentationMode) var presentationMode
    
    @State var folderPaths: [String]? = nil
    let action: SelectFolderAction
    
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

    
    var body: some View {
        if error {
            Text("Something went wrong, please exit and try again.")
        } else {
            Group {
                if folderPaths != nil {
                    VStack {
                        SelectFolderSearchBar(text: $searchInput, placeholder: "Search folder") {
                            guard let selectedFolder = filteredFolderPaths.first else {
                                return
                            }
                            
                            selectFolder(path: selectedFolder.isEmpty ? "/" : selectedFolder)
                        }
                        
                        switch action {
                        case .Move(let ids):
                            Text("Moving \(ids.count) files.")
                        case .Import(let ids):
                            Text("Importing \(ids.count) files.")
                        }
                        
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
                    }
                } else {
                    ProgressView()
                }
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
    }
    
    func selectFolder(path: String) {
        switch core.core.getFileByPath(path: path) {
        case .success(let parent):
            switch action {
            case .Move(let ids):
                for id in ids {
                    if case .failure(_) = core.core.moveFile(id: id, newParent: parent.id) {
                        error = true
                        return
                    }
                }
                
                presentationMode.wrappedValue.dismiss()
                DI.files.successfulAction = .move
                DI.files.refresh()
            case .Import(let paths):
                if case .failure(_) = core.core.importFiles(sources: paths, destination: parent.id) {
                    error = true
                }
                
                presentationMode.wrappedValue.dismiss()
                DI.files.successfulAction = .importFiles
                DI.files.refresh()
            }
            
            print("got the folder id selected: \(path) to \(parent.id)")
        case .failure(_):
            error = true
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


struct SelectFolderSearchBar: View {
    @Binding var text: String
    var placeholder: String
    let onSubmit: () -> Void
    
    @Environment(\.presentationMode) var presentationMode
    
    @FocusState var isSearchFocused: Bool
    
    var body: some View {
        VStack {
            HStack {
                SelectFolderTextField(placeholder: placeholder, onSubmit: onSubmit, text: $text)
                    .frame(height: 19)
                
                Button(action: {
                    presentationMode.wrappedValue.dismiss()
                }, label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.gray)
                })
                .padding(.leading)
            }
            .padding(.horizontal)
            .padding(.bottom, 4)
            .padding(.top)
            
            
            Divider()
        }
    }
}

import SwiftUI
import UIKit

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
            print("pressed enter! \(parent.text)")
            parent.onSubmit()
            return false
        }
    }
}
