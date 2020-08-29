//
//  FolderView.swift
//  ios
//
//  Created by Raayan Pillai on 7/5/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct FolderList: View {
    @ObservedObject var coordinator: Coordinator
    var dirId: UUID
    var dirName: String
    
    var body: some View {
        let files = self.coordinator.navigateAndListFiles(dirId: dirId).sorted(by: { (a, b) -> Bool in
            a.name < b.name
        })
        
        return List {
            ForEach(files){ file in
                if (file.fileType == .Folder) {
                    FolderRow(coordinator: self.coordinator, metadata: file)
                } else {
                    DocumentRow(coordinator: self.coordinator, metadata: file)
                }
            }
            .onDelete { offset in
                let meta = self.coordinator.files[offset.first!]
                self.coordinator.markFileForDeletion(id: meta.id)
            }
        }
        .navigationBarTitle(dirName)
        .navigationBarItems(
            leading: HStack {
              NavigationLink(destination: SettingsView(coordinator: self.coordinator)) {
                    Image(systemName: "dial")
                }
            },
            trailing: HStack {
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
        
        return NavigationView {
            FolderList(coordinator: coord, dirId: coord.root.id, dirName: "root")
            .previewLayout(.sizeThatFits)
            .preferredColorScheme(.dark)
        }
    }
}
