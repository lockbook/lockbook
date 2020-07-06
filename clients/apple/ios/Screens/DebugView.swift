//
//  DebugView.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/19/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct DebugView: View {
    @EnvironmentObject var debugger: Debugger
    @EnvironmentObject var coordinator: Coordinator

    var body: some View {
        VStack {
            Spacer()
            Button(action: {
                print("Syncing files...")
                self.coordinator.sync()
            }) {
                HStack {
                    Image(systemName: "arrow.2.circlepath")
                    Text("Sync")
                    Image(systemName: "arrow.2.circlepath")
                }
                .foregroundColor(.green)
            }
            Button(action: {
                if let username = self.debugger.lockbookApi.getAccount() {
                    print("Username \(username)")
                } else {
                    print("Couldn't get username!")
                }
            }) {
                HStack {
                    Image(systemName: "person.circle")
                    Text("Print Account")
                    Image(systemName: "person.circle")
                }
            }
            Button(action: {
                print("Purging and syncing files in localdb...")
                let _ = self.debugger.lockbookApi.purgeLocal()
                self.coordinator.sync()
            }) {
                HStack {
                    Image(systemName: "trash")
                    Text("Purge Local")
                    Image(systemName: "trash")
                }
                .foregroundColor(.red)
            }
            Button(action: {
                let _ = self.debugger.lockbookApi.calculateWork()
            }) {
                HStack {
                    Image(systemName: "person.badge.minus")
                    Text("Calculate Work")
                    Image(systemName: "person.badge.minus")
                }
                .foregroundColor(.yellow)
            }
            Spacer()
        }
        .navigationBarTitle("Debugger")
    }
}

struct DebugView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            DebugView().environmentObject(Coordinator()).environmentObject(Debugger())
        }
    }
}
