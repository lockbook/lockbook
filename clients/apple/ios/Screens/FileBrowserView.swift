//
//  ListView.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/11/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct FileBrowserView: View {
    @ObservedObject var coordinator: Coordinator
    
    var body: some View {
        NavigationView {
            self.coordinator.getRoot().map {
                FolderList(coordinator: self.coordinator, dirId: $0, dirName: "root")
            } ?? (try? FakeApi().getRoot().map {
                FolderList(coordinator: self.coordinator, dirId: $0.id, dirName: $0.name)
                }.get()).unsafelyUnwrapped
        }
    }
}

struct ListView_Previews: PreviewProvider {
    static var previews: some View {
        FileBrowserView(coordinator: Coordinator()).preferredColorScheme(.dark)
    }
}
