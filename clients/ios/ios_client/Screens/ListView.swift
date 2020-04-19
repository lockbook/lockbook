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
    @EnvironmentObject var screenCoordinator: ScreenCoordinator

    var body: some View {
        VStack {
            NavigationView {
                List {
                    ForEach(self.screenCoordinator.files) { file in
                        FileRow(lockbookApi: self.lockbookApi, metadata: file)
                    }
                }
                .navigationBarTitle("\(self.username)'s Files")
                .navigationBarItems(
                    leading: Button(action: {
                        self.screenCoordinator.files = self.lockbookApi.updateMetadata()
                    }, label: {
                        Image(systemName: "arrow.2.circlepath")
                    }),
                    trailing: NavigationLink(destination: CreateFileView(lockbookApi: self.lockbookApi)) {
                        Image(systemName: "plus")
                    }
                )
                .onAppear {
                    print("List -- Appearing")
                }
                .onDisappear {
                    print("List -- Disappearing")
                }

            }
            HStack {
                Spacer()
                Button(action: {
                    print("Purging files...")
                    let _ = self.lockbookApi.purgeFiles()
                    self.screenCoordinator.files = self.lockbookApi.updateMetadata()
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
        if let username = lockbookApi.getAccount() {
            self.username = username
        } else {
            self.username = "<USERNAME>"
        }
    }
}

struct ListView_Previews: PreviewProvider {
    static var previews: some View {
        ListView(lockbookApi: FakeApi()).environmentObject(ScreenCoordinator())
    }
}
