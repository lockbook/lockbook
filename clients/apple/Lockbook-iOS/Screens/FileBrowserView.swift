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
        FolderList(coordinator: self.coordinator, dir: self.coordinator.root, dirName: "\(self.coordinator.account.username)'s Files")
    }
}

struct ListView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            FileBrowserView(coordinator: Coordinator())
            }.preferredColorScheme(.dark)
    }
}
