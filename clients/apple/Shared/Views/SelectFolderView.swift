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
    
    @State var folderPaths: [String] = []
    @State var error: Bool = false
    
    let action: SelectFolderAction
    
    var body: some View {
        if error {
            Text("Something went wrong, please exit and try again.")
        } else {
            NavigationView {
                List(folderPaths, id: \.self) { path in
                    HStack {
                        Button(action: {
                            selectFolder(path: path)
                        }, label: {
                            Text(path)
                        })
                        
                        Spacer()
                    }
                    .background(.red)
                }
                .listStyle(.inset)
            }.searchable(text: $searchInput, prompt: "Type a folder name")
//            .onAppear {
//                DispatchQueue.global(qos: .userInitiated).async {
//                    switch core.core.listFolderPaths() {
//                    case .success(let paths):
//                        folderPaths = paths
//                    case .failure(let e):
//                        print("Failure in listing folder paths: \(e)")
//                        error = true
//                    }
//                }
//            }
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
            case .Import(_):
                print("unused")
            }
            
            print("got the folder id selected: \(path) to \(parent.id)")
        case .failure(_):
            error = true
        }
    }
}

enum SelectFolderAction {
    case Move([UUID])
    case Import([URL])
}

struct SelectFolderViewPreview: PreviewProvider {
    
    static var previews: some View {
        Color.white
            .sheet(isPresented: .constant(true), content: {
                SelectFolderView(folderPaths: ["cookies", "apples"], action: .Import([]))
            })
            .mockDI()
    }
}

