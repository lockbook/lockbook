//
//  FolderView.swift
//  ios
//
//  Created by Raayan Pillai on 7/5/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct FolderList: View {
    var dirId: UUID
    var dirName: String
    @EnvironmentObject var coordinator: Coordinator
    
    var body: some View {
        let files = coordinator.listFiles(dirId: dirId).sorted(by: { (a, b) -> Bool in
            a.name < b.name
        })
        
        return List {
            ForEach(files){ file in
                if (file.fileType == .Folder) {
                    FolderRow(metadata: file)
                } else {
                    DocumentRow(metadata: file)
                }
            }
            .onDelete { offset in
                let meta = coordinator.files[offset.first!]
                coordinator.markFileForDeletion(id: meta.id)
            }
        }
        .navigationBarTitle(dirName)
        .navigationBarItems(
            trailing: HStack {
                NavigationLink(destination: DebugView()) {
                    Image(systemName: "circle.grid.hex")
                }
                NavigationLink(destination: CreateFileView()) {
                    Image(systemName: "plus")
                }
            }
        )
    }
}

struct FolderView_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            FolderList(dirId: UUID.init(), dirName: "root").environmentObject(Coordinator())
        }
        .previewLayout(.sizeThatFits)
    }
}
