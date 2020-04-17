//
//  ListView.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/11/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct ListView: View {
    var lockbookApi: LockbookApi
    var username: String
    @State private var files: [FileMetadata]
    @EnvironmentObject var screenCoordinator: ScreenCoordinator

    var body: some View {
        VStack {
            NavigationView {
                List {
                    ForEach(files) { file in
                        FileRow(lockbookApi: self.lockbookApi, metadata: file)
                    }
                }
                .navigationBarTitle("\(self.username)'s Files")
                .navigationBarItems(trailing:
                    NavigationLink(destination: CreateFileView(lockbookApi: self.lockbookApi, files: self.$files)) {
                        Image(systemName: "plus")
                    }
                )
                .onAppear {
                    print("List -- Appearing")
                    self.files = self.lockbookApi.updateMetadata(sync: false)

                }
                .onDisappear {
                    print("List -- Disappearing")
                }

            }
            HStack {
                Spacer()
                Button(action: {
                }) {
                    HStack {
                        Image(systemName: "bolt")
                        Text("Reload")
                        Image(systemName: "bolt")
                    }
                }
                Spacer()
                Button(action: {
                    self.files = self.lockbookApi.updateMetadata(sync: true)
                }) {
                    HStack {
                        Image(systemName: "arrow.up.arrow.down")
                        Text("Sync")
                        Image(systemName: "arrow.up.arrow.down")
                    }
                    .foregroundColor(.green)
                }
                Spacer()
            }
            HStack {
                Spacer()
                Button(action: {
                    print("Purging files...")
                    self.lockbookApi.purgeFiles()
                    self.files = self.lockbookApi.updateMetadata(sync: false)
                }) {
                    HStack {
                        Image(systemName: "flame")
                        Text("Purge")
                        Image(systemName: "flame")
                    }
                    .foregroundColor(.red)
                }
                Spacer()
                Button(action: {
                    print("Logging out...")
                }) {
                    HStack {
                        Image(systemName: "person.badge.minus")
                        Text("Logout")
                        Image(systemName: "person.badge.minus")
                    }
                    .foregroundColor(.yellow)
                }
                Spacer()
            }
        }
    }
    
    init(lockbookApi: LockbookApi) {
        self.lockbookApi = lockbookApi
        self._files = State(initialValue: lockbookApi.updateMetadata(sync: false))
        if let username = lockbookApi.getAccount() {
            self.username = username
        } else {
            self.username = "<USERNAME>"
        }
    }
}

struct ListView_Previews: PreviewProvider {
    static var previews: some View {
        ListView(lockbookApi: FakeApi()).environmentObject(ScreenCoordinator()).colorScheme(.dark)
    }
}
