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
    @ObservedObject var coordinator: Coordinator
    
    func fail() -> Void {
        print("Failure!")
    }
    
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Spacer()
            Button(action: {
                self.coordinator.sync()
            }) {
                HStack {
                    Image(systemName: "arrow.up.arrow.down.circle.fill")
                    Text("Sync")
                }
                .foregroundColor(.green)
            }
            Button(action: {
                self.coordinator.iterativeSync()
            }) {
                HStack {
                    Image(systemName: "arrow.up.and.down.circle.fill")
                    Text("Iterative Sync")
                }
                .foregroundColor(.yellow)
            }
            Button(action: {
                self.coordinator.reloadFiles()
            }) {
                HStack {
                    Image(systemName: "arrow.2.circlepath.circle.fill")
                    Text("Reload Files")
                }
                .foregroundColor(.pink)
            }
            Button(action: {
                if case .success(let username) = self.debugger.lockbookApi.getAccount() {
                    print("Username \(username)")
                } else {
                    self.fail()
                }
            }) {
                HStack {
                    Image(systemName: "person.circle.fill")
                    Text("Dump Account")
                }
                .foregroundColor(.purple)
            }
            Spacer()
            Toggle(isOn: self.$coordinator.autoSync) {
                Text("Auto-Sync")
            }
            Toggle(isOn: self.$coordinator.iterativeAutoSync) {
                Text("Iterative")
            }
            Spacer()
        }
        .padding(.horizontal, 100)
        .navigationBarTitle("Debugger")
    }
}

struct DebugView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            DebugView(coordinator: Coordinator()).environmentObject(Debugger())
        }
    }
}
