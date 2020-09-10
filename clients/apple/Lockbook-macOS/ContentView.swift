//
//  ContentView.swift
//  macos
//
//  Created by Raayan Pillai on 5/25/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI
import SwiftLockbookCore

struct ContentView: View {
    var lockbookApi: LockbookApi
    var body: some View {
        NavigationView {
            List((try? lockbookApi.listFiles().get()) ?? []) { item in
                NavigationLink(destination: ItemView(content: (try? self.lockbookApi.getFile(id: item.id).get().secret) ?? "Could not load \(item.name)")) {
                    Text(item.name)
                }
            }
        }
    }
}


struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView(lockbookApi: FakeApi())
    }
}
