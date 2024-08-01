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
    
    var body: some View {
        if error {
            Text("Something went wrong, please exit and try again.")
        } else {
            Group {
                List(folderPaths, id: \.self) { path in
                    Button(action: {
                        selectFolder(path: path)
                    }, label: {
                        Text(path)
                    })
                }
                .searchable(text: $searchInput, prompt: "Type a folder name")
            }
            .onAppear {
                DispatchQueue.global(qos: .userInitiated).async {
                    switch core.core.listFolderPaths() {
                    case .success(let paths):
                        folderPaths = paths
                    case .failure(let e):
                        print("Failure in listing folder paths: \(e)")
                        error = true
                    }
                }
            }
        }
    }
    
    func selectFolder(path: String) {
        switch core.core.getFileByPath(path: path) {
        case .success(let meta):
            print("got the folder id selected: \(path) to \(meta.id)")
        case .failure(_):
            error = true
        }
    }
}

enum SelectFolderAction {
    case Move([UUID])
    case Import([URL])
}
