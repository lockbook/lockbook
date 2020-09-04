//
//  FolderView.swift
//  ios
//
//  Created by Raayan Pillai on 7/5/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI
import SwiftLockbookCore

struct FolderList: View {
    @ObservedObject var coordinator: Coordinator
    @State var path: [FileMetadata] = []
    @State var dir: FileMetadata
    
    func dirName() -> String {
        if (path.isEmpty) {
            return "\(self.coordinator.account.username)'s Files"
        } else {
            return "\(path.map { $0.name.prefix(1) }.joined(separator: "/"))/\(dir.name)"
        }
    }
    
    var body: some View {
        let files = self.coordinator.navigateAndListFiles(dirId: dir.id).sorted(by: { (a, b) -> Bool in
            a.name < b.name
        })
        
        return List {
            ForEach(files){ file in
                if (file.fileType == .Folder) {
                    FolderRow(coordinator: self.coordinator, metadata: file).onTapGesture {
                        self.path.append(self.dir)
                        self.dir = file
                    }
                } else {
                    DocumentRow(coordinator: self.coordinator, metadata: file)
                }
            }
            .onDelete { offset in
                let meta = self.coordinator.files[offset.first!]
                self.coordinator.markFileForDeletion(id: meta.id)
            }
        }
        .navigationBarTitle(dirName())
        .navigationBarItems(
            leading: HStack {
                self.path.last.map { parent in
                    Image(systemName: "arrow.turn.left.up")
                    .onTapGesture {
                        let _ = self.path.popLast()
                        self.dir = parent
                    }
                    .onLongPressGesture {
                        self.path.first.map {
                            self.path = []
                            self.dir = $0
                        }
                    }
                }
            },
            trailing: HStack {
                NavigationLink(destination: SettingsView(coordinator: self.coordinator)) {
                    Image(systemName: "dial")
                }
                NavigationLink(destination: CreateFileView(coordinator: self.coordinator)) {
                    Image(systemName: "plus")
                }
            }
        )
    }
}

struct FolderView_Previews: PreviewProvider {
    
    static var previews: some View {
        let coord = Coordinator()
        
        return Group {
            NavigationView {
                FolderList(coordinator: coord, dir: coord.root)
                .previewLayout(.sizeThatFits)
                .preferredColorScheme(.dark)
            }
        }
    }
}
